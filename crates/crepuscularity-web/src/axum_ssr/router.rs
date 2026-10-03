//! Multi-route SSR router: maps URL paths to .crepus templates.
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{render_ssr_document_with_nodes, SsrDocument};
use axum::{
    extract::{Path, State},
    response::Html,
    routing::get,
    Router,
};
use crepuscularity_core::ast::Node;
use crepuscularity_core::TemplateContext;

/// A single route entry pointing to a pre-parsed template.
///
/// The `nodes` field holds the pre-parsed AST so templates are not re-parsed on every request.
pub struct RouteEntry {
    /// Page title for the HTML document.
    pub title: String,
    /// Pre-parsed template AST, cached at startup to avoid re-parsing on every request.
    pub nodes: Arc<Vec<Node>>,
}

/// Builds an Axum `Router` from a declarative map of URL paths → `.crepus` templates.
///
/// Templates are pre-parsed at registration time (in [`SsrRouter::route`]) so that parse errors
/// surface immediately at startup and the parsed AST is reused across all requests.
///
/// # Example
/// ```rust,no_run
/// use crepuscularity_web::SsrRouter;
///
/// let router = SsrRouter::new()
///     .route("/", r#"div "Home""#, "Home")
///     .route("/about", r#"div "About""#, "About")
///     .into_axum_router();
/// ```
pub struct SsrRouter {
    routes: HashMap<String, RouteEntry>,
}

impl Default for SsrRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl SsrRouter {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Register a URL path with its template source and page title.
    ///
    /// The template is pre-parsed immediately so that syntax errors are caught at startup
    /// rather than at request time. The parsed AST is stored in [`RouteEntry::nodes`] and
    /// shared across all requests via `Arc`.
    pub fn route(
        mut self,
        path: &str,
        template: impl AsRef<str>,
        title: impl Into<String>,
    ) -> Self {
        let template = template.as_ref();
        let title = title.into();
        let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap_or_else(|e| {
            panic!("SsrRouter::route: failed to parse template for {path}: {e}")
        });
        self.routes
            .insert(path.to_string(), RouteEntry { title, nodes });
        self
    }

    /// Build an Axum [`Router`] from the registered routes.
    pub fn into_axum_router(self) -> Router {
        let routes = Arc::new(self.routes);
        Router::new()
            .route("/{*path}", get(handle_route))
            .route("/", get(handle_root))
            .with_state(routes)
    }
}

async fn handle_root(State(routes): State<Arc<HashMap<String, RouteEntry>>>) -> Html<String> {
    render_entry(&routes, "/").await
}

async fn handle_route(
    State(routes): State<Arc<HashMap<String, RouteEntry>>>,
    Path(path): Path<String>,
) -> Html<String> {
    let path = format!("/{path}");
    render_entry(&routes, &path).await
}

/// Look up the route entry and render the template using pre-parsed AST nodes.
async fn render_entry(routes: &HashMap<String, RouteEntry>, path: &str) -> Html<String> {
    let entry = match routes.get(path).or_else(|| routes.get("/")) {
        Some(e) => e,
        None => return Html("<h1>404 Not Found</h1>".to_string()),
    };

    let nodes = Arc::clone(&entry.nodes);
    let title = entry.title.clone();

    // Render on a blocking thread using the pre-parsed AST — no re-parsing needed.
    let result = tokio::task::spawn_blocking(move || {
        let ctx = TemplateContext::new();
        let counter = Cell::new(0u32);
        let mut bind = crate::BindMap::new();
        let doc = SsrDocument {
            title: &title,
            ..Default::default()
        };
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true)
    })
    .await;

    match result {
        Ok(Ok(h)) => Html(h),
        Ok(Err(e)) => {
            tracing::error!("SSR render error: {}", e);
            Html("<pre style='color:red'>Internal Server Error</pre>".to_string())
        }
        Err(e) => {
            tracing::error!("SSR render task panicked: {}", e);
            Html("<pre style='color:red'>Internal Server Error</pre>".to_string())
        }
    }
}
