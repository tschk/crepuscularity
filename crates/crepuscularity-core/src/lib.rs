//! Shared parser, evaluator, preprocessing, and analysis primitives for Crepuscularity.
//!
//! Backend crates use this crate as the source of truth for `.crepus` syntax. Keep changes here
//! mirrored into macro-facing code when the same syntax should work at compile time and runtime.

pub mod analysis;
pub mod ast;
pub mod ast_cache;
pub mod build;
pub mod bundle;
pub mod cache;
pub mod context;
pub mod diagnostics;
pub mod error;
pub mod eval;
pub mod include_paths;
pub mod parser;
pub mod preprocess;
pub mod render_entry;
pub mod tailwind;
pub mod text_binding;
pub mod virtual_files;
pub mod watch;

pub use analysis::{analyze_template, classify_node, BindingMap, Fingerprint, Region};
#[cfg(not(target_arch = "wasm32"))]
pub use cache::DriverCache;
pub use context::{TemplateContext, TemplateValue};
pub use diagnostics::{
    diagnose_crepus_source, diagnose_crepus_source_with_path, is_multi_component_file,
    CrepusDiagnostic,
};
pub use error::CrepusError;
pub use include_paths::resolve_include_path;
pub use parser::{
    parse_component_file, parse_template, parse_template_with_path, unescape_crepus_text_literal,
    ComponentDef, ComponentFile, ComponentMeta,
};
pub use preprocess::{
    extract_head_block, google_fonts_head_markup, merge_unique_font_families,
    strip_indent_decorators, IndentDecorators,
};
pub use virtual_files::lookup_virtual_file;
