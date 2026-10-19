//! File-backed and `include_str!` templates — same ergonomics as the TUI `Template` type.
//!
//! Parsed AST nodes are cached until the template source or component name changes.
//! Variable updates (`set`) skip re-parse and only re-layout + paint.

use std::path::{Path, PathBuf};

use crepuscularity_core::ast::Node;
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::parser::{parse_component_file, parse_template_with_path};

use crate::document::EmbeddedDocument;
use crate::framebuffer::Framebuffer;
use crate::render::render_parsed_nodes_to_framebuffer;
use crate::screen::ScreenSize;

pub struct Template {
    path: PathBuf,
    source: String,
    ctx: TemplateContext,
    screen: ScreenSize,
    component: Option<String>,
    document: Option<EmbeddedDocument>,
    parse_cache: Option<Vec<Node>>,
    source_generation: u64,
    cache_generation: u64,
}

impl Template {
    pub fn from_source(source: impl Into<String>, screen: ScreenSize) -> Self {
        Self {
            path: PathBuf::new(),
            source: source.into(),
            ctx: TemplateContext::new(),
            screen,
            component: None,
            document: None,
            parse_cache: None,
            source_generation: 0,
            cache_generation: u64::MAX,
        }
    }

    pub fn from_source_with_path(
        source: impl Into<String>,
        path: impl AsRef<Path>,
        screen: ScreenSize,
    ) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut ctx = TemplateContext::new();
        ctx.base_dir = path.parent().map(Path::to_path_buf);
        Self {
            path,
            source: source.into(),
            ctx,
            screen,
            component: None,
            document: None,
            parse_cache: None,
            source_generation: 0,
            cache_generation: u64::MAX,
        }
    }

    pub fn from_path(path: impl AsRef<Path>, screen: ScreenSize) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("template error: {:?}: {}", path, e))?;
        let mut ctx = TemplateContext::new();
        ctx.base_dir = path.parent().map(Path::to_path_buf);
        Ok(Self {
            path,
            source,
            ctx,
            screen,
            component: None,
            document: None,
            parse_cache: None,
            source_generation: 0,
            cache_generation: u64::MAX,
        })
    }

    fn invalidate_parse_cache(&mut self) {
        self.source_generation = self.source_generation.wrapping_add(1);
        self.parse_cache = None;
        self.cache_generation = u64::MAX;
    }

    pub fn set_component(&mut self, name: impl Into<String>) -> &mut Self {
        self.component = Some(name.into());
        self.invalidate_parse_cache();
        self.document = None;
        self
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<TemplateValue>) -> &mut Self {
        self.ctx.set(key, value);
        self
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<TemplateValue>) -> Self {
        self.set(key, value);
        self
    }

    pub fn context(&self) -> &TemplateContext {
        &self.ctx
    }

    pub fn context_mut(&mut self) -> &mut TemplateContext {
        &mut self.ctx
    }

    pub fn screen(&self) -> ScreenSize {
        self.screen
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_source(&mut self, source: impl Into<String>) -> &mut Self {
        self.source = source.into();
        self.invalidate_parse_cache();
        self.document = None;
        self
    }

    /// Reload source from disk; parsing is deferred until the next [`Self::draw`].
    ///
    /// Read failures preserve the current source and document. A successful read
    /// invalidates both parsed nodes and the retained document.
    pub fn reload(&mut self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Err("template has no path; reload requires `from_path`".to_string());
        }
        let source = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("template error: {:?}: {}", self.path, e))?;
        self.source = source;
        self.invalidate_parse_cache();
        self.document = None;
        Ok(())
    }

    fn ensure_parsed_nodes(&mut self) -> Result<&[Node], String> {
        let cache_valid =
            self.cache_generation == self.source_generation && self.parse_cache.is_some();
        if !cache_valid {
            let nodes = if let Some(ref name) = self.component {
                let file = parse_component_file(&self.source).map_err(|e| e.to_string())?;
                let component = file
                    .components
                    .get(name)
                    .ok_or_else(|| format!("component not found: {name}"))?;
                let mut nodes = component.nodes.clone();
                for (name, expression) in &component.meta.defaults {
                    nodes.insert(
                        0,
                        Node::LetDecl(crepuscularity_core::ast::LetDecl {
                            name: name.clone(),
                            expr: expression.clone(),
                            is_default: true,
                        }),
                    );
                }
                nodes
            } else {
                parse_template_with_path(&self.source, Some(&self.path))
                    .map_err(|e| e.to_string())?
            };
            self.parse_cache = Some(nodes);
            self.cache_generation = self.source_generation;
        }
        Ok(self
            .parse_cache
            .as_deref()
            .expect("parse cache set when generation matches source"))
    }

    /// Parse (if needed), layout, and paint into `fb`. Returns the retained document (ids, hit targets).
    pub fn draw(&mut self, fb: &mut impl Framebuffer) -> Result<&EmbeddedDocument, String> {
        let screen = self.screen;
        let ctx = self.ctx.clone();
        let nodes = self.ensure_parsed_nodes()?;
        let doc = render_parsed_nodes_to_framebuffer(nodes, &ctx, screen, fb)
            .map_err(|e| e.to_string())?;
        self.document = Some(doc);
        Ok(self.document.as_ref().expect("document set by draw"))
    }

    pub fn document(&self) -> Option<&EmbeddedDocument> {
        self.document.as_ref()
    }
}

pub fn template(path: impl AsRef<Path>, screen: ScreenSize) -> Result<Template, String> {
    Template::from_path(path, screen)
}
