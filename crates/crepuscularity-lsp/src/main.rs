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
struct Backend {
    client: Client,
    docs: Arc<RwLock<HashMap<Url, TextDocumentItem>>>,
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
        let document = params.text_document;
        let mut docs = self.docs.write().await;
        self.publish_doc_diagnostics(&document).await;
        docs.insert(document.uri.clone(), document);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(last) = params.content_changes.into_iter().last() {
            let mut docs = self.docs.write().await;
            let Some(document) = docs.get_mut(&uri) else {
                return;
            };
            if params.text_document.version <= document.version {
                return;
            }
            document.text = last.text;
            document.version = params.text_document.version;
            self.publish_doc_diagnostics(document).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.docs.write().await;
        let uri = params.text_document.uri;
        docs.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
        drop(docs);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let docs = self.docs.read().await;
        let Some(text) = docs.get(&uri).map(|document| document.text.clone()) else {
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
        let Some(text) = docs.get(&uri).map(|document| document.text.clone()) else {
            return Ok(None);
        };
        drop(docs);
        let path = uri.to_file_path().unwrap_or_else(|_| uri.path().into());
        Ok(hover_for_with_path(&text, position, Some(&path)))
    }
}

impl Backend {
    async fn publish_doc_diagnostics(&self, document: &TextDocumentItem) {
        let path = document
            .uri
            .to_file_path()
            .unwrap_or_else(|_| document.uri.path().into());
        let diagnostics = crepus_diagnostics_to_lsp_with_path(&document.text, Some(&path));
        self.client
            .publish_diagnostics(document.uri.clone(), diagnostics, Some(document.version))
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
            // tower-lsp stops its service on exit but still waits for stdin to close.
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
