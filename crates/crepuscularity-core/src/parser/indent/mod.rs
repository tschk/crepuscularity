//! Indentation-based template syntax parser.

mod attrs;
mod control_flow;
mod element;
mod include;
mod lines;

pub(crate) use control_flow::{parse_nodes, try_parse_let_decl};
pub(crate) use element::{parse_text_template, split_class_tokens};
pub use element::{parse_when_attribute_suffix, unescape_crepus_text_literal};
pub(crate) use lines::collect_lines;
#[cfg(test)]
pub(crate) use lines::strip_structural_indent;
