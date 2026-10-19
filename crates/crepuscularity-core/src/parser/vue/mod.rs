//! Vue single-file-component frontend.
//!
//! Compiles the `<template>` block of a `.vue` file into the same `Node`/`Element`
//! AST the indentation and JSX frontends produce, so every backend — web, TUI,
//! native View IR, LVGL — consumes Vue markup unchanged. `<script>` blocks are
//! never executed; `<style>` blocks are preserved for a caller to emit.

pub(crate) mod builders;
mod sfc;
pub(crate) mod template;

use super::RawParseError;
use crate::ast::Node;

pub use sfc::{VueScriptBlock, VueSfc, VueSfcMetadata, VueStyleBlock};

/// Split a `.vue` source file into its `<template>`, `<script>` and `<style>` blocks.
pub fn parse_vue_sfc(src: &str) -> Result<VueSfc, crate::error::CrepusError> {
    sfc::split_sfc(src).map_err(|e| crate::error::CrepusError::parse(e.message))
}

/// Split an SFC and retain generic declarations in the same order as `scripts`.
/// `None` means absent; `Some("")` preserves an explicitly empty declaration.
pub fn parse_vue_sfc_with_generics(
    src: &str,
) -> Result<(VueSfc, Vec<Option<String>>), crate::error::CrepusError> {
    parse_vue_sfc_with_metadata(src).map(|(sfc, metadata)| (sfc, metadata.script_generics))
}

/// Split an SFC with compiler metadata aligned to its script and style blocks.
pub fn parse_vue_sfc_with_metadata(
    src: &str,
) -> Result<(VueSfc, VueSfcMetadata), crate::error::CrepusError> {
    sfc::split_sfc_with_template_offset(src)
        .map(|(sfc, _, metadata)| (sfc, metadata))
        .map_err(|e| crate::error::CrepusError::parse(e.message))
}

pub(crate) fn parse_vue_file(src: &str) -> Result<Vec<Node>, RawParseError> {
    parse_vue_file_with_mode(src, false)
}

fn parse_vue_file_with_mode(src: &str, native_props: bool) -> Result<Vec<Node>, RawParseError> {
    let (sfc, offset, _) = sfc::split_sfc_with_template_offset(src)?;
    match sfc.template {
        Some(body) => (if native_props {
            parse_vue_template_with_mode(&body, true)
        } else {
            parse_vue_template(&body)
        })
        .map_err(|mut error| {
            error.byte_offset = error.byte_offset.map(|byte| byte.saturating_add(offset));
            error
        }),
        None => Ok(Vec::new()),
    }
}

/// Parse a bare Vue template fragment (no SFC blocks).
pub(crate) fn parse_vue_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    parse_vue_template_with_mode(src, false)
}

fn parse_vue_template_with_mode(src: &str, native_props: bool) -> Result<Vec<Node>, RawParseError> {
    let (nodes, rest) = if native_props {
        template::parse_vue_nodes_with_mode(src, src, 0, true)?
    } else {
        template::parse_vue_nodes(src, src, 0)?
    };
    let rest = rest.trim();
    if !rest.is_empty() {
        return Err(RawParseError {
            message: format!(
                "unexpected trailing markup in Vue template: {}",
                super::diagnostic_prefix(rest, 40)
            ),
            byte_offset: Some(super::subslice_byte_offset(src, rest)),
        });
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests;

/// Native Vue module parsing retains class/style source provenance instead of utility classes.
pub fn parse_vue_native_file(src: &str) -> Result<Vec<Node>, crate::error::CrepusError> {
    parse_vue_file_with_mode(src, true).map_err(|e| crate::error::CrepusError::parse(e.message))
}

#[cfg(test)]
mod native_props_tests {
    use super::*;
    use crate::ast::{BindingOrigin, Node};
    #[test]
    fn native_mode_preserves_file_error_offsets_and_missing_template_policy() {
        let source = "<script setup>const x=1;</script><template><div></span></template>";
        let old = parse_vue_file(source).expect_err("invalid close");
        let native = parse_vue_file_with_mode(source, true).expect_err("invalid close");
        assert_eq!(old.message, native.message);
        assert_eq!(old.byte_offset, native.byte_offset);
        assert!(parse_vue_native_file("<script setup>const x=1;</script>")
            .expect("no template")
            .is_empty());
    }
    #[test]
    fn literal_class_uses_html_space_condensing_and_ecmascript_edge_trimming() {
        for (input, expected) in [
            (" a  b ", "a b"),
            ("\ta\t\tb\t", "a b"),
            ("\ra\r\rb\r", "a b"),
            ("\na\n\nb\n", "a b"),
            ("\u{c}a\u{c}\u{c}b\u{c}", "a b"),
            ("\u{b}a\u{b}\u{b}b\u{b}", "a\u{b}\u{b}b"),
            ("\u{85}a\u{85}\u{85}b\u{85}", "\u{85}a\u{85}\u{85}b\u{85}"),
            ("\u{a0}a\u{a0}\u{a0}b\u{a0}", "a\u{a0}\u{a0}b"),
            ("\u{feff}a\u{feff}\u{feff}b\u{feff}", "a\u{feff}\u{feff}b"),
            ("\u{2028}a\u{2028}\u{2028}b\u{2028}", "a\u{2028}\u{2028}b"),
        ] {
            let source = format!("<template><Child class=\"{input}\"/></template>");
            let nodes = parse_vue_native_file(&source).expect("literal class");
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            assert_eq!(
                element.bindings[0].value,
                crate::parser::quote_expression_string(expected),
                "input {input:?}"
            );
        }
    }
    #[test]
    fn native_mode_preserves_class_style_provenance() {
        let source = r#"<template><Child class="{classes}" style="color:red"/><Child :class="classes" :style="'color:red'"/></template>"#;
        let nodes = parse_vue_native_file(source).expect("native template");
        let Node::Element(first) = &nodes[0] else {
            panic!("first element")
        };
        let Node::Element(second) = &nodes[1] else {
            panic!("second element")
        };
        assert_eq!(first.bindings[0].value, "\"{classes}\"");
        assert_eq!(first.bindings[0].origin, BindingOrigin::Literal);
        assert_eq!(first.bindings[1].origin, BindingOrigin::Literal);
        assert_eq!(second.bindings[0].value, "classes");
        assert_eq!(second.bindings[1].origin, BindingOrigin::Expression);
        assert!(first.classes.is_empty());
    }
    #[test]
    fn default_mode_keeps_utility_class_translation_and_style_guard() {
        let nodes = parse_vue_template(r#"<Child class=" a  b " :class="classes"/>"#)
            .expect("utility template");
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert_eq!(element.classes, ["a", "b", "{classes}"]);
        assert!(element.bindings.is_empty());
        assert!(parse_vue_template(r#"<Child :style="styles"/>"#).is_err());
    }
}

#[cfg(test)]
mod native_model_tests {
    use super::*;
    #[test]
    fn native_model_modifiers_are_canonical_and_do_not_change_shared_view_models() {
        let source = "<template><input v-model.trim.lazy.number=\"value\" /></template>";
        let nodes = parse_vue_native_file(source).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert_eq!(element.bindings[0].prop, "bind.lazy.number.trim");
        assert_eq!(element.bindings[0].value, "value");
        assert!(parse_vue_file(source).is_err());
        for directive in ["v-model.unknown", "v-model.trim.trim", "v-model."] {
            assert!(parse_vue_native_file(&format!(
                "<template><input {directive}=\"value\" /></template>"
            ))
            .is_err());
        }
    }
}

#[cfg(test)]
mod native_model_listener_order_tests {
    use super::*;
    #[test]
    fn native_model_listener_order_preserves_attribute_position() {
        for (attributes, expected) in [
            (r#"v-model="value" @update:modelValue="observe""#, true),
            (r#"@update:modelValue="observe" v-model="value""#, false),
        ] {
            let source = format!("<template><Child {attributes}/></template>");
            let nodes = parse_vue_native_file(&source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            assert_eq!(
                element.event_handlers[0].model_before_listener,
                Some(expected)
            );
            let nodes = parse_vue_file(&source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            assert_eq!(element.event_handlers[0].model_before_listener, None);
        }
    }
}

#[cfg(test)]
mod named_model_tests {
    use super::*;
    #[test]
    fn named_models_canonicalize_arguments_and_track_the_matching_listener_order() {
        let source = r#"<template><Child v-model:title="title" @update:count="before" v-model:count="count" v-model:first-name="name" @update:firstName="after"/></template>"#;
        let nodes = parse_vue_native_file(source).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert_eq!(
            element
                .bindings
                .iter()
                .map(|binding| binding.prop.as_str())
                .collect::<Vec<_>>(),
            ["bind:title", "bind:count", "bind:firstName"]
        );
        assert_eq!(element.event_handlers[0].model_before_listener, Some(false));
        assert_eq!(element.event_handlers[1].model_before_listener, Some(true));
        assert!(parse_vue_file(source).is_err());
    }
    #[test]
    fn dynamic_and_malformed_model_arguments_reject() {
        for arg in ["[key]", "", "a--b", "1name", "name:other", "foo-$bar"] {
            assert!(
                parse_vue_native_file(&format!(
                    r#"<template><Child v-model:{arg}="value"/></template>"#
                ))
                .is_err(),
                "{arg}"
            );
        }
    }
}
