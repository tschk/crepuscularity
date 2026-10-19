//! Streaming SSR: renders head shell synchronously, flushes, then sends body chunks.
//!
//! The HTML5 document shell (`<!DOCTYPE html>` + `<head>` section) is rendered and flushed
//! immediately so the browser can start loading external resources (fonts, CSS, etc.)
//! while the body is still rendering. The rendered body content follows as a second chunk.
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

/// Render the HTML5 document head synchronously and return it as bytes.
/// The head includes `<!DOCTYPE html>` + `<html>` + `<head>... </head>`.
/// Caller must append `<body>` + body content + `</body></html>`.
fn render_document_head(doc: &SsrDocument<'_>) -> String {
    let body_class = doc
        .body_class
        .map(|c| format!(r#" class="{}""#, crate::escape_html(c)))
        .unwrap_or_default();
    let title_esc = doc
        .title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    format!(
        "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n  <meta charset=\"utf-8\">\n  \
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  \
         <title>{}</title>\n</head>\n<body{}>\n",
        crate::escape_html(doc.lang),
        title_esc,
        body_class
    )
}

/// Stream an SSR response with true head-first streaming.
///
/// Renders the HTML5 document head and sends it immediately as the first chunk.
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

    // Build head shell string (moved into the closure for sending)
    let head_html = render_document_head(&doc);

    // Spawn body rendering on blocking thread
    tokio::task::spawn_blocking(move || {
        // ponytail: flush head first so browser starts loading resources early.
        // If the channel is full, body render will wait — head is already in flight.
        let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(
            head_html.as_bytes().to_vec(),
        ));

        let counter = Cell::new(0u32);
        let mut bind = BindMap::new();
        let render_result =
            render_nodes_ssr(&nodes, &ctx, &counter, &mut bind, true).and_then(|mut html| {
                crate::ssr::append_hydration_payload(&mut html, &ctx, &bind)?;
                Ok(html)
            });
        let body: String = match render_result {
            Ok(h) => h,
            Err(e) => {
                let err = format!(
                    "<pre style='color:red'>Render error: {}</pre>\n</body>\n</html>\n",
                    e
                );
                let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(err.into_bytes()));
                return;
            }
        };
        let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(body.into_bytes()));
        let _ = tx.blocking_send(Result::<Vec<u8>, std::io::Error>::Ok(Vec::from(
            "</body>\n</html>\n",
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
    let nodes: Arc<Vec<Node>> = match crepuscularity_core::ast_cache::parse_content(&template) {
        Ok(n) => n,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
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
