//! Default MV3 UI assets for `crepus webext build`, embedded relative to this crate.

pub const POPUP_HTML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/popup.html"));
pub const POPUP_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/popup.js"));
pub const OPTIONS_JS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/options.js"));
pub const POPUP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/popup.css"));
pub const BACKGROUND_JS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/background.js"));
pub const CONTENT_JS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/content.js"));
pub const CONTENT_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/content.css"));
pub const DEV_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dev.js"));
pub const BROWSER_SHIM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/browser-shim.js"
));
pub const RUNTIME_ADAPTER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/runtime-as-adapter.js"
));
pub static UNOCSS_JS: &[u8] = crepuscularity_web::UNOCSS_JS;

#[cfg(test)]
mod tests {
    use super::{BACKGROUND_JS, CONTENT_JS, DEV_JS};

    #[test]
    fn background_host_is_mv3_service_worker_safe() {
        assert!(BACKGROUND_JS.contains("import init, * as runtimeModule"));
        assert!(!BACKGROUND_JS.contains("await import("));
        assert!(!BACKGROUND_JS.contains("const runtimeModule = await"));
        assert!(!BACKGROUND_JS.contains("await fetch("));
    }

    #[test]
    fn dev_script_is_csp_safe() {
        // Dev hot reload must not use eval, inline scripts, or remote origins.
        assert!(!DEV_JS.contains("eval("), "dev.js must not use eval");
        assert!(
            !DEV_JS.contains("Function("),
            "dev.js must not use Function constructor"
        );
        assert!(
            DEV_JS.contains("fetch("),
            "dev.js must use fetch for reload-id"
        );
        assert!(
            DEV_JS.contains("location.reload()"),
            "dev.js must trigger reload"
        );
        assert!(
            !DEV_JS.contains("innerHTML"),
            "dev.js must not use innerHTML"
        );
        assert!(
            !DEV_JS.contains("document.write"),
            "dev.js must not use document.write"
        );
    }

    #[test]
    fn content_host_cache_busts_runtime_assets() {
        assert!(CONTENT_JS.contains("?v=${cacheKey}"));
        assert!(CONTENT_JS.contains("cache: \"no-store\""));
    }

    #[test]
    fn content_host_skips_child_frame_wasm_compile_failures() {
        assert!(CONTENT_JS.contains("globalThis.top === globalThis"));
        assert!(CONTENT_JS.contains("if (topFrame) throw error"));
        assert!(CONTENT_JS.contains("error instanceof WebAssembly.CompileError"));
    }

    #[test]
    fn content_sanitize_html_drops_style_and_event_attrs() {
        assert!(CONTENT_JS.contains("function sanitizeHTML(html)"));
        assert!(CONTENT_JS
            .contains("name.startsWith(\"on\") || name === \"style\" || name === \"srcdoc\""));
        assert!(!CONTENT_JS.contains("innerHTML="));
    }

    #[test]
    fn content_host_scans_and_replaces_code_blocks() {
        assert!(CONTENT_JS.contains("extract_widgets"));
        assert!(CONTENT_JS.contains("enhanceCodeBlock"));
        assert!(CONTENT_JS.contains("widgetTextFromPre"));
        assert!(CONTENT_JS.contains("replaceWith"));
        assert!(CONTENT_JS.contains("render_anywhere_parts"));
        assert!(CONTENT_JS.contains("render_frame_doc"));
        assert!(!CONTENT_JS.contains("enhanceMessage"));
        assert!(!CONTENT_JS.contains("throw new Error(\"runtime.content_main is required\")"));
    }
}
