//! Streaming SSR: renders on a blocking thread, sending the head before body chunks.
//!
//! The HTML5 document shell (`<!DOCTYPE html>` + `<head>` section) is rendered and flushed
//! first so the browser can start loading external resources (fonts, CSS, etc.)
//! while the body is still rendering. The rendered body content follows as a second chunk.
use crate::ssr::{append_hydration_payload, render_document_head};
use crate::{render_nodes_ssr, BindMap, SsrDocument};
use axum::body::Body;
use axum::response::{IntoResponse, Response};
use crepuscularity_core::ast::Node;
use crepuscularity_core::TemplateContext;
use http::StatusCode;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Stream an SSR response with true head-first streaming.
///
/// Renders the HTML5 document head in a blocking task and sends it as the first chunk.
/// The body is rendered via [`render_nodes_ssr`] and sent as subsequent chunks.
/// The closing `</body></html>` is appended as the final chunk.
///
/// `doc` must use `'static` lifetime because it is moved into the blocking task.
pub async fn stream_ssr_response_with_nodes(
    nodes: Arc<Vec<Node>>,
    ctx: TemplateContext,
    doc: SsrDocument<'static>,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>(8);

    // Build and send the head before rendering the body on the blocking thread.
    tokio::task::spawn_blocking(move || {
        let head_html = render_document_head(&doc);
        if tx.blocking_send(Ok(head_html.into_bytes())).is_err() {
            return;
        }

        let counter = Cell::new(0u32);
        let mut bind = BindMap::new();
        let render_result =
            render_nodes_ssr(&nodes, &ctx, &counter, &mut bind, true).and_then(|mut html| {
                append_hydration_payload(&mut html, &ctx, &bind)?;
                Ok(html)
            });
        let body: String = match render_result {
            Ok(h) => h,
            Err(e) => {
                let err = format!(
                    "<pre style='color:red'>Render error: {}</pre>\n</body>\n</html>\n",
                    crate::escape_html(&e.to_string())
                );
                let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(err.into_bytes()));
                return;
            }
        };
        let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(body.into_bytes()));
        let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(Vec::from(
            "\n</body>\n</html>\n",
        )));
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(body)
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        })
}

/// Stream an SSR response from a template string (parses then delegates to nodes variant).
pub async fn stream_ssr_response(
    template: String,
    ctx: TemplateContext,
    doc: SsrDocument<'static>,
) -> Response {
    let parsed = tokio::task::spawn_blocking(move || {
        crepuscularity_core::ast_cache::parse_content(&template).map_err(|e| e.to_string())
    })
    .await
    .unwrap_or_else(|e| Err(format!("spawn_blocking panicked: {e}")));
    let nodes = match parsed {
        Ok(n) => n,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::from(format!("Template parse error: {e}")))
                .unwrap();
        }
    };
    stream_ssr_response_with_nodes(nodes, ctx, doc).await
}

/// Convenience wrapper that accepts a `&'static str` template and renders with an empty context.
pub async fn stream_static_template(template: &'static str, title: &'static str) -> Response {
    let ctx = TemplateContext::new();
    let doc = SsrDocument {
        title,
        ..Default::default()
    };
    stream_ssr_response(template.to_string(), ctx, doc).await
}
