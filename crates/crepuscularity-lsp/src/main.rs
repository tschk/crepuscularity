use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crepuscularity_lsp::{
    completion_items_with_path, crepus_diagnostics_to_lsp_with_path, hover_for_with_path,
};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{Request, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Document {
    text: String,
    version: i32,
}

impl Document {
    fn replace_if_newer(&mut self, text: String, version: i32) -> bool {
        if version <= self.version {
            return false;
        }
        self.text = text;
        self.version = version;
        true
    }
}

#[derive(Debug)]
struct Backend {
    client: Client,
    docs: Arc<RwLock<HashMap<Url, Document>>>,
    shutdown: Arc<AtomicBool>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "crepus-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    // `<`, ` `, and `{` cover the three contexts our completion
                    // logic recognizes (JSX tag, class slot, expression slot).
                    trigger_characters: Some(vec![
                        "<".to_string(),
                        " ".to_string(),
                        "{".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        let version = params.text_document.version;
        // Keep state changes and their queued diagnostics in the same lifecycle order.
        let mut docs = self.docs.write().await;
        docs.insert(
            uri.clone(),
            Document {
                text: text.clone(),
                version,
            },
        );
        self.publish_doc_diagnostics(uri, text, version).await;
        drop(docs);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(last) = params.content_changes.into_iter().last() {
            let version = params.text_document.version;
            let mut docs = self.docs.write().await;
            let Some(doc) = docs.get_mut(&uri) else {
                return;
            };
            if !doc.replace_if_newer(last.text.clone(), version) {
                return;
            }
            self.publish_doc_diagnostics(uri, last.text, version).await;
            drop(docs);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut docs = self.docs.write().await;
        docs.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
        drop(docs);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let docs = self.docs.read().await;
        let Some(text) = docs.get(&uri).map(|doc| doc.text.clone()) else {
            return Ok(None);
        };
        drop(docs);
        let path = uri.to_file_path().unwrap_or_else(|_| uri.path().into());
        let items = completion_items_with_path(&text, position, Some(&path));
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let docs = self.docs.read().await;
        let Some(text) = docs.get(&uri).map(|doc| doc.text.clone()) else {
            return Ok(None);
        };
        drop(docs);
        let path = uri.to_file_path().unwrap_or_else(|_| uri.path().into());
        Ok(hover_for_with_path(&text, position, Some(&path)))
    }
}

impl Backend {
    async fn publish_doc_diagnostics(&self, uri: Url, text: String, version: i32) {
        let path = uri.to_file_path().unwrap_or_else(|_| uri.path().into());
        let diagnostics = crepus_diagnostics_to_lsp_with_path(&text, Some(&path));
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--stdio") {
        eprintln!("crepus-lsp: pass --stdio (VS Code / Zed language client)");
        std::process::exit(2);
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let shutdown = Arc::new(AtomicBool::new(false));
    let (service, socket) = LspService::build(|client| Backend {
        client,
        docs: Arc::new(RwLock::new(HashMap::new())),
        shutdown: shutdown.clone(),
    })
    .finish();
    let service = tower::ServiceBuilder::new()
        .map_request(move |request: Request| {
            // tower-lsp waits for more stdin after exit; this standalone server must terminate.
            if request.method() == "exit" && request.id().is_none() {
                std::process::exit(if shutdown.load(Ordering::SeqCst) {
                    0
                } else {
                    1
                });
            }
            request
        })
        .service(service);

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use tower::Service;

    #[test]
    fn document_changes_require_a_strictly_newer_version() {
        let mut doc = Document {
            text: "current".into(),
            version: -2,
        };
        for version in [i32::MIN, -3, -2] {
            assert!(!doc.replace_if_newer("stale".into(), version));
            assert_eq!(doc.text, "current");
            assert_eq!(doc.version, -2);
        }
        assert!(doc.replace_if_newer(String::new(), -1));
        assert_eq!(doc.text, "");
        assert_eq!(doc.version, -1);
        assert!(doc.replace_if_newer("latest".into(), i32::MAX));
        assert!(!doc.replace_if_newer("duplicate".into(), i32::MAX));
        assert_eq!(doc.text, "latest");
    }

    #[tokio::test]
    async fn blocked_publication_keeps_document_lifecycle_ordered() {
        let (mut service, _socket) = LspService::new(|client| Backend {
            client,
            docs: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
        });
        let initialize: Request =
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#
                .parse()
                .unwrap();
        service.call(initialize).await.unwrap();
        let backend = service.inner();
        let mut context = Context::from_waker(Waker::noop());
        let mut fill = Box::pin(async {
            loop {
                backend.client.show_message(MessageType::INFO, "fill").await;
            }
        });
        assert!(matches!(fill.as_mut().poll(&mut context), Poll::Pending));
        drop(fill);
        let uri: Url = "file:///tmp/crepus-lifecycle.crepus".parse().unwrap();
        let mut open = Box::pin(backend.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "crepus".into(),
                version: 1,
                text: "div".into(),
            },
        }));
        assert!(open.as_mut().poll(&mut context).is_pending());
        assert!(
            backend.docs.try_write().is_err(),
            "publication released the lifecycle lock before enqueueing"
        );
        let mut close = Box::pin(backend.did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        }));
        assert!(close.as_mut().poll(&mut context).is_pending());
        drop(open);
        drop(close);
        assert!(backend.docs.try_write().is_ok());
    }
}
