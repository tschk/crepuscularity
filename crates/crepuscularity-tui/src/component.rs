use std::collections::HashMap;
use std::path::Path;

use ratatui::layout::Rect;
use ratatui::Frame;

use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::parser::{
    parse_component_file, parse_template, split_component_body_sections, split_frontmatter_parts,
};
use crepuscularity_core::CrepusError;

use crate::render::render_nodes;

fn eval_default(expr: &str) -> TemplateValue {
    eval_expr(expr, &TemplateContext::default()).unwrap_or(TemplateValue::Null)
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub source: String,
    pub defaults: HashMap<String, TemplateValue>,
}

impl Component {
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            defaults: HashMap::new(),
        }
    }

    pub fn with_default(mut self, key: impl Into<String>, value: impl Into<TemplateValue>) -> Self {
        self.defaults.insert(key.into(), value.into());
        self
    }

    pub fn with_props(&self, props: &[(String, TemplateValue)]) -> TemplateContext {
        let mut ctx = TemplateContext::new();
        ctx.vars = self.defaults.clone();
        for (key, value) in props {
            ctx.vars.insert(key.clone(), value.clone());
        }
        ctx
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComponentRegistry {
    pub components: HashMap<String, Component>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, source: &str) {
        self.components.insert(
            name.to_string(),
            Component {
                name: name.to_string(),
                source: source.to_string(),
                defaults: HashMap::new(),
            },
        );
    }

    pub fn register_file(&mut self, path: &Path) -> Result<(), CrepusError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CrepusError::render(format!("component file error: {:?}: {}", path, e)))?;

        let file = parse_component_file(&content)?;

        let (_, body, _) = split_frontmatter_parts(&content);
        let sections = split_component_body_sections(body);
        let mut section_sources: HashMap<String, String> = sections
            .into_iter()
            .map(|(name, source, _)| (name, source))
            .collect();

        for (name, def) in file.components {
            let source = section_sources.remove(&name).unwrap_or_default();
            let defaults: HashMap<String, TemplateValue> = def
                .meta
                .defaults
                .into_iter()
                .map(|(k, expr)| (k, eval_default(&expr)))
                .collect();
            self.components.insert(
                name.clone(),
                Component {
                    name,
                    source,
                    defaults,
                },
            );
        }

        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Component> {
        self.components.get(name)
    }

    pub fn render(
        &self,
        name: &str,
        props: &[(String, TemplateValue)],
        ctx: &TemplateContext,
        frame: &mut Frame,
        area: Rect,
    ) -> Result<(), CrepusError> {
        let component = self
            .get(name)
            .ok_or_else(|| CrepusError::render(format!("component not found: {name}")))?;

        let mut child_ctx = ctx.clone();
        for (key, value) in props {
            child_ctx.vars.insert(key.clone(), value.clone());
        }
        for (key, value) in &component.defaults {
            if !child_ctx.vars.contains_key(key) {
                child_ctx.vars.insert(key.clone(), value.clone());
            }
        }

        let nodes = parse_template(&component.source)?;
        render_nodes(&nodes, &child_ctx, frame, area)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateKey(String);

impl StateKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StateKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StateKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&StateKey> for StateKey {
    fn from(s: &StateKey) -> Self {
        s.clone()
    }
}

impl AsRef<str> for StateKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComponentState {
    vars: HashMap<String, TemplateValue>,
}

impl ComponentState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_state(&self, key: &StateKey) -> Option<&TemplateValue> {
        self.vars.get(key.as_str())
    }

    pub fn set_state(&mut self, key: impl Into<StateKey>, value: impl Into<TemplateValue>) {
        self.vars.insert(key.into().0, value.into());
    }

    pub fn update_state<F>(&mut self, key: &StateKey, f: F) -> &TemplateValue
    where
        F: FnOnce(&TemplateValue) -> TemplateValue,
    {
        let entry = self
            .vars
            .entry(key.0.clone())
            .or_insert(TemplateValue::Null);
        let new_val = f(entry);
        *entry = new_val;
        entry
    }
}
