//! JSX / HTML tag syntax parser.
//!
//! Activated automatically when `parse_template()` detects JSX mode (first content
//! line starts with `<`). Produces the same `Node`/`Element` AST as the indentation
//! parser, so every backend — GPUI, web, webext — works unchanged.

pub(crate) mod attrs;
pub(crate) mod builders;
mod tags;
mod template;
pub(crate) mod text;

use super::RawParseError;
use crate::ast::Node;

pub(crate) use attrs::{jsx_parse_attrs, JsxAttr, JsxAttrValue};
use template::parse_jsx_template as parse_jsx_template_inner;
pub(crate) use text::{jsx_close, jsx_err};

/// Maximum nesting depth for JSX tag recursion.
/// Prevents stack overflow on pathologically deep input.
pub(super) const MAX_JSX_DEPTH: usize = 256;

pub(crate) fn parse_jsx_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    parse_jsx_template_inner(src)
}
