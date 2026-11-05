//! Templates baked at compile time, rendered without `std`.
//!
//! `embedded_document!("ui.crepus")` (feature `macros`) parses the template on the build host
//! and expands to a `&'static` [`CompiledTemplate`]. Firmware then evaluates variables, lays out,
//! and paints with only `core` + `alloc`; no parser runs on the device.
//!
//! The macro accepts elements, static and conditional classes, static ids, text with
//! `{variable}` interpolation, button `@click` names, and `if`/`else` on `name` or `!name`.
//! Anything else is a compile error rather than a silent drop.
//! Rendering matches the `std` renderer for that subset, including the embedded target
//! variables (`is_embedded`, `screen_width`, …).

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use crate::document::{container, text_node, EmbeddedDocument, EmbeddedNode, EmbeddedStyle};
use crate::framebuffer::Framebuffer;
use crate::layout::layout_document;
use crate::paint::paint_document;
use crate::screen::ScreenSize;

/// Root of a compiled template.
#[derive(Debug)]
pub struct CompiledTemplate {
    pub nodes: &'static [CNode],
}

#[derive(Debug)]
pub enum CNode {
    Element(CElement),
    Text(&'static [CText]),
    If {
        cond: Cond,
        then: &'static [CNode],
        otherwise: &'static [CNode],
    },
}

#[derive(Debug)]
pub struct CElement {
    pub tag: &'static str,
    pub id: Option<&'static str>,
    pub classes: &'static [&'static str],
    pub conditional_classes: &'static [(Cond, &'static str)],
    pub on_click: Option<&'static str>,
    pub children: &'static [CNode],
}

#[derive(Debug)]
pub enum CText {
    Lit(&'static str),
    Var(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub enum Cond {
    Var(&'static str),
    Not(&'static str),
}

/// A runtime variable value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value<'a> {
    Str(&'a str),
    Int(i64),
    Bool(bool),
}

impl Value<'_> {
    fn truthy(self) -> bool {
        match self {
            Value::Str(s) => !s.is_empty(),
            Value::Int(n) => n != 0,
            Value::Bool(b) => b,
        }
    }

    fn push_to(self, out: &mut String) {
        match self {
            Value::Str(s) => out.push_str(s),
            Value::Int(n) => out.push_str(&format!("{n}")),
            Value::Bool(b) => out.push_str(if b { "true" } else { "false" }),
        }
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(value: &'a str) -> Self {
        Value::Str(value)
    }
}

impl From<i64> for Value<'_> {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value<'_> {
    fn from(value: i32) -> Self {
        Value::Int(value.into())
    }
}

impl From<bool> for Value<'_> {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

/// Variable lookup for [`CompiledTemplate::render`]. Missing names read as empty and falsy.
pub trait Vars {
    fn get(&self, name: &str) -> Option<Value<'_>>;
}

impl Vars for () {
    fn get(&self, _: &str) -> Option<Value<'_>> {
        None
    }
}

impl Vars for [(&str, Value<'_>)] {
    fn get(&self, name: &str) -> Option<Value<'_>> {
        self.iter().find(|(key, _)| *key == name).map(|(_, v)| *v)
    }
}

impl<const N: usize> Vars for [(&str, Value<'_>); N] {
    fn get(&self, name: &str) -> Option<Value<'_>> {
        Vars::get(self.as_slice(), name)
    }
}

struct Scope<'v, V: ?Sized> {
    vars: &'v V,
    screen: ScreenSize,
}

impl<V: Vars + ?Sized> Scope<'_, V> {
    // Target variables override caller values, as in `render::with_embedded_target`.
    fn get(&self, name: &str) -> Option<Value<'_>> {
        Some(match name {
            "crepus_target" => Value::Str("embedded"),
            "is_embedded" => Value::Bool(true),
            "is_tui" | "is_web" | "is_gui" => Value::Bool(false),
            "screen_width" => Value::Int(self.screen.width.into()),
            "screen_height" => Value::Int(self.screen.height.into()),
            _ => return self.vars.get(name),
        })
    }

    fn holds(&self, cond: Cond) -> bool {
        match cond {
            Cond::Var(name) => self.get(name).is_some_and(Value::truthy),
            Cond::Not(name) => !self.get(name).is_some_and(Value::truthy),
        }
    }

    fn list(&self, nodes: &[CNode]) -> Vec<EmbeddedNode> {
        nodes.iter().map(|node| self.node(node)).collect()
    }

    /// Button label: the first text child, looking through `span`s, as the std renderer does.
    fn label(&self, children: &[CNode]) -> String {
        for child in children {
            match child {
                CNode::Text(parts) => return self.text(parts),
                CNode::Element(el) if el.tag == "span" => return self.label(el.children),
                _ => {}
            }
        }
        String::new()
    }

    fn text(&self, parts: &[CText]) -> String {
        let mut out = String::new();
        for part in parts {
            match part {
                CText::Lit(s) => out.push_str(s),
                CText::Var(name) => {
                    if let Some(value) = self.get(name) {
                        value.push_to(&mut out);
                    }
                }
            }
        }
        out
    }

    fn node(&self, node: &CNode) -> EmbeddedNode {
        match node {
            CNode::Text(parts) => text_node(self.text(parts)),
            CNode::If {
                cond,
                then,
                otherwise,
            } => {
                let body = if self.holds(*cond) { then } else { otherwise };
                container("if", EmbeddedStyle::default(), self.list(body), None, None)
            }
            CNode::Element(el) => self.element(el),
        }
    }

    fn element(&self, el: &CElement) -> EmbeddedNode {
        let mut classes: Vec<&str> = el.classes.to_vec();
        for (cond, class) in el.conditional_classes {
            if self.holds(*cond) {
                classes.push(class);
            }
        }
        let mut style = EmbeddedStyle::default();
        crate::tailwind_apply::apply_classes(&classes, &mut style, None);
        let id = el.id.map(String::from);
        if el.tag == "button" {
            let mut n = text_node(self.label(el.children));
            n.id = id;
            n.tag = "button".into();
            n.style = style;
            n.on_click = el.on_click.map(String::from);
            return n;
        }
        if el.tag == "span" {
            if let [CNode::Text(parts)] = el.children {
                let mut n = text_node(self.text(parts));
                n.id = id;
                n.tag = el.tag.into();
                n.style = style;
                return n;
            }
        }
        container(el.tag, style, self.list(el.children), id, None)
    }
}

impl CompiledTemplate {
    /// Evaluate variables into an unlaid-out document.
    pub fn document<V: Vars + ?Sized>(&self, vars: &V, screen: ScreenSize) -> EmbeddedDocument {
        EmbeddedDocument::new(Scope { vars, screen }.list(self.nodes), screen)
    }

    /// Evaluate, lay out, and paint into `fb`. The returned document supports hit-testing.
    pub fn render<V: Vars + ?Sized>(
        &self,
        vars: &V,
        screen: ScreenSize,
        fb: &mut impl Framebuffer,
    ) -> EmbeddedDocument {
        let mut doc = self.document(vars, screen);
        layout_document(&mut doc);
        paint_document(fb, &doc);
        doc
    }
}
