//! Authored Svelte syntax serialized to JSX without DSL AST normalization.
use super::super::jsx::attrs::{jsx_parse_authored_attrs, JsxAttr, JsxAttrItem, JsxAttrValue};
use super::super::RawParseError;
use super::{extract_top_level_blocks_with_boundary, svelte_err, MAX_SVELTE_DEPTH};
use crate::error::CrepusError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Namespace {
    Html,
    Svg,
    MathMl,
}
impl Namespace {
    fn uri(self) -> &'static str {
        match self {
            Self::Html => "http://www.w3.org/1999/xhtml",
            Self::Svg => "http://www.w3.org/2000/svg",
            Self::MathMl => "http://www.w3.org/1998/Math/MathML",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SvelteSnippet {
    pub name: String,
    pub parameters: String,
    pub template_jsx: String,
}

/// Ordered compile-only intrinsic directive marker. Values remain JSX expressions
/// so downstream semantic rune/prop/each rewrites keep their authored bindings.
#[derive(Debug, Clone)]
pub enum SvelteDirective {
    Action { attribute: String, parameter: bool },
    Attachment { attribute: String },
}

impl SvelteDirective {
    pub fn attribute(&self) -> &str {
        match self {
            Self::Action { attribute, .. } | Self::Attachment { attribute } => attribute,
        }
    }

    /// None denotes an attachment; action parameter presence is not its value.
    pub fn action_parameter(&self) -> Option<bool> {
        match self {
            Self::Action { parameter, .. } => Some(*parameter),
            Self::Attachment { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SvelteModule {
    /// Raw CSS from the explicit style-aware entry; its caller owns validation and emission.
    pub style: Option<String>,
    pub script: Option<String>,
    pub module_script: Option<String>,
    /// Original validated script languages; generated JSX is not language provenance.
    pub script_is_typescript: bool,
    pub module_script_is_typescript: bool,
    pub template_jsx: String,
    /// Generated JSX component name that the native module host must bind to keyed `For`.
    pub key_block_component: Option<String>,
    /// Authored each header/body marker, lowered by the native Oxc module host.
    pub each_block_component: Option<String>,
    /// Authored await headers and branch patterns are validated by the native Oxc host.
    pub await_block_component: Option<String>,
    /// Compile-only lexical fragment marker; never a runtime component.
    pub const_scope_component: Option<String>,
    /// Private JSX attribute marker consumed by the native checked-binding lowering.
    pub checked_binding_attribute: Option<String>,
    /// Private JSX attribute marker consumed by the native details open-binding lowering.
    pub open_binding_attribute: Option<String>,
    /// Private JSX attribute marker retaining source targets for native file-input bindings.
    pub files_binding_attribute: Option<String>,
    pub value_binding_attribute: Option<String>,
    /// Private marker retaining source group targets for native scope analysis.
    pub group_binding_attribute: Option<String>,
    /// Private attribute carrying the raw first literal of a textarea value.
    pub textarea_value_attribute: Option<String>,
    pub component_binding_attribute: Option<String>,
    /// Lexical authored element namespace, retained before snippet/control relocation.
    pub intrinsic_namespace_attribute: Option<String>,
    pub intrinsic_binding_attribute: Option<String>,
    pub element_component: Option<String>,
    pub element_tag_attribute: Option<String>,
    /// Private marker carrying a dynamic svelte:element xmlns value.
    pub element_namespace_attribute: Option<String>,
    /// Private attribute marker and authored prop name, reused across occurrences.
    pub component_bindings: Vec<(String, String)>,
    pub snippets: Vec<SvelteSnippet>,
    /// Generated declarations scoped to individual component occurrences.
    pub inline_snippet_names: Vec<String>,
    pub render_component: Option<String>,
    pub raw_html_component: Option<String>,
    pub text_component: Option<String>,
    /// Root `<svelte:head>` wrapper marker; native lowering owns its meaning.
    pub head_component: Option<String>,
    /// Self-closing marker for a `<title>` directly inside `<svelte:head>`.
    pub title_component: Option<String>,
    pub attribute_coercion_function: Option<String>,
    pub directives: Vec<SvelteDirective>,
    pub class_directive_helper: Option<String>,
}

/// Parse the admitted authored-Svelte syntax; JavaScript is validated downstream.
pub fn parse_svelte_module(source: &str) -> Result<SvelteModule, CrepusError> {
    parse_svelte_module_with_boundary(source, |src| super::super::match_delimiter(src, '{', '}'))
}

/// Parse authored Svelte with a JavaScript-aware expression boundary reader.
/// The callback receives a slice starting with `{` and returns the matching `}`
/// byte offset in that slice, or `None` for an unterminated expression.
pub fn parse_svelte_module_with_boundary(
    source: &str,
    boundary: fn(&str) -> Option<usize>,
) -> Result<SvelteModule, CrepusError> {
    parse(source, boundary, false).map_err(|error| CrepusError::parse(error.message))
}

/// Parse authored markup for a host that explicitly validates and emits CSS.
/// Ordinary authored-parser entry points continue rejecting styles.
pub fn parse_svelte_module_with_styles_and_boundary(
    source: &str,
    boundary: fn(&str) -> Option<usize>,
) -> Result<SvelteModule, CrepusError> {
    parse(source, boundary, true).map_err(|error| CrepusError::parse(error.message))
}
fn parse(
    source: &str,
    boundary: fn(&str) -> Option<usize>,
    styles: bool,
) -> Result<SvelteModule, RawParseError> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let extracted = extract_top_level_blocks_with_boundary(source, true, Some(boundary))?;
    if extracted.style.is_some() && !styles {
        return Err(svelte_err(
            source,
            source,
            "authored Svelte styles are unsupported until scoped CSS ownership is implemented",
        ));
    }
    if styles {
        validate_style_attributes(source, &extracted.style_attributes)?;
    }
    let mut parser = Markup {
        source: &extracted.blanked,
        original_source: source,
        boundary,
        at: 0,
        key_component: (0..)
            .map(|index| format!("__CrepusKeyBlock{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_key: false,
        each_component: (0..)
            .map(|index| format!("__CrepusEachBlock{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_each: false,
        await_component: (0..)
            .map(|index| format!("__CrepusAwaitBlock{index}"))
            .find(|name| !source.contains(name))
            .ok_or_else(|| svelte_err(source, source, "cannot allocate await marker"))?,
        uses_await: false,
        const_component: None,
        checked_attribute: (0..)
            .map(|index| format!("__crepusCheckedBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_checked: false,
        open_attribute: (0..)
            .map(|index| format!("__crepusOpenBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_open: false,
        files_attribute: (0..)
            .map(|index| format!("__crepusFilesBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_files: false,
        group_attribute: (0..)
            .map(|index| format!("__crepusGroupBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_group: false,
        value_attribute: (0..)
            .map(|index| format!("__crepusValueBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_value: false,
        textarea_value_attribute: (0..)
            .map(|index| format!("__crepusTextareaValue{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_textarea_value: false,
        component_attribute: (0..)
            .map(|index| format!("__crepusComponentBinding{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_component: false,
        intrinsic_namespace_attribute: (0..)
            .map(|index| format!("__crepusIntrinsicNamespace{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_intrinsic_namespace: false,
        intrinsic_binding_attribute: (0..)
            .map(|index| format!("__crepusIntrinsicReference{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_intrinsic_binding: false,
        element_component: (0..)
            .map(|index| format!("__CrepusElement{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        element_tag_attribute: (0..)
            .map(|index| format!("__crepusElementTag{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        element_namespace_attribute: (0..)
            .map(|index| format!("__crepusElementNamespace{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_element: false,
        uses_element_namespace: false,
        math_text: false,
        element_depth: 0,
        namespace: Namespace::Html,
        component_bindings: Vec::new(),
        snippets: Vec::new(),
        inline_snippet_names: Vec::new(),
        component_scopes: Vec::new(),
        render_component: (0..)
            .map(|index| format!("__CrepusRenderSnippet{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_render: false,
        raw_html_component: (0..)
            .map(|index| format!("__CrepusRawHTML{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_raw_html: false,
        text_component: (0..)
            .map(|index| format!("__CrepusText{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_text: false,
        head_component: (0..)
            .map(|index| format!("__CrepusHead{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_head: false,
        title_component: (0..)
            .map(|index| format!("__CrepusTitle{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_title: false,
        in_head: false,
        in_title: false,
        attribute_coercion_function: (0..)
            .map(|index| format!("__crepusAttributeText{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_attribute_coercion: false,
        directives: Vec::new(),
        class_directive_helper: (0..)
            .map(|index| format!("__CrepusClassDirectives{index}"))
            .find(|name| !source.contains(name))
            .unwrap(),
        uses_class_directives: false,
    };
    let (body, _) = parser.nodes(None, None, 0, FragmentKind::Root)?;
    Ok(SvelteModule {
        style: extracted.style,
        script: extracted.script,
        module_script: extracted.module_script,
        script_is_typescript: extracted.script_is_typescript,
        module_script_is_typescript: extracted.module_script_is_typescript,
        template_jsx: format!("<>{body}</>"),
        key_block_component: parser.uses_key.then_some(parser.key_component),
        each_block_component: parser.uses_each.then_some(parser.each_component),
        await_block_component: parser.uses_await.then_some(parser.await_component),
        const_scope_component: parser.const_component,
        checked_binding_attribute: parser.uses_checked.then_some(parser.checked_attribute),
        open_binding_attribute: parser.uses_open.then_some(parser.open_attribute),
        files_binding_attribute: parser.uses_files.then_some(parser.files_attribute),
        group_binding_attribute: parser.uses_group.then_some(parser.group_attribute),
        value_binding_attribute: parser.uses_value.then_some(parser.value_attribute),
        textarea_value_attribute: parser
            .uses_textarea_value
            .then_some(parser.textarea_value_attribute),
        component_binding_attribute: parser.uses_component.then_some(parser.component_attribute),
        intrinsic_namespace_attribute: parser
            .uses_intrinsic_namespace
            .then_some(parser.intrinsic_namespace_attribute),
        intrinsic_binding_attribute: parser
            .uses_intrinsic_binding
            .then_some(parser.intrinsic_binding_attribute),
        element_component: parser.uses_element.then_some(parser.element_component),
        element_tag_attribute: parser.uses_element.then_some(parser.element_tag_attribute),
        element_namespace_attribute: parser
            .uses_element_namespace
            .then_some(parser.element_namespace_attribute),
        component_bindings: parser.component_bindings,
        snippets: parser.snippets,
        inline_snippet_names: parser.inline_snippet_names,
        render_component: parser.uses_render.then_some(parser.render_component),
        raw_html_component: parser.uses_raw_html.then_some(parser.raw_html_component),
        text_component: parser.uses_text.then_some(parser.text_component),
        head_component: parser.uses_head.then_some(parser.head_component),
        title_component: parser.uses_title.then_some(parser.title_component),
        directives: parser.directives,
        attribute_coercion_function: parser
            .uses_attribute_coercion
            .then_some(parser.attribute_coercion_function),
        class_directive_helper: parser
            .uses_class_directives
            .then_some(parser.class_directive_helper),
    })
}

pub(super) fn validate_script_attributes(
    root: &str,
    at: usize,
    attributes: &[JsxAttr],
) -> Result<(), RawParseError> {
    let mut names = std::collections::HashSet::new();
    for attribute in attributes {
        let valid = names.insert(attribute.key.as_str())
            && match (attribute.key.as_str(), &attribute.value) {
                ("lang", JsxAttrValue::Str(value)) => matches!(value.as_str(), "js" | "ts"),
                ("module", JsxAttrValue::Bool(true)) => true,
                ("context", JsxAttrValue::Str(value)) => value == "module",
                _ => false,
            };
        if !valid {
            return Err(svelte_err(
                root,
                &root[at..],
                "unsupported authored Svelte script attribute (only lang=js/ts and module/context=module are supported)",
            ));
        }
    }
    Ok(())
}

struct InlineSnippet {
    name: String,
    generated_name: String,
    parameters: String,
    body: String,
}

struct ComponentSnippetScope {
    depth: usize,
    snippets: Vec<InlineSnippet>,
    meaningful_body: bool,
}

#[derive(Clone, Copy)]
enum FragmentKind {
    Root,
    Intrinsic,
    If,
    Each,
    EachFallback,
    Key,
    Await,
    Snippet,
    Component,
}
impl FragmentKind {
    fn name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Intrinsic => "intrinsic",
            Self::If => "if",
            Self::Each => "each",
            Self::EachFallback => "each-fallback",
            Self::Key => "key",
            Self::Await => "await",
            Self::Snippet => "snippet",
            Self::Component => "component",
        }
    }
    fn permits_const(self) -> bool {
        !matches!(self, Self::Root | Self::Intrinsic)
    }
}

struct Markup<'a> {
    source: &'a str,
    original_source: &'a str,
    boundary: fn(&str) -> Option<usize>,
    at: usize,
    key_component: String,
    uses_key: bool,
    each_component: String,
    uses_each: bool,
    await_component: String,
    uses_await: bool,
    const_component: Option<String>,
    checked_attribute: String,
    uses_checked: bool,
    open_attribute: String,
    uses_open: bool,
    files_attribute: String,
    uses_files: bool,
    group_attribute: String,
    uses_group: bool,
    value_attribute: String,
    uses_value: bool,
    textarea_value_attribute: String,
    uses_textarea_value: bool,
    component_attribute: String,
    uses_component: bool,
    intrinsic_namespace_attribute: String,
    uses_intrinsic_namespace: bool,
    intrinsic_binding_attribute: String,
    uses_intrinsic_binding: bool,
    element_component: String,
    element_tag_attribute: String,
    element_namespace_attribute: String,
    uses_element_namespace: bool,
    uses_element: bool,
    math_text: bool,
    element_depth: usize,
    namespace: Namespace,
    component_bindings: Vec<(String, String)>,
    snippets: Vec<SvelteSnippet>,
    inline_snippet_names: Vec<String>,
    component_scopes: Vec<ComponentSnippetScope>,
    render_component: String,
    uses_render: bool,
    raw_html_component: String,
    uses_raw_html: bool,
    text_component: String,
    uses_text: bool,
    head_component: String,
    uses_head: bool,
    title_component: String,
    uses_title: bool,
    in_head: bool,
    in_title: bool,
    attribute_coercion_function: String,
    uses_attribute_coercion: bool,
    class_directive_helper: String,
    uses_class_directives: bool,
    directives: Vec<SvelteDirective>,
}
impl Markup<'_> {
    fn mark_body_content(&mut self, depth: usize) {
        if let Some(scope) = self.component_scopes.last_mut() {
            if scope.depth == depth {
                scope.meaningful_body = true;
            }
        }
    }

    fn error(&self, message: impl Into<String>) -> RawParseError {
        svelte_err(self.source, &self.source[self.at..], message)
    }
    fn brace(&mut self) -> Result<String, RawParseError> {
        let rest = &self.source[self.at..];
        let end = (self.boundary)(rest)
            .ok_or_else(|| self.error("unclosed authored Svelte expression"))?;
        let value = rest[1..end].trim().to_string();
        self.at += end + 1;
        Ok(value)
    }
    fn nodes(
        &mut self,
        closing: Option<&str>,
        block: Option<&str>,
        depth: usize,
        kind: FragmentKind,
    ) -> Result<(String, Option<String>), RawParseError> {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.nodes_inner(closing, block, depth, kind)
        })
    }
    fn nodes_inner(
        &mut self,
        closing: Option<&str>,
        block: Option<&str>,
        depth: usize,
        kind: FragmentKind,
    ) -> Result<(String, Option<String>), RawParseError> {
        if depth > MAX_SVELTE_DEPTH {
            return Err(self.error("authored Svelte nesting limit exceeded"));
        }
        let mut out = String::new();
        let mut parts: Vec<(bool, String)> = Vec::new();
        let mut declarations = Vec::new();
        while self.at < self.source.len() {
            let rest = &self.source[self.at..];
            if closing == Some("textarea") {
                if let Some(end) = textarea_close(rest) {
                    self.at += end;
                    self.flush_text(&mut out, &mut parts)?;
                    return self.const_fragment(out, None, declarations, kind);
                }
            }
            if closing == Some("textarea") && !rest.starts_with('{') {
                let end = rest
                    .char_indices()
                    .find_map(|(at, ch)| {
                        (ch == '{' || (ch == '<' && textarea_close(&rest[at..]).is_some()))
                            .then_some(at)
                    })
                    .unwrap_or(rest.len());
                if end > 0 {
                    parts.push((false, rest[..end].to_string()));
                    self.at += end;
                    continue;
                }
            }
            if rest.starts_with("<!--") {
                if self.in_title {
                    return Err(self.error("title can only contain text and expressions"));
                }
                let end = rest
                    .find("-->")
                    .ok_or_else(|| self.error("unclosed markup comment"))?;
                self.at += end + 3;
            } else if rest.starts_with("</") {
                let end = rest
                    .find('>')
                    .ok_or_else(|| self.error("unclosed closing tag"))?;
                let name = rest[2..end].trim();
                if closing != Some(name) {
                    return Err(self.error(format!("unexpected closing tag </{name}>")));
                }
                self.at += end + 1;
                self.flush_text(&mut out, &mut parts)?;
                return self.const_fragment(out, None, declarations, kind);
            } else if rest.starts_with('<') {
                if self.in_title {
                    return Err(self.error("title can only contain text and expressions"));
                }
                self.mark_body_content(depth);
                self.flush_text(&mut out, &mut parts)?;
                let head_allowed = closing.is_none()
                    && block.is_none()
                    && depth == 0
                    && matches!(kind, FragmentKind::Root);
                out.push_str(&self.tag(depth + 1, head_allowed)?);
            } else if rest.starts_with('{') {
                let expression = self.brace()?;
                if self.in_title && expression.starts_with(['#', ':', '/', '@']) {
                    return Err(self.error("title can only contain text and expressions"));
                }
                if closing == Some("textarea") && expression.starts_with(['#', ':', '/', '@']) {
                    return Err(self.error("blocks and directives are not allowed inside textarea"));
                }
                if expression.starts_with(['#', ':', '/', '@']) {
                    self.flush_text(&mut out, &mut parts)?;
                }
                if expression.starts_with('/') || expression.starts_with(':') {
                    let valid = match block {
                        Some("if") => {
                            expression == "/if"
                                || expression == ":else"
                                || expression.starts_with(":else if ")
                        }
                        Some("key") => expression == "/key",
                        Some("each") => matches!(expression.as_str(), "/each" | ":else"),
                        Some("snippet") => expression == "/snippet",
                        Some("await") => {
                            expression == "/await"
                                || expression == ":then"
                                || expression
                                    .strip_prefix(":then")
                                    .is_some_and(|tail| tail.starts_with(char::is_whitespace))
                                || expression == ":catch"
                                || expression
                                    .strip_prefix(":catch")
                                    .is_some_and(|tail| tail.starts_with(char::is_whitespace))
                        }
                        _ => false,
                    };
                    if !valid {
                        return Err(self.error("unexpected authored Svelte block terminator"));
                    }
                    return self.const_fragment(out, Some(expression), declarations, kind);
                }
                if let Some(raw) = expression.strip_prefix("@const") {
                    if !raw.starts_with(char::is_whitespace) || raw.trim().is_empty() {
                        return Err(self.error("const requires a single initialized declaration"));
                    }
                    if !kind.permits_const() {
                        return Err(self.error("const must be an immediate child of a supported block, snippet, or component"));
                    }
                    // A const-only default body still creates implicit children
                    // (SSR evaluates it when requested), and conflicts with a
                    // named children snippet just like an emitting default body.
                    self.mark_body_content(depth);
                    declarations.push(format!("const {}\n;", expression_payload(raw)));
                } else if let Some(header) = expression.strip_prefix("#snippet ") {
                    let local = block.is_none()
                        && self
                            .component_scopes
                            .last()
                            .is_some_and(|scope| scope.depth == depth);
                    let root = depth == 0 && block.is_none();
                    if !root && !local {
                        return Err(self.error("nested authored Svelte snippets are unsupported"));
                    }
                    let open = header
                        .find('(')
                        .ok_or_else(|| self.error("snippet requires name(parameters)"))?;
                    let name = header[..open].trim();
                    let mut chars = name.chars();
                    if !chars
                        .next()
                        .is_some_and(|ch| ch.is_alphabetic() || matches!(ch, '_' | '$'))
                        || !chars.all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '$'))
                        || !header.ends_with(')')
                    {
                        return Err(self.error("invalid authored Svelte snippet header"));
                    }
                    let duplicate = if local {
                        self.component_scopes
                            .last()
                            .unwrap()
                            .snippets
                            .iter()
                            .any(|snippet| snippet.name == name)
                    } else {
                        self.snippets.iter().any(|snippet| snippet.name == name)
                    };
                    if duplicate {
                        return Err(self.error("duplicate authored Svelte snippet"));
                    }
                    let parameters = header[open..].to_string();
                    let previous_element_depth = std::mem::replace(&mut self.element_depth, 0);
                    let (body, stop) =
                        self.nodes(None, Some("snippet"), depth + 1, FragmentKind::Snippet)?;
                    self.element_depth = previous_element_depth;
                    if stop.as_deref() != Some("/snippet") {
                        return Err(self.error("unclosed authored Svelte snippet"));
                    }
                    if local {
                        let generated_name = (0..)
                            .map(|index| format!("__crepusInlineSnippet{index}"))
                            .find(|candidate| {
                                !self.original_source.contains(candidate)
                                    && !self.inline_snippet_names.contains(candidate)
                            })
                            .unwrap();
                        self.inline_snippet_names.push(generated_name.clone());
                        self.component_scopes
                            .last_mut()
                            .unwrap()
                            .snippets
                            .push(InlineSnippet {
                                name: name.to_string(),
                                generated_name,
                                parameters,
                                body,
                            });
                    } else {
                        self.snippets.push(SvelteSnippet {
                            name: name.to_string(),
                            parameters,
                            template_jsx: format!("<>{body}</>"),
                        });
                    }
                } else if let Some(value) = expression.strip_prefix("@html") {
                    if !value.starts_with(char::is_whitespace) || value.trim().is_empty() {
                        return Err(self.error("html requires a nonempty expression"));
                    }
                    self.mark_body_content(depth);
                    self.uses_raw_html = true;
                    out.push_str(&format!(
                        "<{} value={{{}}}/>",
                        self.raw_html_component,
                        expression_payload(value)
                    ));
                } else if let Some(call) = expression.strip_prefix("@render") {
                    if !call.starts_with(char::is_whitespace) || call.trim().is_empty() {
                        return Err(self.error("render requires a call expression"));
                    }
                    self.mark_body_content(depth);
                    self.uses_render = true;
                    out.push_str(&format!(
                        "<{} call={{()=> ({})}}/>",
                        self.render_component,
                        expression_payload(call)
                    ));
                } else if let Some(condition) = expression.strip_prefix("#if ") {
                    self.mark_body_content(depth);
                    out.push_str(&self.if_block(condition, depth + 1)?);
                } else if let Some(header) = expression
                    .strip_prefix("#await")
                    .filter(|tail| tail.starts_with(char::is_whitespace))
                {
                    self.mark_body_content(depth);
                    self.uses_await = true;
                    if header.trim().is_empty() {
                        return Err(self.error("await requires an expression"));
                    }
                    let (initial, mut stop) =
                        self.nodes(None, Some("await"), depth + 1, FragmentKind::Await)?;
                    let mut arms = String::new();
                    let mut saw_then = false;
                    let mut saw_catch = false;
                    while let Some(arm) = stop.as_deref().filter(|arm| *arm != "/await") {
                        let (kind, pattern) = if let Some(pattern) = arm.strip_prefix(":then") {
                            if saw_then || saw_catch {
                                return Err(
                                    self.error("duplicate or out-of-order await then branch")
                                );
                            }
                            saw_then = true;
                            ("then", pattern.trim())
                        } else if let Some(pattern) = arm.strip_prefix(":catch") {
                            if saw_catch {
                                return Err(self.error("duplicate await catch branch"));
                            }
                            saw_catch = true;
                            ("catch", pattern.trim())
                        } else {
                            return Err(self.error("invalid await branch"));
                        };
                        let pattern = serde_json::to_string(pattern).map_err(|error| {
                            self.error(format!("cannot encode await binding: {error}"))
                        })?;
                        let (body, next) =
                            self.nodes(None, Some("await"), depth + 1, FragmentKind::Await)?;
                        arms.push_str(&format!(
                            " {kind}Pattern={{{pattern}}} {kind}={{()=> <>{body}</>}}"
                        ));
                        stop = next;
                    }
                    if stop.as_deref() != Some("/await") {
                        return Err(self.error("unclosed authored Svelte await block"));
                    }
                    let header = serde_json::to_string(header).map_err(|error| {
                        self.error(format!("cannot encode await header: {error}"))
                    })?;
                    out.push_str(&format!(
                        "<{} header={{{header}}} initial={{()=> <>{initial}</>}}{arms}/>",
                        self.await_component
                    ));
                } else if let Some(header) = expression.strip_prefix("#each ") {
                    self.mark_body_content(depth);
                    self.uses_each = true;
                    let (body, stop) =
                        self.nodes(None, Some("each"), depth + 1, FragmentKind::Each)?;
                    let fallback = if stop.as_deref() == Some(":else") {
                        let (fallback, close) =
                            self.nodes(None, Some("each"), depth + 1, FragmentKind::EachFallback)?;
                        if close.as_deref() != Some("/each") {
                            return Err(self.error("unclosed authored Svelte each block"));
                        }
                        format!(" fallback={{()=> <>{fallback}</>}}")
                    } else if stop.as_deref() == Some("/each") {
                        String::new()
                    } else {
                        return Err(self.error("unclosed authored Svelte each block"));
                    };
                    let header = serde_json::to_string(header).map_err(|error| {
                        self.error(format!("cannot encode each header: {error}"))
                    })?;
                    out.push_str(&format!(
                        "<{} header={{{header}}} children={{()=> <>{body}</>}}{fallback}/>",
                        self.each_component
                    ));
                } else if let Some(key) = expression.strip_prefix("#key ") {
                    if key.trim().is_empty() {
                        return Err(self.error("Svelte key requires an expression"));
                    }
                    self.mark_body_content(depth);
                    self.uses_key = true;
                    let (body, stop) =
                        self.nodes(None, Some("key"), depth + 1, FragmentKind::Key)?;
                    if stop.as_deref() != Some("/key") {
                        return Err(self.error("unclosed authored Svelte key block"));
                    }
                    out.push_str(&format!(
                        "<{} each={{()=>[({})]}} children={{()=> <>{body}</>}}/>",
                        self.key_component,
                        expression_payload(key)
                    ));
                } else if expression.starts_with(['#', ':', '/', '@']) {
                    return Err(self.error("unsupported authored Svelte block/raw HTML directive"));
                } else if expression.is_empty() {
                    return Err(self.error("empty authored Svelte expression"));
                } else {
                    self.mark_body_content(depth);
                    parts.push((true, expression_payload(&expression)));
                }
            } else {
                let end = rest.find(['<', '{']).unwrap_or(rest.len());
                let text = &rest[..end];
                if !text.is_empty() {
                    let text = text.to_string();
                    if !text.trim().is_empty() {
                        self.mark_body_content(depth);
                    }
                    parts.push((false, text));
                }
                self.at += end;
            }
        }
        if closing.is_some() || block.is_some() {
            return Err(self.error("unclosed authored Svelte tag or block"));
        }
        self.flush_text(&mut out, &mut parts)?;
        self.const_fragment(out, None, declarations, kind)
    }
    fn flush_text(
        &mut self,
        out: &mut String,
        parts: &mut Vec<(bool, String)>,
    ) -> Result<(), RawParseError> {
        if parts.is_empty() {
            return Ok(());
        }
        let dynamic = parts.iter().any(|(expression, _)| *expression);
        if self.in_title {
            self.uses_title = true;
        } else {
            self.uses_text = true;
        }
        let mut values = Vec::new();
        for (expression, part) in parts.drain(..) {
            if expression {
                values.push(format!("()=> ({part})"));
            } else {
                let text = serde_json::to_string(&part)
                    .expect("text serialization")
                    .replace('<', "\\u003c");
                // Preserve authored entity spelling until native text decoding.
                values.push(if part.contains('&') {
                    format!("{{text:{text}}}")
                } else {
                    text
                });
            }
        }
        let (marker, literal) = if self.in_title {
            (&self.title_component, "")
        } else {
            (&self.text_component, if dynamic { "" } else { " literal" })
        };
        out.push_str(&format!(
            "<{marker}{literal} parts={{[{}]}}/>",
            values.join(",")
        ));
        Ok(())
    }
    fn const_fragment(
        &mut self,
        body: String,
        stop: Option<String>,
        declarations: Vec<String>,
        kind: FragmentKind,
    ) -> Result<(String, Option<String>), RawParseError> {
        if declarations.is_empty() {
            return Ok((body, stop));
        }
        if self.const_component.is_none() {
            self.const_component = Some(
                (0..)
                    .map(|index| format!("__CrepusConstScope{index}"))
                    .find(|name| !self.original_source.contains(name))
                    .ok_or_else(|| self.error("cannot allocate const marker"))?,
            );
        }
        let marker = self
            .const_component
            .as_deref()
            .ok_or_else(|| self.error("missing const marker"))?;
        let conflicts = if matches!(kind, FragmentKind::Component) {
            let names = self
                .component_scopes
                .last()
                .map(|scope| {
                    scope
                        .snippets
                        .iter()
                        .map(|snippet| snippet.name.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if names.is_empty() {
                String::new()
            } else {
                format!(
                    " conflicts={{{}}}",
                    serde_json::to_string(&names).map_err(|error| self
                        .error(format!("cannot encode const fragment bindings: {error}")))?
                )
            }
        } else {
            String::new()
        };
        Ok((
            format!(
                "<{} kind=\"{}\"{conflicts} body={{()=>{{{}return <>{body}</>;}}}}/>",
                marker,
                kind.name(),
                declarations.join("\n")
            ),
            stop,
        ))
    }

    fn if_block(&mut self, condition: &str, depth: usize) -> Result<String, RawParseError> {
        if condition.trim().is_empty() {
            return Err(self.error("Svelte if requires a condition"));
        }
        let (yes, stop) = self.nodes(None, Some("if"), depth, FragmentKind::If)?;
        let no = match stop.as_deref() {
            Some("/if") => "null".to_string(),
            Some(":else") => {
                let (body, stop) = self.nodes(None, Some("if"), depth, FragmentKind::If)?;
                if stop.as_deref() != Some("/if") {
                    return Err(self.error("expected {/if} after {:else}"));
                }
                format!("<>{body}</>")
            }
            Some(stop) if stop.starts_with(":else if ") => self.if_block(&stop[9..], depth + 1)?,
            _ => return Err(self.error("unclosed authored Svelte conditional")),
        };
        // Nested conditional output is an expression, not a JSX child container.
        let no = if no.starts_with('{') && no.ends_with('}') {
            &no[1..no.len() - 1]
        } else {
            &no
        };
        Ok(format!(
            "{{({}) ? <>{yes}</> : {no}}}",
            expression_payload(condition)
        ))
    }
    fn head(&mut self, tail: &str, depth: usize, allowed: bool) -> Result<String, RawParseError> {
        if !allowed {
            return Err(
                self.error("svelte:head must be a root element, not inside an element or block")
            );
        }
        if self.uses_head {
            return Err(self.error("a component can only have one svelte:head element"));
        }
        let (attributes, next, self_closing) =
            jsx_parse_authored_attrs(self.source, tail, self.boundary)?;
        self.at = self.source.len() - next.len();
        if !attributes.is_empty() {
            return Err(self.error("svelte:head cannot have attributes or directives"));
        }
        self.uses_head = true;
        if self_closing {
            return Ok(format!("<{}/>", self.head_component));
        }
        let previous_head = std::mem::replace(&mut self.in_head, true);
        let previous_math_text = std::mem::replace(&mut self.math_text, false);
        let previous_namespace = std::mem::replace(&mut self.namespace, Namespace::Html);
        let (body, _) = self.nodes(Some("svelte:head"), None, depth, FragmentKind::Intrinsic)?;
        self.in_head = previous_head;
        self.namespace = previous_namespace;
        self.math_text = previous_math_text;
        Ok(format!(
            "<{marker}>{body}</{marker}>",
            marker = self.head_component
        ))
    }
    fn title(
        &mut self,
        attributes: &[JsxAttrItem],
        self_closing: bool,
        depth: usize,
    ) -> Result<String, RawParseError> {
        if !attributes.is_empty() {
            return Err(self.error("title cannot have attributes or directives"));
        }
        self.uses_title = true;
        if self_closing {
            return Ok(format!("<{} parts={{[]}}/>", self.title_component));
        }
        let previous = std::mem::replace(&mut self.in_title, true);
        let (body, _) = self.nodes(Some("title"), None, depth, FragmentKind::Intrinsic)?;
        self.in_title = previous;
        Ok(if body.is_empty() {
            format!("<{} parts={{[]}}/>", self.title_component)
        } else {
            body
        })
    }
    fn dynamic_selector(
        &mut self,
        attributes: &mut Vec<JsxAttrItem>,
    ) -> Result<String, RawParseError> {
        let missing = "svelte:element requires a this attribute with a value";
        let index = attributes
            .iter()
            .position(
                |item| matches!(item, JsxAttrItem::Named(attribute) if attribute.key == "this"),
            )
            .ok_or_else(|| self.error(missing))?;
        let JsxAttrItem::Named(attribute) = attributes.remove(index) else {
            unreachable!("position matched a named attribute")
        };
        let mixed = "svelte:element this must be one expression or a plain string literal";
        let expression = match attribute.value {
            JsxAttrValue::Bool(_) => return Err(self.error(missing)),
            JsxAttrValue::Expr(value) => value,
            JsxAttrValue::Str(value) if value.starts_with('{') => {
                if (self.boundary)(&value) != Some(value.len() - 1) {
                    return Err(self.error(mixed));
                }
                value[1..value.len() - 1].to_string()
            }
            JsxAttrValue::Str(value) if value.contains('{') => return Err(self.error(mixed)),
            JsxAttrValue::Str(value) => {
                let literal = super::entities::decode_entities(&value, true);
                return Ok(serde_json::to_string(&literal).expect("literal serialization"));
            }
        };
        let expression = expression_payload(&expression);
        if expression.is_empty() {
            return Err(self.error("empty svelte:element this expression"));
        }
        Ok(expression)
    }
    fn tag(&mut self, depth: usize, head_allowed: bool) -> Result<String, RawParseError> {
        let rest = &self.source[self.at..];
        if rest.starts_with("<>") {
            self.at += 2;
            let (body, _) = self.nodes(Some(""), None, depth, FragmentKind::Intrinsic)?;
            return Ok(format!("<>{body}</>"));
        }
        let tail = &rest[1..];
        let name_end = tail
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '/' | '>'))
            .unwrap_or(tail.len());
        let name = &tail[..name_end];
        if name == "svelte:head" {
            return self.head(&tail[name_end..], depth, head_allowed);
        }
        let dynamic = name == "svelte:element";
        if !dynamic
            && (name.is_empty()
                || (name != "annotation-xml" && !name.split('.').all(valid_name))
                || name.starts_with("svelte:")
                || matches!(name, "slot" | "include" | "script" | "style"))
        {
            return Err(self.error(format!("unsupported authored Svelte tag <{name}>")));
        }
        let (mut attributes, next, self_closing) =
            jsx_parse_authored_attrs(self.source, &tail[name_end..], self.boundary)?;
        self.at = self.source.len() - next.len();
        let selector = if dynamic {
            self.uses_element = true;
            Some(self.dynamic_selector(&mut attributes)?)
        } else {
            None
        };
        let mut xmlns_override = None;
        let mut xmlns_marker = None;
        if dynamic {
            let mut found = None;
            for item in &attributes {
                let (key, value) = match item {
                    JsxAttrItem::Named(attribute) => {
                        let key = attribute
                            .key
                            .strip_prefix('{')
                            .and_then(|key| key.strip_suffix('}'))
                            .map(str::trim)
                            .unwrap_or(&attribute.key);
                        (key, &attribute.value)
                    }
                    JsxAttrItem::Spread(_) => continue,
                };
                if key != "xmlns" {
                    continue;
                }
                if found.is_some() {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                found = Some((item, value));
            }
            if let Some((item, value)) = found {
                let shorthand =
                    matches!(item, JsxAttrItem::Named(attribute) if attribute.key.starts_with('{'));
                match value {
                    _ if shorthand => {
                        xmlns_marker = Some("xmlns".to_string());
                    }
                    JsxAttrValue::Bool(value) => {
                        xmlns_marker = Some(value.to_string());
                    }
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => {
                        xmlns_marker = Some(format!("({}\n)", expression_payload(value)));
                    }
                    JsxAttrValue::Expr(_) => {
                        return Err(self.error("empty authored Svelte attribute expression"))
                    }
                    JsxAttrValue::Str(value) => {
                        match quoted_attribute(
                            value,
                            self.boundary,
                            &self.attribute_coercion_function,
                        )
                        .map_err(|message| self.error(message))?
                        {
                            Some(expression) => {
                                self.uses_attribute_coercion |=
                                    expression.contains(&self.attribute_coercion_function);
                                xmlns_marker = Some(expression);
                            }
                            None => {
                                // Static xmlns replaces the inherited source namespace.
                                let literal = super::entities::decode_entities(value, true);
                                xmlns_override = Some(if literal == Namespace::Svg.uri() {
                                    Namespace::Svg
                                } else if literal == Namespace::MathMl.uri() {
                                    Namespace::MathMl
                                } else {
                                    Namespace::Html
                                });
                            }
                        }
                    }
                }
            }
        }
        if name == "title" && self.in_head && matches!(self.namespace, Namespace::Html) {
            return self.title(&attributes, self_closing, depth);
        }
        let component = name
            .starts_with(|ch: char| ch.is_ascii_uppercase() || ch == '_' || ch == '$')
            || name.contains('.');
        let named: Vec<_> = attributes
            .iter()
            .filter_map(|item| match item {
                JsxAttrItem::Named(attribute) => Some(attribute),
                JsxAttrItem::Spread(_) => None,
            })
            .collect();
        let type_attributes: Vec<_> = named
            .iter()
            .filter(|attribute| attribute.key == "type")
            .collect();
        let checkbox = name == "input"
            && type_attributes.len() == 1
            && matches!(&type_attributes[0].value, JsxAttrValue::Str(value) if value == "checkbox");
        let file_input = name == "input"
            && type_attributes.len() == 1
            && matches!(&type_attributes[0].value, JsxAttrValue::Str(value) if value == "file");
        let group_input = name == "input"
            && type_attributes.len() == 1
            && matches!(&type_attributes[0].value, JsxAttrValue::Str(value) if matches!(value.as_str(), "radio" | "checkbox"));
        let canonical_key = |attribute: &JsxAttr| {
            attribute
                .key
                .strip_prefix('{')
                .and_then(|key| key.strip_suffix('}'))
                .map(str::trim)
                .unwrap_or(&attribute.key)
                .to_string()
        };
        let named_keys: Vec<_> = named
            .iter()
            .map(|attribute| canonical_key(attribute))
            .collect();
        let dynamic_select_multiple = name == "select" && named.iter().any(|attribute| {
            canonical_key(attribute) == "multiple"
                && (attribute.key != "multiple"
                    || !matches!(
                        &attribute.value,
                        JsxAttrValue::Bool(true) | JsxAttrValue::Str(_)
                    )
                    || matches!(&attribute.value, JsxAttrValue::Str(value) if value.contains('{')))
        });
        let has_checked = named
            .iter()
            .any(|attribute| canonical_key(attribute) == "checked");
        let has_open = named
            .iter()
            .any(|attribute| canonical_key(attribute) == "open");
        let has_files = named
            .iter()
            .any(|attribute| canonical_key(attribute) == "files");
        let has_value = named
            .iter()
            .any(|attribute| canonical_key(attribute) == "value");
        let value_input = name == "input"
            && (type_attributes.is_empty()
                || (type_attributes.len() == 1
                    && matches!(&type_attributes[0].value,
                JsxAttrValue::Str(value) if matches!(value.as_str(), "text" | "search" | "tel" | "url" | "email" | "password" | "number" | "range" | "date" | "datetime-local" | "month" | "week" | "time" | "color"))));
        let mut binds_value = false;
        let element_namespace = if !component && !dynamic {
            match name {
                "svg" => Namespace::Svg,
                "g" | "foreignObject" if self.namespace != Namespace::MathMl && !self.math_text => {
                    Namespace::Svg
                }
                "math" if self.namespace != Namespace::Svg => Namespace::MathMl,
                "mi" | "mn" | "mo" | "ms" | "mtext" | "mrow" | "mfrac" | "msqrt" | "mroot"
                | "mstyle" | "merror" | "mpadded" | "mphantom" | "mfenced" | "menclose"
                | "msub" | "msup" | "msubsup" | "munder" | "mover" | "munderover"
                | "mmultiscripts" | "mprescripts" | "mtable" | "mtr" | "mlabeledtr" | "mtd"
                | "maction" | "semantics" | "annotation" | "annotation-xml" | "mspace"
                    if self.element_depth == 0 =>
                {
                    Namespace::MathMl
                }
                "mglyph" | "malignmark" if self.math_text => Namespace::MathMl,
                _ => self.namespace,
            }
        } else {
            xmlns_override.unwrap_or(self.namespace)
        };
        let intrinsic_spread = !component
            && attributes
                .iter()
                .any(|item| matches!(item, JsxAttrItem::Spread(_)));
        if intrinsic_spread
            && (dynamic
                || element_namespace != Namespace::Html
                || name.contains('-')
                || matches!(name, "title")
                || named.iter().any(|attribute| {
                    let key = canonical_key(attribute);
                    (key.contains(':')
                        && !matches!(
                            key.as_str(),
                            "bind:value"
                                | "bind:checked"
                                | "bind:files"
                                | "bind:open"
                                | "bind:group"
                        ))
                        || matches!(key.as_str(), "xmlns" | "is")
                        || key.starts_with("@attach")
                }))
        {
            return Err(self.error(
                "authored intrinsic spread attributes are unsupported on title, dynamic or namespaced/custom elements, and with directives",
            ));
        }
        let details =
            !component && !dynamic && name == "details" && element_namespace == Namespace::Html;
        let mut annotation_html = false;
        if !component
            && !dynamic
            && name == "annotation-xml"
            && element_namespace == Namespace::MathMl
        {
            for item in &attributes {
                let JsxAttrItem::Named(attribute) = item else {
                    return Err(self.error(
                        "annotation-xml spread attributes cannot define its namespace: encoding must be static",
                    ));
                };
                if canonical_key(attribute) != "encoding" {
                    continue;
                }
                match &attribute.value {
                    _ if attribute.key.starts_with('{') => {
                        return Err(self.error(
                            "annotation-xml dynamic encoding cannot define its namespace: encoding must be static",
                        ));
                    }
                    JsxAttrValue::Str(value) if !value.contains('{') => {
                        let literal = super::entities::decode_entities(value, true);
                        annotation_html = literal.eq_ignore_ascii_case("text/html")
                            || literal.eq_ignore_ascii_case("application/xhtml+xml");
                    }
                    JsxAttrValue::Bool(_) => {}
                    _ => {
                        return Err(self.error(
                            "annotation-xml dynamic encoding cannot define its namespace: encoding must be static",
                        ));
                    }
                }
            }
        }
        let tag_out = if dynamic {
            std::borrow::Cow::Owned(self.element_component.clone())
        } else {
            std::borrow::Cow::Borrowed(name)
        };
        let mut out = format!("<{tag_out}");
        if !component {
            self.uses_intrinsic_namespace = true;
            out.push_str(&format!(
                " {}={:?}",
                self.intrinsic_namespace_attribute,
                element_namespace.uri()
            ));
        }
        if let Some(selector) = &selector {
            out.push_str(&format!(" {}={{({selector})}}", self.element_tag_attribute));
        }
        if let Some(expression) = &xmlns_marker {
            self.uses_element_namespace = true;
            out.push_str(&format!(
                " {}={{{expression}}}",
                self.element_namespace_attribute
            ));
        }
        let has_class_directives = named
            .iter()
            .any(|attribute| attribute.key.starts_with("class:"));
        let mut class_base: Option<String> = None;
        let mut class_position = None;
        let mut class_flags: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in attributes {
            let attribute = match item {
                JsxAttrItem::Named(attribute) => attribute,
                JsxAttrItem::Spread(expression) => {
                    out.push_str(&format!(" {{...({expression})}}"));
                    continue;
                }
            };
            let shorthand = attribute.key.starts_with('{');
            let key = if shorthand {
                attribute.key[1..attribute.key.len() - 1].trim().to_string()
            } else {
                attribute.key
            };
            if key.starts_with("use:") || (shorthand && key.starts_with("@attach")) {
                if component {
                    return Err(self.error("actions and attachments require an intrinsic element"));
                }
                let marker = (0usize..)
                    .map(|index| format!("__crepusDirective{index}"))
                    .find(|name| {
                        !self.original_source.contains(name)
                            && !self.directives.iter().any(|directive| match directive {
                                SvelteDirective::Action { attribute, .. }
                                | SvelteDirective::Attachment { attribute } => attribute == name,
                            })
                    })
                    .ok_or_else(|| self.error("directive marker allocation failed"))?;
                if let Some(action) = key.strip_prefix("use:") {
                    if action.is_empty()
                        || !action.split('.').enumerate().all(|(index, part)| {
                            valid_name(part) && (index > 0 || !reserved_name(part))
                        })
                    {
                        return Err(self.error(
                            "unsupported action path; expected an identifier or static member path",
                        ));
                    }
                    if !seen.insert(key.clone()) {
                        return Err(self.error("duplicate authored Svelte action"));
                    }
                    let parameter = match attribute.value {
                        JsxAttrValue::Bool(true) => None,
                        JsxAttrValue::Expr(value) if !value.trim().is_empty() => Some(value),
                        _ => return Err(self.error("action parameter requires an expression")),
                    };
                    let node = (0usize..)
                        .map(|index| format!("__crepusActionNode{index}"))
                        .find(|name| !self.original_source.contains(name))
                        .ok_or_else(|| self.error("action node allocation failed"))?;
                    let value = (0usize..)
                        .map(|index| format!("__crepusActionValue{index}"))
                        .find(|name| !self.original_source.contains(name))
                        .ok_or_else(|| self.error("action value allocation failed"))?;
                    let expression = match &parameter {
                        Some(parameter) => format!(
                            "[({node},{value})=>{action}?.({node},{value}),()=>({parameter}\n)]"
                        ),
                        None => format!("[({node})=>{action}?.({node})]"),
                    };
                    out.push_str(&format!(" {marker}={{{expression}}}"));
                    self.directives.push(SvelteDirective::Action {
                        attribute: marker,
                        parameter: parameter.is_some(),
                    });
                } else {
                    let expression = key
                        .strip_prefix("@attach")
                        .filter(|tail| tail.starts_with(char::is_whitespace))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| self.error("attachment requires an expression"))?;
                    out.push_str(&format!(" {marker}={{()=>({expression}\n)}}"));
                    self.directives
                        .push(SvelteDirective::Attachment { attribute: marker });
                }
                continue;
            }
            if !shorthand && key.starts_with("class:") {
                let directive = &key["class:".len()..];
                if directive.is_empty() {
                    return Err(self.error("class directive requires a name"));
                }
                if component {
                    return Err(self.error("class directives require an intrinsic element"));
                }
                if directive
                    .chars()
                    .any(|ch| matches!(ch, '|' | '{' | '}' | '"' | '\''))
                {
                    return Err(self.error("unsupported class directive name or modifier"));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte class directive"));
                }
                let flag = match attribute.value {
                    JsxAttrValue::Bool(true) if valid_name(directive) && !reserved_name(directive) => {
                        directive.to_string()
                    }
                    JsxAttrValue::Bool(true) => {
                        return Err(self.error(
                            "class directive shorthand requires a variable identifier; use an explicit expression",
                        ))
                    }
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => {
                        format!("({}\n)", expression_payload(&value))
                    }
                    JsxAttrValue::Str(value)
                        if value.starts_with('{')
                            && (self.boundary)(&value) == Some(value.len() - 1)
                            && !value[1..value.len() - 1].trim().is_empty() =>
                    {
                        format!("({}\n)", expression_payload(&value[1..value.len() - 1]))
                    }
                    JsxAttrValue::Str(value) if value.starts_with('{') => {
                        return Err(self.error("class directive requires one non-empty expression"))
                    }
                    JsxAttrValue::Str(_) => {
                        return Err(self.error(
                            "class directive value must be a JavaScript expression enclosed in curly braces",
                        ))
                    }
                    _ => return Err(self.error("empty class directive expression")),
                };
                class_flags.push(format!(
                    "[{}]:{flag}",
                    serde_json::to_string(directive).expect("directive serialization")
                ));
                continue;
            }
            if !component && name == "textarea" && key == "value" {
                let first_literal = match &attribute.value {
                    JsxAttrValue::Str(value) => value.split('{').next().unwrap_or(""),
                    _ => "",
                };
                self.uses_textarea_value = true;
                out.push_str(&format!(
                    " {}={{{}}}",
                    self.textarea_value_attribute,
                    serde_json::to_string(first_literal).expect("literal serialization")
                ));
            }
            if key == "bind:this" {
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let JsxAttrValue::Expr(target) = attribute.value else {
                    return Err(self.error("component bind:this requires an explicit expression"));
                };
                if target.trim().is_empty() {
                    return Err(self.error("empty component bind:this expression"));
                }
                if component {
                    self.uses_component = true;
                    out.push_str(&format!(" {}={{{target}}}", self.component_attribute));
                } else {
                    self.uses_intrinsic_binding = true;
                    // Function-pair targets are sequence expressions. Keep one
                    // uniformly parenthesized operand in the synthetic JSX so
                    // normal scope validation does not reject the comma.
                    out.push_str(&format!(
                        " {}={{({target})}}",
                        self.intrinsic_binding_attribute
                    ));
                }
                continue;
            }
            if component && key.starts_with("bind:") {
                let property = &key[5..];
                if property.is_empty()
                    || (!valid_name(property)
                        && !property
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')))
                {
                    return Err(self.error("unsupported authored component binding property"));
                }
                if named_keys.iter().any(|name| name == property) {
                    return Err(self.error(format!("{property} conflicts with bind:{property}")));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) if valid_name(property) && !reserved_name(property) => property.to_string(),
                    JsxAttrValue::Bool(true) => return Err(self.error("component binding shorthand requires a variable identifier; use an explicit expression")),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => {
                        return Err(
                            self.error("component binding requires an expression or shorthand")
                        )
                    }
                };
                let marker = if let Some((marker, _)) = self
                    .component_bindings
                    .iter()
                    .find(|(_, name)| name == property)
                {
                    marker.clone()
                } else {
                    let marker = (0..)
                        .map(|index| format!("__crepusComponentBindingProp{index}"))
                        .find(|marker| {
                            !self.original_source.contains(marker)
                                && !self
                                    .component_bindings
                                    .iter()
                                    .any(|(used, _)| used == marker)
                        })
                        .unwrap();
                    self.component_bindings
                        .push((marker.clone(), property.to_string()));
                    marker
                };
                out.push_str(&format!(" {marker}={{({target}\n)}}"));
                continue;
            }
            if key == "bind:value" {
                if dynamic_select_multiple {
                    return Err(self.error("multiple must be static when select uses bind:value"));
                }
                if has_value {
                    return Err(self.error("value conflicts with bind:value"));
                }
                if !value_input && name != "textarea" && name != "select" {
                    return Err(self.error(if dynamic {
                        "bind:value cannot target svelte:element: the element type is unknown"
                    } else {
                        "bind:value requires a select, textarea or input with a supported static type"
                    }));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) => "value".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("bind:value requires an expression or shorthand")),
                };
                self.uses_value = true;
                binds_value = true;
                out.push_str(&format!(" {}={{{target}}}", self.value_attribute));
                continue;
            }
            if key == "bind:group" {
                if !group_input {
                    return Err(self.error(
                        "bind:group requires an input with static type=radio or type=checkbox",
                    ));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) => "group".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("bind:group requires an expression or shorthand")),
                };
                self.uses_group = true;
                out.push_str(&format!(" {}={{{target}}}", self.group_attribute));
                continue;
            }
            if key == "bind:files" {
                if has_files {
                    return Err(self.error("files conflicts with bind:files"));
                }
                if !file_input {
                    return Err(self.error(if dynamic {
                        "bind:files cannot target svelte:element: the element type is unknown"
                    } else {
                        "bind:files requires an input with static type=file"
                    }));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) => "files".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("bind:files requires an expression or shorthand")),
                };
                self.uses_files = true;
                out.push_str(&format!(" {}={{{target}}}", self.files_attribute));
                continue;
            }
            if key == "bind:checked" {
                if has_checked {
                    return Err(self.error("checked conflicts with bind:checked"));
                }
                if !checkbox {
                    return Err(self.error(if dynamic {
                        "bind:checked cannot target svelte:element: the element type is unknown"
                    } else {
                        "bind:checked requires an input with static type=checkbox"
                    }));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) => "checked".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("bind:checked requires an expression or shorthand")),
                };
                self.uses_checked = true;
                out.push_str(&format!(" {}={{{target}}}", self.checked_attribute));
                continue;
            }
            if key == "bind:open" {
                if has_open {
                    return Err(self.error("open conflicts with bind:open"));
                }
                if !details {
                    return Err(self.error(if dynamic {
                        "bind:open cannot target svelte:element: the element type is unknown"
                    } else {
                        "bind:open requires a static HTML details element"
                    }));
                }
                if !seen.insert(key.clone()) {
                    return Err(self.error("duplicate authored Svelte attribute"));
                }
                let target = match attribute.value {
                    JsxAttrValue::Bool(true) => "open".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("bind:open requires an expression or shorthand")),
                };
                self.uses_open = true;
                out.push_str(&format!(" {}={{{target}}}", self.open_attribute));
                continue;
            }
            if !valid_name(&key)
                && !key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
            {
                return Err(self.error("unsupported authored Svelte directive/attribute"));
            }
            if !seen.insert(key.clone()) {
                return Err(self.error("duplicate authored Svelte attribute"));
            }
            if has_class_directives && !component && !shorthand && key == "class" {
                class_position = Some(out.len());
                class_base = Some(match &attribute.value {
                    JsxAttrValue::Bool(_) => "\"\"".to_string(),
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => {
                        format!("({}\n)", expression_payload(value))
                    }
                    JsxAttrValue::Expr(_) => {
                        return Err(self.error("empty authored Svelte attribute expression"))
                    }
                    JsxAttrValue::Str(value) => {
                        match quoted_attribute(
                            value,
                            self.boundary,
                            &self.attribute_coercion_function,
                        )
                        .map_err(|message| self.error(message))?
                        {
                            Some(expression) => {
                                self.uses_attribute_coercion |=
                                    expression.contains(&self.attribute_coercion_function);
                                format!("({expression}\n)")
                            }
                            None => serde_json::to_string(&super::entities::decode_entities(
                                value, true,
                            ))
                            .expect("literal serialization"),
                        }
                    }
                });
                continue;
            }
            let event_expression = match &attribute.value {
                JsxAttrValue::Expr(_) => true,
                JsxAttrValue::Str(value) => {
                    value.starts_with('{') && (self.boundary)(value) == Some(value.len() - 1)
                }
                _ => false,
            };
            if !component
                && !shorthand
                && key.starts_with("on")
                && key.len() > 2
                && !event_expression
            {
                return Err(
                    self.error("intrinsic event attributes require a JavaScript expression")
                );
            }
            let key_out = if !component && key == "class" {
                "className".to_string()
            } else if !component && key.starts_with("on") && key.len() > 2 {
                let event = &key[2..];
                match event {
                    "keydown" => "onKeyDown".into(),
                    "keyup" => "onKeyUp".into(),
                    _ => format!("on{}{}", event[..1].to_ascii_uppercase(), &event[1..]),
                }
            } else {
                key.clone()
            };
            if !shorthand {
                if let JsxAttrValue::Str(value) = &attribute.value {
                    if let Some(expression) =
                        quoted_attribute(value, self.boundary, &self.attribute_coercion_function)
                            .map_err(|message| self.error(message))?
                    {
                        self.uses_attribute_coercion |=
                            expression.contains(&self.attribute_coercion_function);
                        out.push_str(&format!(" {key_out}={{{expression}}}"));
                    } else if !component && key == "checked" {
                        // Static HTML checked strings express presence, including the empty string.
                        out.push_str(&format!(" {key_out}={{true}}"));
                        if name == "input"
                            && !intrinsic_spread
                            && !named_keys.iter().any(|key| key == "defaultChecked")
                        {
                            out.push_str(" defaultChecked={true}");
                        }
                    } else {
                        out.push_str(&format!(" {key_out}=\"{}\"", value.replace('"', "&quot;")));
                        if name == "input"
                            && !intrinsic_spread
                            && key == "value"
                            && !named_keys.iter().any(|key| key == "defaultValue")
                        {
                            out.push_str(&format!(
                                " defaultValue=\"{}\"",
                                value.replace('"', "&quot;")
                            ));
                        }
                    }
                    continue;
                }
            }
            let reset_default = if name == "input"
                && !intrinsic_spread
                && !shorthand
                && matches!(&attribute.value, JsxAttrValue::Bool(true))
            {
                match key.as_str() {
                    "checked" if !named_keys.iter().any(|key| key == "defaultChecked") => {
                        Some("defaultChecked")
                    }
                    "value" if !named_keys.iter().any(|key| key == "defaultValue") => {
                        Some("defaultValue")
                    }
                    _ => None,
                }
            } else {
                None
            };
            let value = if shorthand {
                if !valid_name(&key) || reserved_name(&key) {
                    return Err(self.error("unsupported shorthand expression"));
                }
                key
            } else {
                match attribute.value {
                    JsxAttrValue::Bool(true)
                        if !component
                            && !(dynamic && key == "xmlns")
                            && !matches!(
                                key.as_str(),
                                "checked" | "defaultChecked" | "defaultValue"
                            )
                            && !(key == "value"
                                && matches!(name, "textarea" | "select" | "option")) =>
                    {
                        "\"\"".to_string()
                    }
                    JsxAttrValue::Bool(value) => value.to_string(),
                    JsxAttrValue::Str(value) => {
                        serde_json::to_string(&value).expect("attribute serialization")
                    }
                    JsxAttrValue::Expr(value) if !value.trim().is_empty() => value,
                    _ => return Err(self.error("empty authored Svelte attribute expression")),
                }
            };
            out.push_str(&format!(" {key_out}={{{value}}}"));
            if let Some(default) = reset_default {
                out.push_str(&format!(" {default}={{{value}}}"));
            }
        }
        if has_class_directives {
            self.uses_class_directives = true;
            out.insert_str(
                class_position.unwrap_or(out.len()),
                &format!(
                    " className={{{}({},null,{{{}}})}}",
                    self.class_directive_helper,
                    class_base.as_deref().unwrap_or("undefined"),
                    class_flags.join(",")
                ),
            );
        }
        if self_closing || super::tags::VOID_TAGS.contains(&name) {
            out.push_str(" />");
            return Ok(out);
        }
        let body_start = self.at;
        let leading_newline_stripped = if name == "textarea" {
            // HTML consumes one initial literal line break, including CRLF.
            let body = &self.source[self.at..];
            let length = if body.starts_with("\r\n") {
                2
            } else {
                usize::from(body.starts_with(['\n', '\r']))
            };
            let prefix = body[..length].to_owned();
            self.at += length;
            (!prefix.is_empty()).then_some(prefix)
        } else {
            None
        };
        self.component_scopes.push(ComponentSnippetScope {
            depth,
            snippets: Vec::new(),
            meaningful_body: false,
        });
        let previous_namespace = self.namespace;
        let previous_head = std::mem::replace(&mut self.in_head, false);
        let (child_namespace, child_math_text) = if component {
            (element_namespace, self.math_text)
        } else if dynamic {
            (element_namespace, false)
        } else {
            match (element_namespace, name) {
                (Namespace::MathMl, "mi" | "mo" | "mn" | "ms" | "mtext") => (Namespace::Html, true),
                (Namespace::MathMl, "annotation-xml") => (
                    if annotation_html {
                        Namespace::Html
                    } else {
                        Namespace::MathMl
                    },
                    false,
                ),
                (Namespace::Svg, "foreignObject") => (Namespace::Html, false),
                (namespace, _) => (namespace, false),
            }
        };
        let previous_math_text = std::mem::replace(&mut self.math_text, child_math_text);
        self.namespace = child_namespace;
        self.element_depth += 1;
        let (mut body, _) = self.nodes(
            Some(name),
            None,
            depth,
            if component {
                FragmentKind::Component
            } else {
                FragmentKind::Intrinsic
            },
        )?;
        self.element_depth -= 1;
        self.namespace = previous_namespace;
        self.in_head = previous_head;
        self.math_text = previous_math_text;
        if let Some(prefix) = leading_newline_stripped {
            if body.is_empty() {
                // Preserve an authored text node even when HTML's initial newline
                // rule consumed its entire contents.
                self.uses_text = true;
                body = format!("<{} literal parts={{[\"\"]}}/>", self.text_component);
            }
            let text_marker_prefix = format!("<{} ", self.text_component);
            if body.starts_with(&text_marker_prefix) {
                let encoded = serde_json::to_string(&prefix).expect("newline serialization");
                body.insert_str(
                    text_marker_prefix.len(),
                    &format!("leadingNewlineStripped={{{encoded}}} "),
                );
            }
        }
        let scope = self.component_scopes.pop();
        if name == "textarea" && body.contains(&format!("<{} ", self.raw_html_component)) {
            return Err(self.error("raw HTML is not allowed inside textarea"));
        }
        if name == "textarea" && (binds_value || has_value) {
            let closing_start = self.source[..self.at].rfind("</").unwrap_or(body_start);
            if closing_start != body_start {
                return Err(self.error(if binds_value {
                    "bind:value textarea body must be empty"
                } else {
                    "textarea value conflicts with authored body"
                }));
            }
        }
        if let Some(scope) = scope.filter(|scope| !scope.snippets.is_empty()) {
            if !component {
                // Intrinsic attributes remain in their outer lexical scope.
                // Only children see these element-local snippet declarations.
                let mut declarations = String::new();
                let mut aliases = String::new();
                for snippet in scope.snippets {
                    declarations.push_str(&format!(
                        "function {}{}{{return <>{}</>;}}",
                        snippet.generated_name, snippet.parameters, snippet.body
                    ));
                    aliases.push_str(&format!(
                        "const {}={};",
                        snippet.name, snippet.generated_name
                    ));
                }
                out.push_str(&format!(
                    ">{{(()=>{{{declarations}{aliases}return <>{body}</>;}})()}}</{tag_out}>"
                ));
                return Ok(out);
            }
            for snippet in &scope.snippets {
                if named_keys.iter().any(|key| {
                    key == &snippet.name || key.strip_prefix("bind:") == Some(snippet.name.as_str())
                }) {
                    return Err(self.error(format!(
                        "snippet {} conflicts with an explicit component prop",
                        snippet.name
                    )));
                }
                if snippet.name == "children" && scope.meaningful_body {
                    return Err(self.error(
                        "children snippet cannot be combined with implicit component body",
                    ));
                }
                out.push_str(&format!(" {}={{{}}}", snippet.name, snippet.name));
            }
            if !scope.meaningful_body {
                body.clear();
            }
            if body.is_empty() {
                out.push_str(" />");
            } else {
                out.push('>');
                out.push_str(&body);
                out.push_str(&format!("</{name}>"));
            }
            let mut declarations = String::new();
            let mut aliases = String::new();
            for snippet in scope.snippets {
                declarations.push_str(&format!(
                    "function {}{}{{return <>{}</>;}}",
                    snippet.generated_name, snippet.parameters, snippet.body
                ));
                aliases.push_str(&format!(
                    "const {}={};",
                    snippet.name, snippet.generated_name
                ));
            }
            return Ok(format!(
                "{{(()=>{{{declarations}{aliases}return {out};}})()}}"
            ));
        }
        out.push('>');
        out.push_str(&body);
        out.push_str(&format!("</{tag_out}>"));
        Ok(out)
    }
}
pub(super) fn textarea_close(source: &str) -> Option<usize> {
    let tail = source.strip_prefix("</")?;
    if !tail.get(..8)?.eq_ignore_ascii_case("textarea") {
        return None;
    }
    let tail = &tail[8..];
    let whitespace = tail.bytes().take_while(u8::is_ascii_whitespace).count();
    (tail.as_bytes().get(whitespace) == Some(&b'>')).then_some(11 + whitespace)
}

// A terminating line comment must not consume generated JSX/JavaScript punctuation.
// Adding a line break is harmless when the slash pair belongs to a string or regex.
fn expression_payload(value: &str) -> String {
    let mut value = value.trim().to_string();
    if value.contains("//") {
        value.push('\n');
    }
    value
}

fn valid_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '_' | '$'))
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
}

fn reserved_name(value: &str) -> bool {
    matches!(
        value,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

/// A sole interpolation preserves its value type; mixed values stringify with
/// Svelte's nullish-to-empty behavior. Decode literal entities only after finding
/// interpolation boundaries, so entity-encoded braces remain literal data.
fn quoted_attribute(
    value: &str,
    boundary: fn(&str) -> Option<usize>,
    coercion: &str,
) -> Result<Option<String>, String> {
    if !value.contains('{') {
        return Ok(None);
    }
    let mut parts = Vec::new();
    let mut rest = value;
    let mut literals = false;
    while let Some(at) = rest.find('{') {
        if at > 0 {
            literals = true;
            parts.push((false, super::entities::decode_entities(&rest[..at], true)));
        }
        let end = boundary(&rest[at..])
            .ok_or_else(|| "unclosed quoted attribute interpolation".to_string())?;
        let expression = rest[at + 1..at + end].trim();
        if expression.is_empty() {
            return Err("empty quoted attribute interpolation".into());
        }
        parts.push((true, expression_payload(expression)));
        rest = &rest[at + end + 1..];
    }
    if !rest.is_empty() {
        literals = true;
        parts.push((false, super::entities::decode_entities(rest, true)));
    }
    if !literals && parts.len() == 1 {
        return Ok(Some(parts.remove(0).1));
    }
    let mut expression = "\"\"".to_string();
    for (dynamic, part) in parts {
        if dynamic {
            // Per-part markers preserve evaluation/coercion order; the target
            // chooses DOM string-hint versus SSR default-hint conversion.
            expression.push_str(&format!(" + {coercion}(({part}))"));
        } else {
            expression.push_str(&format!(
                " + {}",
                serde_json::to_string(&part).expect("literal serialization")
            ));
        }
    }
    Ok(Some(expression))
}

#[cfg(test)]
mod inline_snippet_tests {
    use super::*;

    #[test]
    fn component_snippets_keep_occurrence_scope_and_append_props_after_spreads() {
        let module = parse_svelte_module(
            "<Card other={row} {...props}>{#snippet row(value:string)}<b>{value}</b>{@render footer()}{/snippet}{#snippet footer()}<i>end</i>{/snippet}{@render row(name)}</Card>",
        ).unwrap();
        assert!(module.snippets.is_empty());
        assert_eq!(module.inline_snippet_names.len(), 2);
        let jsx = &module.template_jsx;
        assert!(
            jsx.contains("{(()=>{function __crepusInlineSnippet"),
            "{jsx}"
        );
        assert!(jsx.contains("const row=__crepusInlineSnippet"), "{jsx}");
        assert!(jsx.contains("const footer=__crepusInlineSnippet"), "{jsx}");
        let returned = &jsx[jsx.find("return <Card").unwrap()..];
        assert!(returned.contains("other={row}"), "{jsx}");
        assert!(returned.contains("row={row} footer={footer}"), "{jsx}");
        assert!(returned.find("{...(props)}").unwrap() < returned.find("row={row}").unwrap());
        assert!(returned.contains("call={()=> (row(name))}"), "{jsx}");
    }

    #[test]
    fn nested_components_and_root_snippets_can_reuse_authored_names() {
        let module = parse_svelte_module(
            "{#snippet row()}root{/snippet}<Card>{#snippet row()}outer{/snippet}<Child>{#snippet row()}inner{/snippet}{@render row()}</Child>{@render row()}</Card><Card>{#snippet row()}other{/snippet}</Card>{@render row()}",
        ).unwrap();
        assert_eq!(module.snippets.len(), 1);
        assert_eq!(module.snippets[0].name, "row");
        assert_eq!(module.inline_snippet_names.len(), 3);
        let unique: std::collections::HashSet<_> = module.inline_snippet_names.iter().collect();
        assert_eq!(unique.len(), 3);
        assert_eq!(module.template_jsx.matches("const row=").count(), 3);
        assert!(module.template_jsx.ends_with("call={()=> (row())}/></>"));
    }

    #[test]
    fn generated_inline_names_avoid_authored_source_and_names_are_not_root_hoisted() {
        let module = parse_svelte_module(
            "<script>const __crepusInlineSnippet0=7;</script><Card>{#snippet row()}local{/snippet}</Card><p>{row}</p>",
        ).unwrap();
        assert_eq!(module.inline_snippet_names, ["__crepusInlineSnippet1"]);
        assert!(module.snippets.is_empty());
        let marker = module.intrinsic_namespace_attribute.as_deref().unwrap();
        let jsx = &module.template_jsx;
        assert!(
            jsx.contains(&format!("const row=__crepusInlineSnippet1;return <Card row={{row}} />;}})()}}<p {marker}=\"http://www.w3.org/1999/xhtml\">")),
            "{jsx}"
        );
    }

    #[test]
    fn named_only_bodies_do_not_create_implicit_children() {
        for content in [
            "{#snippet row()}named{/snippet}",
            " \n<!-- comment --> {#snippet row()}named{/snippet} \n",
            " \n{#snippet children()}named{/snippet}<!-- comment -->\n",
        ] {
            let module = parse_svelte_module(&format!("<Card>{content}</Card>")).unwrap();
            let jsx = &module.template_jsx;
            let returned = &jsx[jsx.find("return <Card").unwrap()..];
            assert!(returned.contains(" />;})()}"), "{jsx}");
            assert!(!returned.contains("</Card>"), "{jsx}");
        }
    }

    #[test]
    fn meaningful_body_preserves_authored_spaces_and_rejects_children_snippet_conflicts() {
        let module = parse_svelte_module(
            "<Card>{#snippet row()}named{/snippet} before <b>body</b> after </Card>",
        )
        .unwrap();
        assert!(module
            .template_jsx
            .contains("literal parts={[\" before \"]}"));
        assert!(module
            .template_jsx
            .contains("literal parts={[\" after \"]}"));
        for body in [
            "text",
            "<b/>",
            "{value}",
            "{@render row()}",
            "{@html value}",
            "{#if value}<b/>{/if}",
            "{#key value}<b/>{/key}",
        ] {
            let source = format!("<Card>{{#snippet children()}}named{{/snippet}}{body}</Card>");
            let error = parse_svelte_module(&source).unwrap_err().to_string();
            assert!(error.contains("children snippet"), "{source}: {error}");
        }
    }

    #[test]
    fn duplicate_names_and_explicit_component_prop_conflicts_fail() {
        for source in [
            "<Card>{#snippet row()}one{/snippet}{#snippet row()}two{/snippet}</Card>",
            "<Card row={existing}>{#snippet row()}one{/snippet}</Card>",
            "<Card {row}>{#snippet row()}one{/snippet}</Card>",
            "<Card bind:row={value}>{#snippet row()}one{/snippet}</Card>",
            "<Card children={existing}>{#snippet children()}one{/snippet}</Card>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn intrinsic_snippets_bind_only_their_children_and_keep_sibling_scopes_independent() {
        let source = "<div title={row}>{#snippet row(value:string)}<b>{value}</b>{/snippet}{@render row('inner')}<span>{#snippet row()}nested{/snippet}{@render row()}</span></div><aside>{#snippet row()}sibling{/snippet}{@render row()}</aside>";
        let parsed = parse_svelte_module(source).unwrap();
        assert!(parsed.snippets.is_empty());
        assert_eq!(parsed.inline_snippet_names.len(), 3);
        let marker = parsed.intrinsic_namespace_attribute.as_deref().unwrap();
        let opening = format!("<div {marker}=\"http://www.w3.org/1999/xhtml\" title={{row}}");
        assert!(
            parsed
                .template_jsx
                .contains(&format!("{opening}>{{(()=>{{")),
            "{}",
            parsed.template_jsx
        );
        assert!(!parsed.template_jsx.contains(&format!("{opening} row=")));
        assert!(parse_svelte_module(
            "<div>{#snippet row()}one{/snippet}{#snippet row()}two{/snippet}</div>"
        )
        .is_err());
    }

    #[test]
    fn declarations_outside_direct_component_bodies_remain_unsupported() {
        for source in [
            "{#if ok}{#snippet row()}one{/snippet}{/if}",
            "<Card>{#if value}{#snippet row()}one{/snippet}{/if}</Card>",
            "<Card>{#key value}{#snippet row()}one{/snippet}{/key}</Card>",
            "{#snippet outer()}{#snippet row()}one{/snippet}{/snippet}",
        ] {
            let error = parse_svelte_module(source).unwrap_err().to_string();
            assert!(
                error.contains("nested authored Svelte snippets"),
                "{source}: {error}"
            );
        }
    }
}

#[cfg(test)]
mod bare_attribute_tests {
    use super::*;

    #[test]
    fn bare_intrinsic_attributes_are_empty_strings_except_control_properties() {
        let module = parse_svelte_module("<input data-root aria-label title hidden disabled required checked defaultChecked defaultValue value />").unwrap();
        for name in [
            "data-root",
            "aria-label",
            "title",
            "hidden",
            "disabled",
            "required",
            "value",
        ] {
            assert!(
                module.template_jsx.contains(&format!(" {name}={{\"\"}}")),
                "{}",
                module.template_jsx
            );
        }
        for name in ["checked", "defaultChecked", "defaultValue"] {
            assert!(
                module.template_jsx.contains(&format!(" {name}={{true}}")),
                "{}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn bound_select_requires_authored_static_multiple_before_normalization() {
        for source in [
            "<select multiple bind:value={value}/>",
            "<select multiple=\"\" bind:value={value}/>",
            "<select multiple={flag}/>",
        ] {
            assert!(parse_svelte_module(source).is_ok(), "{source}");
        }
        for source in [
            "<select multiple={true} bind:value={value}/>",
            "<select multiple={flag} bind:value={value}/>",
            "<select {multiple} bind:value={value}/>",
            "<select multiple=\"{flag}\" bind:value={value}/>",
        ] {
            let error = parse_svelte_module(source).unwrap_err();
            assert!(
                error.to_string().contains("multiple must be static"),
                "{source}: {error}"
            );
        }
    }

    #[test]
    fn bare_value_preserves_boolean_identity_for_textarea_select_and_option() {
        for tag in ["textarea", "select", "option"] {
            let module = parse_svelte_module(&format!("<{tag} value></{tag}>")).unwrap();
            assert!(
                module.template_jsx.contains("value={true}"),
                "{}",
                module.template_jsx
            );
        }
        for tag in ["input", "progress", "meter", "li"] {
            let module = parse_svelte_module(&format!("<{tag} value />")).unwrap();
            assert!(
                module.template_jsx.contains("value={\"\"}"),
                "{}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn component_bare_props_and_intrinsic_expressions_keep_their_values() {
        let module = parse_svelte_module("<Child data-root checked defaultChecked defaultValue onclick/><div data-root={true} aria-label={false} title={\"\"} {hidden}/>").unwrap();
        for name in [
            "data-root",
            "checked",
            "defaultChecked",
            "defaultValue",
            "onclick",
        ] {
            assert!(
                module.template_jsx.contains(&format!(" {name}={{true}}")),
                "{}",
                module.template_jsx
            );
        }
        for attribute in [
            "data-root={true}",
            "aria-label={false}",
            "title={\"\"}",
            "hidden={hidden}",
        ] {
            assert!(
                module.template_jsx.contains(attribute),
                "{}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn static_checked_strings_express_presence_without_changing_dynamic_or_component_props() {
        for value in ["", "false", "checked"] {
            let module = parse_svelte_module(&format!("<input checked=\"{value}\" />")).unwrap();
            assert!(
                module.template_jsx.contains("checked={true}"),
                "{}",
                module.template_jsx
            );
        }
        let module = parse_svelte_module(
            "<input checked={\"\"}/><input checked=\"{flag}\"/><Child checked=\"\"/>",
        )
        .unwrap();
        assert!(module.template_jsx.contains("checked={\"\"}"));
        assert!(module.template_jsx.contains("checked={flag}"));
        assert!(module.template_jsx.contains("<Child checked=\"\""));
        assert!(!module.template_jsx.contains("checked={true}"));
    }

    #[test]
    fn intrinsic_event_handlers_require_expressions_but_shorthand_and_component_props_are_allowed()
    {
        for attribute in [
            "onclick",
            "onclick=\"\"",
            "onclick=\"handler\"",
            "onkeydown=true",
            "onclick=\"x{handler}\"",
            "onclick=\"{first}{second}\"",
            "onclick=\" {handler}\"",
        ] {
            let error = parse_svelte_module(&format!("<button {attribute}>go</button>"))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("event attributes require a JavaScript expression"),
                "{attribute}: {error}"
            );
        }
        let module = parse_svelte_module(
            "<button onclick={handler} {onkeydown}>go</button><Child onclick onkeydown=\"text\"/>",
        )
        .unwrap();
        assert!(module.template_jsx.contains("onClick={handler}"));
        assert!(module.template_jsx.contains("onKeyDown={onkeydown}"));
        assert!(module.template_jsx.contains("onclick={true}"));
        assert!(module.template_jsx.contains("onkeydown=\"text\""));
        assert!(parse_svelte_module("<button on:click={handler}/>").is_err());
    }

    #[test]
    fn intrinsic_quoted_event_values_allow_exactly_one_expression() {
        for value in [
            "{handler}",
            "{() => 1}",
            "{handler /* comment */}",
            "{() => { return 1 }}",
        ] {
            let module =
                parse_svelte_module(&format!("<button onclick=\"{value}\">go</button>")).unwrap();
            assert!(
                module.template_jsx.contains("onClick={"),
                "{}",
                module.template_jsx
            );
            assert!(
                !module.template_jsx.contains("onClick=\""),
                "{}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn binding_shorthands_are_consumed_before_bare_attribute_conversion() {
        let module =
            parse_svelte_module("<input type=\"checkbox\" bind:checked/><input bind:value/>")
                .unwrap();
        let checked = module.checked_binding_attribute.unwrap();
        let value = module.value_binding_attribute.unwrap();
        assert!(module
            .template_jsx
            .contains(&format!("{checked}={{checked}}")));
        assert!(module.template_jsx.contains(&format!("{value}={{value}}")));
    }

    #[test]
    fn files_binding_accepts_expression_and_shorthand_with_collision_free_marker() {
        let module = parse_svelte_module(
            "<input type=\"file\" bind:files={form.files}/><input type=\"file\" bind:files/>",
        )
        .unwrap();
        let files = module.files_binding_attribute.unwrap();
        assert_eq!(files, "__crepusFilesBinding0");
        assert!(module
            .template_jsx
            .contains(&format!("{files}={{form.files}}")));
        assert!(module.template_jsx.contains(&format!("{files}={{files}}")));
        let module = parse_svelte_module(
            "<script>let __crepusFilesBinding0 = 1;</script><input type=\"file\" bind:files/>",
        )
        .unwrap();
        assert_eq!(
            module.files_binding_attribute.as_deref(),
            Some("__crepusFilesBinding1")
        );
        assert!(parse_svelte_module("<input/>")
            .unwrap()
            .files_binding_attribute
            .is_none());
    }

    #[test]
    fn files_binding_rejects_non_file_dynamic_duplicate_and_explicit_files() {
        for source in [
            "<input bind:files={f}/>",
            "<input type=\"text\" bind:files={f}/>",
            "<input type={kind} bind:files={f}/>",
            "<textarea bind:files={f}></textarea>",
            "<svelte:element this={tag} type=\"file\" bind:files={f}/>",
            "<input type=\"file\" bind:files={a} bind:files={b}/>",
            "<input type=\"file\" files={x} bind:files={f}/>",
            "<input type=\"file\" bind:files={f} files={x}/>",
            "<input type=\"file\" bind:files=\"\"/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn open_binding_accepts_static_details_expression_and_shorthand_with_collision_free_marker() {
        let module = parse_svelte_module(
            "<details bind:open={panel.open}></details><details bind:open></details>",
        )
        .unwrap();
        let open = module.open_binding_attribute.unwrap();
        assert_eq!(open, "__crepusOpenBinding0");
        assert!(module
            .template_jsx
            .contains(&format!("{open}={{panel.open}}")));
        assert!(module.template_jsx.contains(&format!("{open}={{open}}")));
        let module = parse_svelte_module(
            "<script>let __crepusOpenBinding0 = 1;</script><details bind:open></details>",
        )
        .unwrap();
        assert_eq!(
            module.open_binding_attribute.as_deref(),
            Some("__crepusOpenBinding1")
        );
        assert!(parse_svelte_module("<details></details>")
            .unwrap()
            .open_binding_attribute
            .is_none());
        let module = parse_svelte_module("<Field bind:open={panel.open}/>").unwrap();
        assert!(module.open_binding_attribute.is_none());
        assert_eq!(module.component_bindings[0].1, "open");
    }

    #[test]
    fn files_and_open_bindings_preserve_admitted_intrinsic_spreads() {
        for (source, binding) in [
            ("<input type=\"file\" bind:files={f} {...rest}/>", "files"),
            ("<details bind:open={o} {...rest}></details>", "open"),
        ] {
            let module = parse_svelte_module(source).unwrap();
            let marker = if binding == "files" {
                module.files_binding_attribute.as_ref().unwrap()
            } else {
                module.open_binding_attribute.as_ref().unwrap()
            };
            let expression = if binding == "files" { "f" } else { "o" };
            assert!(
                module
                    .template_jsx
                    .contains(&format!("{marker}={{{expression}}}")),
                "{}",
                module.template_jsx
            );
            assert!(
                module.template_jsx.contains("{...(rest)}"),
                "{}",
                module.template_jsx
            );
            assert!(
                module.template_jsx.find(marker).unwrap()
                    < module.template_jsx.find("{...(rest)}").unwrap()
            );
        }
    }

    #[test]
    fn open_binding_rejects_other_dynamic_conflicting_duplicate_and_invalid_targets() {
        for source in [
            "<div bind:open={o}></div>",
            "<dialog bind:open={o}></dialog>",
            "<input bind:open={o}/>",
            "<svelte:element this={tag} bind:open={o}/>",
            "<svg><details bind:open={o}></details></svg>",
            "<details bind:open={a} bind:open={b}></details>",
            "<details bind:open={o} open></details>",
            "<details open bind:open={o}></details>",
            "<details bind:open={o} open={x}></details>",
            "<details bind:open={o} {open}></details>",
            "<details bind:open=\"\"></details>",
            "<details bind:open={ }></details>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod static_input_default_tests {
    use super::parse_svelte_module;

    #[test]
    fn static_input_state_keeps_live_props_and_reset_defaults() {
        for (source, live, default) in [
            (
                "<input checked/>",
                "checked={true}",
                "defaultChecked={true}",
            ),
            (
                r#"<input checked="false"/>"#,
                "checked={true}",
                "defaultChecked={true}",
            ),
            (
                r#"<input checked=""/>"#,
                "checked={true}",
                "defaultChecked={true}",
            ),
            ("<input value/>", r#"value={""}"#, r#"defaultValue={""}"#),
            (
                r#"<input value="a&amp;&quot;b"/>"#,
                r#"value="a&amp;&quot;b""#,
                r#"defaultValue="a&amp;&quot;b""#,
            ),
        ] {
            let jsx = parse_svelte_module(source).unwrap().template_jsx;
            assert!(jsx.contains(live), "{jsx}");
            assert!(jsx.contains(default), "{jsx}");
        }
    }

    #[test]
    fn authored_defaults_win_in_both_orders_including_shorthand() {
        for source in [
            r#"<input checked defaultChecked={false}/>"#,
            r#"<input defaultChecked={false} checked="false"/>"#,
            r#"<input value="live" defaultValue="initial"/>"#,
            r#"<input defaultValue="initial" value="live"/>"#,
            r#"<input checked {defaultChecked} value="live" {defaultValue}/>"#,
        ] {
            let jsx = parse_svelte_module(source).unwrap().template_jsx;
            if source.contains("defaultChecked") {
                assert_eq!(jsx.matches("defaultChecked=").count(), 1, "{jsx}");
            }
            if source.contains("defaultValue") {
                assert_eq!(jsx.matches("defaultValue=").count(), 1, "{jsx}");
            }
        }
    }

    #[test]
    fn expressions_components_other_tags_and_bindings_do_not_gain_defaults() {
        for source in [
            r#"<input checked={true} value={value}/>",
            r#"<input checked="{flag}" value="prefix {value}"/>"#,
            r#"<input {checked} {value}/>"#,
            r#"<Child checked value="literal"/>"#,
            r#"<textarea value="literal"></textarea>"#,
            r#"<option value="literal"></option>"#,
            r#"<input type="checkbox" bind:checked/>"#,
            r#"<input bind:value/>"#,
        ] {
            let jsx = parse_svelte_module(source).unwrap().template_jsx;
            assert!(!jsx.contains("defaultChecked="), "{jsx}");
            assert!(!jsx.contains("defaultValue="), "{jsx}");
        }
    }
}

#[cfg(test)]
mod textarea_content_tests {
    use super::parse_svelte_module;

    #[test]
    fn value_and_body_conflict_without_dropping_authored_content() {
        for attribute in ["value", "value=\"x\"", "value={value}", "{value}"] {
            for body in [" ", "\n", "plain", "{value}"] {
                let source = format!("<textarea {attribute}>{body}</textarea>");
                assert!(parse_svelte_module(&source).is_err(), "{source}");
            }
            let source = format!("<textarea {attribute}></textarea>");
            assert!(parse_svelte_module(&source).is_ok(), "{source}");
        }
        assert!(parse_svelte_module("<textarea defaultValue=\"reset\">{value}</textarea>").is_ok());
    }
}

#[cfg(test)]
mod textarea_literal_tests {
    use super::parse_svelte_module;

    #[test]
    fn textarea_consumes_only_one_initial_literal_line_break() {
        for prefix in ["\n", "\r", "\r\n"] {
            let module =
                parse_svelte_module(&format!("<textarea>{prefix}  plain</textarea>")).unwrap();
            assert!(
                module
                    .template_jsx
                    .contains(r#"literal parts={["  plain"]}"#),
                "{}",
                module.template_jsx
            );
        }
        let module = parse_svelte_module("<textarea>\n\nplain</textarea>").unwrap();
        assert!(module
            .template_jsx
            .contains(r#"literal parts={["\nplain"]}"#));
        assert!(parse_svelte_module("<textarea value=\"x\">\n</textarea>").is_err());
    }

    #[test]
    fn textarea_literals_do_not_become_top_level_script_or_style_blocks() {
        let module = parse_svelte_module(
            r#"<textarea><script>literal</script><style>literal</style>< broken {"</textarea>"}</textarea><script>let actual=1;</script>"#,
        )
        .unwrap();
        assert_eq!(module.script.as_deref(), Some("let actual=1;"));
        assert!(module.module_script.is_none());
        assert!(module.template_jsx.contains("literal"));
        assert!(module.template_jsx.contains(r#"()=> ("</textarea>")"#));
    }

    #[test]
    fn textarea_markup_and_comments_are_literal_data() {
        let module =
            parse_svelte_module("<textarea>  <Child/><!-- comment --><b>{value}</b>\n </textarea>")
                .unwrap();
        assert!(module.component_bindings.is_empty());
        assert!(
            module
                .template_jsx
                .contains(r#""  \u003cChild/>\u003c!-- comment -->\u003cb>""#),
            "{}",
            module.template_jsx
        );
        assert!(module.template_jsx.contains("()=> (value)"));
        assert!(module.template_jsx.contains(r#""\u003c/b>\n ""#));
    }

    #[test]
    fn textarea_entities_braces_strings_and_closing_prefixes_keep_boundaries() {
        let module = parse_svelte_module(
            r#"<textarea>&lt;b&gt;&#123;literal&#125;{value + "}"}</textareaX> tail</textarea>"#,
        )
        .unwrap();
        assert!(module
            .template_jsx
            .contains(r#"{text:"&lt;b&gt;&#123;literal&#125;"}"#));
        assert!(module.template_jsx.contains(r#"()=> (value + "}")"#));
        assert!(module.template_jsx.contains(r#""\u003c/textareaX> tail""#));
        assert!(parse_svelte_module("<textarea> \n\t </textarea>")
            .unwrap()
            .template_jsx
            .contains(r#"literal parts={[" \n\t "]}"#));
    }

    #[test]
    fn textarea_close_is_case_insensitive_with_only_trailing_ascii_whitespace() {
        for close in [
            "</textarea>",
            "</textarea >",
            "</TEXTAREA>",
            "</TeXtArEa\t>",
        ] {
            assert!(
                parse_svelte_module(&format!("<textarea>body{close}")).is_ok(),
                "{close}"
            );
        }
        for literal in ["</textareaX>", "</ textarea>", "</textarea/>"] {
            assert!(
                parse_svelte_module(&format!("<textarea>{literal}")).is_err(),
                "{literal}"
            );
            let module = parse_svelte_module(&format!("<textarea>{literal}</textarea>")).unwrap();
            assert!(
                module
                    .template_jsx
                    .contains(&literal.replace('<', "\\u003c")),
                "{}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn textarea_blocks_and_directives_fail_before_recursing() {
        for source in [
            "<textarea>{#if ready}x{/if}</textarea>",
            "<textarea>{#key value}x{/key}</textarea>",
            "<textarea>{@html value}</textarea>",
            "<textarea>{@render snippet()}</textarea>",
            "<textarea>{#snippet row()}{/snippet}</textarea>",
            "<textarea>{:else}</textarea>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod textarea_newline_metadata_tests {
    use super::*;

    #[test]
    fn raw_initial_line_breaks_mark_dynamic_textarea_parts_after_one_strip() {
        for newline in ["\n", "\r", "\r\n"] {
            for count in [1, 2] {
                let source = format!(
                    "<textarea>{}&#10;{{value}}</textarea>",
                    newline.repeat(count)
                );
                let module = parse_svelte_module(&source).unwrap();
                let marker = module.text_component.unwrap();
                assert!(
                    module.template_jsx.contains(&format!(
                        "<{marker} leadingNewlineStripped={{{}}} parts=",
                        serde_json::to_string(newline).unwrap()
                    )),
                    "{source:?}: {}",
                    module.template_jsx
                );
                assert_eq!(
                    module
                        .template_jsx
                        .matches("leadingNewlineStripped")
                        .count(),
                    1
                );
            }
        }
    }

    #[test]
    fn newline_only_textarea_preserves_empty_literal_body_provenance() {
        for newline in ["\n", "\r\n", "\r"] {
            for attributes in ["", " defaultValue=\"fallback\""] {
                let source = format!("<textarea{attributes}>{newline}</textarea>");
                let module = parse_svelte_module(&source).unwrap();
                let marker = module.text_component.unwrap();
                assert!(
                    module.template_jsx.contains(&format!(
                        "<{marker} leadingNewlineStripped={{{}}} literal parts={{[\"\"]}}/>",
                        serde_json::to_string(newline).unwrap()
                    )),
                    "{source:?}: {}",
                    module.template_jsx
                );
            }
        }
        for source in [
            "<textarea></textarea>",
            "<textarea defaultValue=\"fallback\"></textarea>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(module.text_component.is_none(), "{}", module.template_jsx);
            assert!(!module.template_jsx.contains("leadingNewlineStripped"));
        }
    }

    #[test]
    fn metadata_requires_a_stripped_raw_line_break_in_a_textarea_body() {
        for source in [
            "<textarea>&#10;{value}</textarea>",
            "<textarea>text{value}</textarea>",
            "<textarea>{value}</textarea>",
            "<div>\n&#10;{value}</div>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(
                !module.template_jsx.contains("leadingNewlineStripped"),
                "{source:?}: {}",
                module.template_jsx
            );
        }
    }
}

#[cfg(test)]
mod literal_entity_provenance_tests {
    use super::*;

    #[test]
    fn static_entities_keep_raw_text_in_a_literal_marker() {
        let text = "&copy &amp;amp; &#65 &#x41; &unknown; &#123; &#60;b&#62;\n\tend";
        let module = parse_svelte_module(&format!("<p>{text}</p>")).unwrap();
        let marker = module.text_component.unwrap();
        let encoded = serde_json::to_string(text).unwrap();
        assert!(
            module.template_jsx.contains(&format!(
                "<{marker} literal parts={{[{{text:{encoded}}}]}}/>"
            )),
            "{}",
            module.template_jsx
        );
        assert!(!module.template_jsx.contains("©"));
        assert!(!module.template_jsx.contains("<b>"));
        assert!(!module.template_jsx.contains("<__CrepusRawHTML"));
    }

    #[test]
    fn static_comment_separated_chunks_keep_distinct_parts_and_original_spellings() {
        let module = parse_svelte_module("<p>one &copy<!-- comment -->two &amp;amp;</p>").unwrap();
        assert_eq!(module.template_jsx.matches(" literal ").count(), 1);
        assert!(
            module
                .template_jsx
                .contains(r#"parts={[{text:"one &copy"},{text:"two &amp;amp;"}]}"#),
            "{}",
            module.template_jsx
        );
        let split_entity = parse_svelte_module("<p>&#1<!--x-->28;</p>").unwrap();
        assert!(split_entity
            .template_jsx
            .contains(r#"parts={[{text:"&#1"},"28;"]}"#));
    }

    #[test]
    fn dynamic_text_retains_its_existing_shape_and_javascript_strings_are_not_entity_literals() {
        let module = parse_svelte_module("<p>&copy {value}&amp;amp;</p>").unwrap();
        assert!(module.text_component.is_some());
        assert!(!module.template_jsx.contains(" literal "));
        assert!(
            module
                .template_jsx
                .contains("{text:\"&copy \"},()=> (value),{text:\"&amp;amp;\"}"),
            "{}",
            module.template_jsx
        );
        let expression = parse_svelte_module("<p>{'&copy;'}</p>").unwrap();
        assert!(!expression.template_jsx.contains(" literal "));
        assert!(!expression.template_jsx.contains("{text:"));
        assert!(expression.template_jsx.contains("()=> ('&copy;')"));
    }

    #[test]
    fn plain_static_text_keeps_raw_bytes_in_a_literal_marker() {
        let text = "plain text\twith quotes \" and slash \\";
        let module = parse_svelte_module(&format!("<p>{text}</p>")).unwrap();
        assert!(module.text_component.is_some());
        assert!(module.template_jsx.contains(" literal "));
        let encoded = serde_json::to_string(text).unwrap();
        assert!(
            module
                .template_jsx
                .contains(&format!("parts={{[{encoded}]}}")),
            "{}",
            module.template_jsx
        );
    }

    #[test]
    fn static_entity_textareas_preserve_consumed_newline_tokens_for_native_unwrapping() {
        for newline in ["\n", "\r", "\r\n"] {
            for count in [1, 2] {
                let source = format!("<textarea>{}&#10;static</textarea>", newline.repeat(count));
                let module = parse_svelte_module(&source).unwrap();
                let marker = module.text_component.unwrap();
                let prefix = serde_json::to_string(newline).unwrap();
                assert!(
                    module.template_jsx.contains(&format!(
                        "<{marker} leadingNewlineStripped={{{prefix}}} literal parts="
                    )),
                    "{source:?}: {}",
                    module.template_jsx
                );
            }
        }
        let module = parse_svelte_module("<textarea>&#10;static</textarea>").unwrap();
        assert!(module.template_jsx.contains(" literal "));
        assert!(!module.template_jsx.contains("leadingNewlineStripped"));
        let module = parse_svelte_module("<textarea>\nstatic</textarea>").unwrap();
        assert!(module.text_component.is_some());
        assert!(module
            .template_jsx
            .contains("leadingNewlineStripped={\"\\n\"} literal parts={[\"static\"]}"));
    }

    #[test]
    fn quoted_attribute_entity_policy_remains_separate() {
        let interpolated = parse_svelte_module("<p title=\"&copy {value}\">text</p>").unwrap();
        assert!(interpolated.template_jsx.contains("© "));
        let module = parse_svelte_module("<p title=\"&copy\">plain</p>").unwrap();
        assert!(module.text_component.is_some());
        assert!(module.template_jsx.contains("literal parts={[\"plain\"]}"));
        assert!(module.template_jsx.contains("title=\"&copy\""));
    }
}

#[cfg(test)]
mod textarea_value_provenance_tests {
    use super::*;

    #[test]
    fn textarea_values_preserve_only_the_raw_first_literal() {
        for (attribute, raw) in [
            ("value=\"\ntext\"", "\ntext"),
            ("value=\"&#10;{value}tail\"", "&#10;"),
            ("value=\"prefix{value}tail\"", "prefix"),
            ("value=\"{value}tail\"", ""),
            ("value=\"{value}\"", ""),
            ("value={value}", ""),
            ("value", ""),
            ("{value}", ""),
        ] {
            let module = parse_svelte_module(&format!("<textarea {attribute}/>")).unwrap();
            let marker = module.textarea_value_attribute.unwrap();
            assert!(
                module.template_jsx.contains(&format!(
                    "{marker}={{{}}}",
                    serde_json::to_string(raw).unwrap()
                )),
                "{attribute}: {}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn marker_is_hygienic_and_limited_to_explicit_intrinsic_textarea_values() {
        let module = parse_svelte_module(
            "<script>const __crepusTextareaValue0=1;</script><textarea value=\"&unknown;\"/>",
        )
        .unwrap();
        assert_eq!(
            module.textarea_value_attribute.as_deref(),
            Some("__crepusTextareaValue1")
        );
        assert!(module.template_jsx.contains("value=\"&unknown;\""));
        for source in [
            "<textarea>body</textarea>",
            "<textarea bind:value={value}/>",
            "<input value=\"x\"/>",
            "<Textarea value=\"x\"/>",
        ] {
            assert!(
                parse_svelte_module(source)
                    .unwrap()
                    .textarea_value_attribute
                    .is_none(),
                "{source}"
            );
        }
    }
}

#[cfg(test)]
mod each_authored_tests {
    use super::*;
    #[test]
    fn each_retains_header_index_key_nested_body_and_fallback() {
        let module=parse_svelte_module("{#each rows as row, index (row.id)}<p>{row.label}/{index}</p>{:else}<b>empty</b>{/each}").unwrap();
        assert!(module.each_block_component.is_some());
        assert!(
            module.template_jsx.contains("rows as row, index (row.id)"),
            "{}",
            module.template_jsx
        );
        assert!(module.template_jsx.contains("children={"));
        assert!(module.template_jsx.contains("fallback={"));
    }
    #[test]
    fn unclosed_each_and_else_if_are_errors() {
        for source in [
            "{#each rows as row}<p/>",
            "{#each rows as row}<p/>{:else if ready}<b/>{/each}",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

fn validate_style_attributes(root: &str, attributes: &[JsxAttr]) -> Result<(), RawParseError> {
    if attributes.len() > 1 || attributes.iter().any(|attribute|
        !matches!((attribute.key.as_str(), &attribute.value), ("lang", JsxAttrValue::Str(value)) if value == "css")
    ) {
        return Err(svelte_err(root, root, "unsupported authored Svelte style attributes (only lang=css is supported)"));
    }
    Ok(())
}

#[cfg(test)]
mod directive_authored_tests {
    use super::*;

    #[test]
    fn intrinsic_directives_keep_order_and_lazy_expressions() {
        let parsed = parse_svelte_module("<button use:actions.first={parameter} {@attach attachment} use:second {@attach other}/>").unwrap();
        assert_eq!(parsed.directives.len(), 4);
        assert!(matches!(
            &parsed.directives[0],
            SvelteDirective::Action {
                parameter: true,
                ..
            }
        ));
        assert!(matches!(
            &parsed.directives[1],
            SvelteDirective::Attachment { .. }
        ));
        assert!(matches!(
            &parsed.directives[2],
            SvelteDirective::Action {
                parameter: false,
                ..
            }
        ));
        assert!(matches!(
            &parsed.directives[3],
            SvelteDirective::Attachment { .. }
        ));
        assert!(parsed.template_jsx.contains("=>actions.first?.("));
        assert!(
            parsed.template_jsx.contains("()=> (parameter")
                || parsed.template_jsx.contains("()=>(parameter")
        );
        assert!(parsed.template_jsx.contains("()=>(attachment"));
        assert!(!parsed.template_jsx.contains("use:"));
        assert!(!parsed.template_jsx.contains("@attach"));
    }

    #[test]
    fn markers_are_hygienic_and_parameter_presence_is_explicit() {
        let parsed = parse_svelte_module("<script>const __crepusDirective0=1;</script><p use:action={undefined}/><p use:action/>").unwrap();
        assert!(
            matches!(&parsed.directives[0], SvelteDirective::Action { attribute, parameter: true } if attribute == "__crepusDirective1")
        );
        assert!(
            matches!(&parsed.directives[1], SvelteDirective::Action { attribute, parameter: false } if attribute == "__crepusDirective2")
        );
        assert!(parse_svelte_module("<p/>").unwrap().directives.is_empty());
        assert!(parse_svelte_module("<p use:actions.default/><p use:actions.delete/>").is_ok());
    }

    #[test]
    fn unsupported_directive_forms_do_not_become_attributes() {
        for source in [
            "<Child use:action/>",
            "<Child {@attach attachment}/>",
            "<p use:action|once/>",
            "<p use:action use:action/>",
            "<p use:action=\"literal\"/>",
            "<p {@attach}/>",
            "<p use:actions..action/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod script_language_tests {
    use super::*;
    #[test]
    fn authored_script_languages_are_retained_independently() {
        let mixed=parse_svelte_module("<script module lang='ts'>const shared=1;</script><script lang='js'>const local=2;</script><p/>").unwrap();
        assert!(mixed.module_script_is_typescript);
        assert!(!mixed.script_is_typescript);
        let reverse=parse_svelte_module("<script context='module'>const shared=1;</script><script lang='ts'>const local:number=2;</script><p/>").unwrap();
        assert!(!reverse.module_script_is_typescript);
        assert!(reverse.script_is_typescript);
        let implicit = parse_svelte_module("<script>const local=2;</script><p/>").unwrap();
        assert!(!implicit.script_is_typescript);
        assert!(!implicit.module_script_is_typescript);
    }
}

#[cfg(test)]
mod await_tests {
    use super::*;
    #[test]
    fn await_structure_preserves_headers_patterns_and_nested_blocks() {
        let source = "{#await input}<b>pending</b>{:then {value:label='fallback',...rest}}{#if label}<i>{label}</i>{/if}{:catch error}<em>{error}</em>{/await}";
        let module = parse_svelte_module(source).unwrap();
        let marker = module.await_block_component.unwrap();
        assert!(module.template_jsx.contains(&marker));
        assert!(module.template_jsx.contains("thenPattern="));
        assert!(module.template_jsx.contains("catchPattern="));
        assert!(module.template_jsx.contains("initial="));
    }
    #[test]
    fn await_shorthand_is_retained_without_guessing_javascript() {
        let module =
            parse_svelte_module("{#await request then {value}}<b>{value}</b>{/await}").unwrap();
        assert!(module.template_jsx.contains("request then {value}"));
    }
    #[test]
    fn await_separators_accept_javascript_whitespace() {
        for source in [
            "{#await\tinput}<b/>{:then\tvalue}{value}{:catch\r\nerror}{error}{/await}",
            "{#await\r\ninput then value}{value}{/await}",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(module.await_block_component.is_some());
        }
    }
    #[test]
    fn await_duplicate_or_unclosed_arms_fail_closed() {
        for source in [
            "{#await input}<b/>",
            "{#await input}{:then value}{:then other}{/await}",
            "{#await input}{:catch error}{:then value}{/await}",
            "{#await input}{:catch error}{:catch other}{/await}",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod const_fragment_tests {
    use super::*;
    #[test]
    fn immediate_placement_is_explicit_and_fragments_share_one_marker() {
        for source in [
            "{@const x=1}",
            "<div>{@const x=1}</div>",
            "{#if true}<span>{@const x=1}</span>{/if}",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
        for source in [
            "{#if true}{@const x=1}{x}{/if}",
            "{#each [1] as v,i}{@const x=v+i}{x}{:else}{@const x=0}{x}{/each}",
            "{#key 1}{@const x=1}{x}{/key}",
            "{#await promise}{@const x=1}{x}{:then value}{@const x=value}{x}{:catch error}{@const x=error}{x}{/await}",
            "{#snippet row(value)}{@const x=value}{x}{/snippet}{@render row(1)}",
            "<Card>{@const x=1}<b>{x}</b><i>{x}</i></Card>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            let marker = module
                .const_scope_component
                .as_deref()
                .expect("explicit const marker");
            let templates = std::iter::once(module.template_jsx.as_str())
                .chain(
                    module
                        .snippets
                        .iter()
                        .map(|snippet| snippet.template_jsx.as_str()),
                )
                .collect::<String>();
            assert!(templates.contains(marker), "{source}: {templates}");
            assert!(templates.contains("body={()=>{const "), "{templates}");
            assert!(!templates.contains("})()"), "{templates}");
        }
    }
    #[test]
    fn nonconst_fragments_do_not_gain_const_metadata_or_wrappers() {
        let module = parse_svelte_module("{#if yes}<p>text</p>{/if}").unwrap();
        assert!(module.const_scope_component.is_none());
        let marker = module.intrinsic_namespace_attribute.as_deref().unwrap();
        assert_eq!(
            module.template_jsx,
            format!("<>{{(yes) ? <><p {marker}=\"http://www.w3.org/1999/xhtml\"><__CrepusText0 literal parts={{[\"text\"]}}/></p></> : null}}</>")
        );
    }
    #[test]
    fn const_only_component_children_are_not_dropped_beside_named_snippets() {
        let module = parse_svelte_module(
            "<Card>{@const x=side()}{#snippet heading()}title{/snippet}</Card>",
        )
        .unwrap();
        assert!(module.template_jsx.contains("body={()=>{const x=side()"));
        assert!(parse_svelte_module(
            "<Card>{@const x=side()}{#snippet children()}body{/snippet}</Card>"
        )
        .is_err());
    }
    #[test]
    fn only_sibling_named_snippets_are_carried_as_component_conflicts() {
        let module = parse_svelte_module(
            "<Card>{@const heading=1}{#snippet heading()}title{/snippet}</Card>",
        )
        .unwrap();
        assert!(
            module.template_jsx.contains("conflicts={[\"heading\"]}"),
            "{}",
            module.template_jsx
        );
        let snippet = parse_svelte_module(
            "{#snippet row(value)}{@const value=1}{value}{/snippet}{@render row(1)}",
        )
        .unwrap();
        assert!(!snippet.snippets[0].template_jsx.contains("conflicts="));
    }
    #[test]
    fn intrinsic_namespace_witness_precedes_snippet_and_control_relocation() {
        let module = parse_svelte_module("<svg>{#snippet row()}<a data-kind='snippet'/>{/snippet}{#if shown}<title/>{/if}<foreignObject><a/><svg><title/></svg></foreignObject></svg><a/>").unwrap();
        let marker = module.intrinsic_namespace_attribute.unwrap();
        let html = format!("{marker}=\"http://www.w3.org/1999/xhtml\"");
        let svg = format!("{marker}=\"http://www.w3.org/2000/svg\"");
        assert_eq!(
            module.template_jsx.matches(&html).count(),
            2,
            "{}",
            module.template_jsx
        );
        assert_eq!(
            module.template_jsx.matches(&svg).count(),
            6,
            "{}",
            module.template_jsx
        );
        assert!(
            module.template_jsx.contains(&format!("<a {svg} data-kind")),
            "{}",
            module.template_jsx
        );
    }

    #[test]
    fn intrinsic_reference_operands_keep_pairs_and_nonsequence_targets_parenthesized() {
        let module =
            parse_svelte_module("<div bind:this={get,set}/><span bind:this={cells[index]}/>")
                .unwrap();
        let marker = module.intrinsic_binding_attribute.unwrap();
        assert!(
            module
                .template_jsx
                .contains(&format!("{marker}={{(get,set)}}")),
            "{}",
            module.template_jsx
        );
        assert!(
            module
                .template_jsx
                .contains(&format!("{marker}={{(cells[index])}}")),
            "{}",
            module.template_jsx
        );
    }

    #[test]
    fn intrinsic_reference_marker_is_distinct_from_component_reference() {
        let module =
            parse_svelte_module("<div bind:this={node}/><Child bind:this={instance}/>").unwrap();
        let intrinsic = module.intrinsic_binding_attribute.unwrap();
        let component = module.component_binding_attribute.unwrap();
        assert_ne!(intrinsic, component);
        assert!(module
            .template_jsx
            .contains(&format!("{intrinsic}={{(node)}}")));
        assert!(module
            .template_jsx
            .contains(&format!("{component}={{instance}}")));
        assert!(parse_svelte_module("<div bind:this={node} bind:this={other}/>").is_err());
    }
}

#[cfg(test)]
mod head_title_tests {
    use super::*;

    #[test]
    fn accepted_head_and_title_forms() {
        for source in [
            "<svelte:head/>",
            "<svelte:head></svelte:head>",
            "<svelte:head><title/></svelte:head>",
            "<svelte:head><title></title></svelte:head>",
            "<svelte:head><title>one</title><title>two</title></svelte:head>",
            "<svelte:head>foo{\"bar\"}</svelte:head>",
            "<svelte:head><svg><title>SVG</title></svg></svelte:head>",
            "<title>body</title>",
        ] {
            assert!(parse_svelte_module(source).is_ok(), "{source}");
        }
    }

    #[test]
    fn rejected_head_and_title_forms() {
        for source in [
            "<svelte:head><meta name=\"a\"/></svelte:head><svelte:head><meta name=\"b\"/></svelte:head>",
            "<svelte:head/><svelte:head/>",
            "<svelte:head lang=\"en\"></svelte:head>",
            "<svelte:head {a}></svelte:head>",
            "<div><svelte:head><title>x</title></svelte:head></div>",
            "{#if true}<svelte:head><title>x</title></svelte:head>{/if}",
            "{#each items as item}<svelte:head/>{/each}",
            "<svelte:head><svelte:head/></svelte:head>",
            "<svelte:head><title lang=\"en\">x</title></svelte:head>",
            "<svelte:head><title {a}>x</title></svelte:head>",
            "<svelte:head><title><b>x</b></title></svelte:head>",
            "<svelte:head><title>{#if true}x{/if}</title></svelte:head>",
            "<svelte:head><title>{@html \"x\"}</title></svelte:head>",
            "<svelte:head><title>{@const x=1}{x}</title></svelte:head>",
            "<svelte:head><title><!--comment-->x</title></svelte:head>",
            "<svelte:head><title>x</svelte:head>",
            "<svelte:head><title>x</title>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn head_title_only_module_has_markers_without_ordinary_text_marker() {
        let module =
            parse_svelte_module("<svelte:head><title>one</title><title>two</title></svelte:head>")
                .unwrap();
        let head = module.head_component.unwrap();
        let title = module.title_component.unwrap();
        assert!(module.text_component.is_none(), "{}", module.template_jsx);
        assert_eq!(
            module.template_jsx,
            format!(
                "<><{head}><{title} parts={{[\"one\"]}}/><{title} parts={{[\"two\"]}}/></{head}></>"
            )
        );
        assert!(!module.template_jsx.contains("<title"));
        assert!(!module.template_jsx.contains(" literal"));
    }

    #[test]
    fn empty_titles_emit_empty_parts_and_mark_usage() {
        for source in [
            "<svelte:head><title/></svelte:head>",
            "<svelte:head><title></title></svelte:head>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            let title = module.title_component.unwrap();
            assert!(
                module
                    .template_jsx
                    .contains(&format!("<{title} parts={{[]}}/>")),
                "{}",
                module.template_jsx
            );
            assert!(module.text_component.is_none());
        }
    }

    #[test]
    fn self_closing_head_is_a_bare_marker() {
        let module = parse_svelte_module("<svelte:head/>").unwrap();
        let head = module.head_component.unwrap();
        assert_eq!(module.template_jsx, format!("<><{head}/></>"));
        assert!(module.title_component.is_none());
    }

    #[test]
    fn title_entities_keep_authored_spelling_and_are_never_literal() {
        let module = parse_svelte_module(
            "<svelte:head><title>&copy; {name} &amp;amp;</title></svelte:head>",
        )
        .unwrap();
        let title = module.title_component.unwrap();
        assert!(
            module.template_jsx.contains(&format!(
                "<{title} parts={{[{{text:\"&copy; \"}},()=> (name),{{text:\" &amp;amp;\"}}]}}/>"
            )),
            "{}",
            module.template_jsx
        );
        let module =
            parse_svelte_module("<svelte:head><title>a&amp;b</title></svelte:head>").unwrap();
        assert!(
            module.template_jsx.contains("parts={[{text:\"a&amp;b\"}]}"),
            "{}",
            module.template_jsx
        );
        assert!(!module.template_jsx.contains(" literal"));
    }

    #[test]
    fn title_expressions_share_one_marker() {
        let module =
            parse_svelte_module("<svelte:head><title>{a} - {b}</title></svelte:head>").unwrap();
        let title = module.title_component.unwrap();
        assert!(module
            .template_jsx
            .contains(&format!("<{title} parts={{[()=> (a),\" - \",()=> (b)]}}/>")));
        assert_eq!(
            module.template_jsx.matches(&format!("<{title} ")).count(),
            1
        );
    }

    #[test]
    fn head_children_use_ordinary_text_markers_in_source_order() {
        let module = parse_svelte_module(
            "<svelte:head>foo{\"bar\"}<meta name=\"a\"/><title>t</title></svelte:head>",
        )
        .unwrap();
        let head = module.head_component.unwrap();
        let text = module.text_component.unwrap();
        let title = module.title_component.unwrap();
        let jsx = &module.template_jsx;
        assert!(
            jsx.starts_with(&format!("<><{head}><{text} parts=")),
            "{jsx}"
        );
        let meta = jsx.find("<meta").unwrap();
        let marker = jsx.find(&format!("<{title} ")).unwrap();
        assert!(meta < marker, "{jsx}");
    }

    #[test]
    fn ordinary_and_svg_titles_stay_intrinsic() {
        for source in [
            "<title>body</title>",
            "<svelte:head><svg><title>SVG</title></svg></svelte:head>",
            "<svelte:head><div><title>x</title></div></svelte:head>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(module.title_component.is_none(), "{source}");
            assert!(module.template_jsx.contains("<title "), "{source}");
            assert!(module.text_component.is_some(), "{source}");
        }
    }

    #[test]
    fn title_after_head_is_ordinary_again() {
        let module = parse_svelte_module(
            "<svelte:head><title>x</title></svelte:head><title>y</title><svg><title>z</title></svg>",
        )
        .unwrap();
        let title = module.title_component.unwrap();
        assert_eq!(
            module.template_jsx.matches(&format!("<{title} ")).count(),
            1
        );
        assert_eq!(module.template_jsx.matches("<title ").count(), 2);
    }

    #[test]
    fn markers_avoid_source_collisions() {
        let module = parse_svelte_module(
            "<p>{__CrepusHead0}{__CrepusTitle0}</p><svelte:head><title>x</title></svelte:head>",
        )
        .unwrap();
        assert_eq!(module.head_component.as_deref(), Some("__CrepusHead1"));
        assert_eq!(module.title_component.as_deref(), Some("__CrepusTitle1"));
    }

    #[test]
    fn modules_without_head_expose_no_head_metadata() {
        let module = parse_svelte_module("<p>hello</p><title>t</title>").unwrap();
        assert!(module.head_component.is_none());
        assert!(module.title_component.is_none());
        assert!(!module.template_jsx.contains("__CrepusHead"));
        assert!(!module.template_jsx.contains("__CrepusTitle"));
    }
}

#[cfg(test)]
mod dynamic_element_tests {
    use super::*;

    const HTML: &str = "http://www.w3.org/1999/xhtml";
    const SVG: &str = "http://www.w3.org/2000/svg";

    fn parse_with_style(source: &str) -> Result<SvelteModule, CrepusError> {
        parse_svelte_module_with_styles_and_boundary(source, |src| {
            crate::parser::match_delimiter(src, '{', '}')
        })
    }

    fn opening(module: &SvelteModule, namespace: &str, selector: &str) -> String {
        format!(
            "<{} {}=\"{namespace}\" {}={{({selector})}}",
            module.element_component.as_deref().unwrap(),
            module.intrinsic_namespace_attribute.as_deref().unwrap(),
            module.element_tag_attribute.as_deref().unwrap(),
        )
    }

    #[test]
    fn expression_selector_lands_on_the_marker_with_ordinary_attributes() {
        let module = parse_svelte_module(
            "<svelte:element this={tag} class=\"box\" onclick={go}>x</svelte:element>",
        )
        .unwrap();
        let marker = module.element_component.as_deref().unwrap();
        let open = opening(&module, HTML, "tag");
        assert!(
            module
                .template_jsx
                .starts_with(&format!("<>{open} className=\"box\" onClick={{go}}>")),
            "{}",
            module.template_jsx
        );
        assert!(module.template_jsx.ends_with(&format!("</{marker}></>")));
        assert!(!module.template_jsx.contains("svelte:element"));
    }

    #[test]
    fn literal_and_sole_quoted_interpolation_selectors() {
        let literal = parse_svelte_module("<svelte:element this=\"section\"/>").unwrap();
        assert_eq!(
            literal.template_jsx,
            format!("<>{} /></>", opening(&literal, HTML, "\"section\""))
        );
        let quoted = parse_svelte_module("<svelte:element this=\"{tag}\"/>").unwrap();
        assert_eq!(
            quoted.template_jsx,
            format!("<>{} /></>", opening(&quoted, HTML, "tag"))
        );
        for source in [
            "<svelte:element this=\"\"/>",
            "<svelte:element this={\"\"}/>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert_eq!(
                module.template_jsx,
                format!("<>{} /></>", opening(&module, HTML, "\"\"")),
                "{source}"
            );
        }
    }

    #[test]
    fn only_the_first_this_is_the_selector() {
        let module = parse_svelte_module("<svelte:element this={a} this={b}/>").unwrap();
        assert_eq!(
            module.template_jsx,
            format!("<>{} this={{b}} /></>", opening(&module, HTML, "a"))
        );
        let module =
            parse_svelte_module("<svelte:element this=\"a\" this={b} bind:this={n}/>").unwrap();
        let reference = module.intrinsic_binding_attribute.as_deref().unwrap();
        assert_eq!(
            module.template_jsx,
            format!(
                "<>{} this={{b}} {reference}={{(n)}} /></>",
                opening(&module, HTML, "\"a\"")
            )
        );
    }

    #[test]
    fn missing_empty_or_mixed_this_is_rejected() {
        for source in [
            "<svelte:element>x</svelte:element>",
            "<svelte:element class=\"a\"/>",
            "<svelte:element this>x</svelte:element>",
            "<svelte:element this={}/>",
            "<svelte:element this={ }/>",
            "<svelte:element this=\"{}\"/>",
            "<svelte:element this=\"h{n}\"/>",
            "<svelte:element this=\"{a}{b}\"/>",
            "<svelte:element this=\" {a}\"/>",
            "<svelte:element {this}/>",
            "<svelte:element bind:this={n}/>",
            "<svelte:element this/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn source_closing_tag_matching_is_preserved() {
        for source in [
            "<svelte:element this={a}></div>",
            "<svelte:element this={a}></svelte:elemen>",
            "<svelte:element this={a}>x",
            "<div><svelte:element this={a}></div></svelte:element>",
            "<svelte:element this={a}></svelte:element></svelte:element>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
        let module = parse_svelte_module(
            "<svelte:element this=\"div\"><svelte:element this=\"span\">x</svelte:element></svelte:element>",
        )
        .unwrap();
        let marker = module.element_component.as_deref().unwrap();
        assert_eq!(
            module.template_jsx.matches(&format!("<{marker} ")).count(),
            2
        );
        assert_eq!(
            module.template_jsx.matches(&format!("</{marker}>")).count(),
            2
        );
    }

    #[test]
    fn existing_intrinsic_machinery_is_reused() {
        let module = parse_svelte_module(
            "<svelte:element this={tag} bind:this={node} use:tip={p} {@attach handle} onkeydown={k} style=\"color:red\"/>",
        )
        .unwrap();
        let reference = module.intrinsic_binding_attribute.as_deref().unwrap();
        let jsx = &module.template_jsx;
        assert!(jsx.contains(&format!("{reference}={{(node)}}")), "{jsx}");
        assert!(jsx.contains("onKeyDown={k}"), "{jsx}");
        assert!(jsx.contains("style=\"color:red\""), "{jsx}");
        assert_eq!(module.directives.len(), 2);
        assert_eq!(module.directives[0].action_parameter(), Some(true));
        assert_eq!(module.directives[1].action_parameter(), None);
        assert!(parse_svelte_module("<svelte:element this={a} {...props}/>").is_err());
        assert!(parse_svelte_module("<svelte:element this={a} onclick=\"go()\"/>").is_err());
        assert!(parse_svelte_module("<svelte:element this={a} use:tip use:tip/>").is_err());
    }

    #[test]
    fn type_specific_bindings_reject_unknown_tags() {
        for source in [
            "<svelte:element this=\"input\" bind:value={v}/>",
            "<svelte:element this=\"input\" bind:value/>",
            "<svelte:element this=\"textarea\" bind:value={v}></svelte:element>",
            "<svelte:element this=\"input\" type=\"checkbox\" bind:checked={c}/>",
            "<svelte:element this=\"input\" type=\"text\" bind:value={v}/>",
            "<svelte:element this={a} bind:group={g}/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
        let error = parse_svelte_module("<svelte:element this=\"input\" bind:value={v}/>")
            .unwrap_err()
            .to_string();
        assert!(error.contains("svelte:element"), "{error}");
    }

    #[test]
    fn lexical_namespace_witness_is_kept_and_children_do_not_follow_the_runtime_tag() {
        let module = parse_svelte_module("<svelte:element this={t}/>").unwrap();
        assert!(module.template_jsx.contains(&opening(&module, HTML, "t")));
        let module = parse_svelte_module("<svg><svelte:element this={t}/></svg>").unwrap();
        assert!(module.template_jsx.contains(&opening(&module, SVG, "t")));
        let module = parse_svelte_module(
            "<svg><foreignObject><svelte:element this={t}/></foreignObject></svg>",
        )
        .unwrap();
        assert!(module.template_jsx.contains(&opening(&module, HTML, "t")));
        let witness = module.intrinsic_namespace_attribute.as_deref().unwrap();
        let module =
            parse_svelte_module("<svelte:element this=\"svg\"><a/></svelte:element>").unwrap();
        let witness_html = module.intrinsic_namespace_attribute.as_deref().unwrap();
        assert_eq!(witness, witness_html);
        assert!(module
            .template_jsx
            .contains(&format!("<a {witness}=\"{HTML}\" />")));
        let module =
            parse_svelte_module("<svg><svelte:element this={t}><a/></svelte:element></svg>")
                .unwrap();
        assert!(module
            .template_jsx
            .contains(&format!("<a {witness}=\"{SVG}\" />")));
    }

    const MATH: &str = "http://www.w3.org/1998/Math/MathML";

    fn has(module: &SvelteModule, tag: &str, namespace: &str) -> bool {
        let witness = module.intrinsic_namespace_attribute.as_deref().unwrap();
        module
            .template_jsx
            .contains(&format!("<{tag} {witness}=\"{namespace}\""))
    }

    #[test]
    fn mathml_nested_descendants_and_dynamic_elements_get_mathml() {
        let module = parse_svelte_module(
            "<math><mrow><mfrac><mi>x</mi><svelte:element this={a}><mn/></svelte:element></mfrac></mrow></math><p/>",
        )
        .unwrap();
        for tag in ["math", "mrow", "mfrac"] {
            assert!(has(&module, tag, MATH), "{tag}");
        }
        assert!(module.template_jsx.contains(&opening(&module, MATH, "a")));
        assert!(has(&module, "p", HTML));
        let module = parse_svelte_module(
            "<svelte:element this={a} xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi/></svelte:element>",
        )
        .unwrap();
        assert!(module.template_jsx.contains(&opening(&module, MATH, "a")));
        assert!(has(&module, "mi", MATH));
    }

    #[test]
    fn mathml_text_integration_points_give_html_except_glyph_markers() {
        let module = parse_svelte_module(
            "<math><mi><b/><mglyph/><malignmark/><svg><a/></svg></mi><mtext><i/></mtext><mrow><b/></mrow></math>",
        )
        .unwrap();
        assert!(has(&module, "mi", MATH));
        assert!(has(&module, "b", HTML));
        assert!(has(&module, "mglyph", MATH));
        assert!(has(&module, "malignmark", MATH));
        assert!(has(&module, "svg", SVG));
        assert!(has(&module, "a", SVG));
        assert!(has(&module, "i", HTML));
        let module = parse_svelte_module(
            "<math><mi><b><mglyph/></b></mi><mglyph/><mo><math><mn/></math></mo></math>",
        )
        .unwrap();
        assert!(has(&module, "mglyph", MATH));
        assert!(has(&module, "mn", MATH));
        let module =
            parse_svelte_module("<math><mi>{#if x}<mglyph/><p/>{/if}</mi></math>").unwrap();
        assert!(has(&module, "mglyph", MATH));
        assert!(has(&module, "p", HTML));
    }

    #[test]
    fn standalone_mathml_component_roots_keep_their_own_namespace() {
        let module = parse_svelte_module("<mi>x</mi>").unwrap();
        assert!(has(&module, "mi", MATH));
        let module = parse_svelte_module("<div><mi>x</mi></div>").unwrap();
        assert!(has(&module, "mi", HTML));
    }

    #[test]
    fn logical_root_blocks_preserve_mathml_root_inference() {
        for source in [
            "{#if ok}<mi>x</mi>{/if}",
            "{#each items as item}<mi>{item}</mi>{/each}",
            "{#snippet s()}<mi>x</mi>{/snippet}{@render s()}",
            "{#key value}<mi>x</mi>{/key}",
            "<>{#if ok}<mi>x</mi>{/if}</>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            let witness = module.intrinsic_namespace_attribute.as_deref().unwrap();
            let expected = format!("<mi {witness}=\"{MATH}\"");
            assert!(
                has(&module, "mi", MATH)
                    || module
                        .snippets
                        .iter()
                        .any(|snippet| snippet.template_jsx.contains(&expected)),
                "{source}"
            );
        }
        for source in [
            "<div>{#if ok}<mi>x</mi>{/if}</div>",
            "<div>{#each items as item}<mi>{item}</mi>{/each}</div>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(has(&module, "mi", HTML), "{source}");
        }
    }

    #[test]
    fn snippets_in_html_parents_infer_their_own_mathml_roots() {
        let module = parse_svelte_module(
            "<div>{#snippet s()}<mi>x</mi>{/snippet}<math>{@render s()}</math><mi>y</mi></div>",
        )
        .unwrap();
        assert_eq!(module.inline_snippet_names.len(), 1);
        assert!(has(&module, "mi", MATH));
        assert!(has(&module, "mi", HTML));
    }

    #[test]
    fn mathml_does_not_treat_svg_looking_names_as_namespace_switches() {
        let module=parse_svelte_module("<math><g/><foreignObject><mi/></foreignObject><mtext><g/><math><mi/></math></mtext><svg><g/><math/></svg></math>").unwrap();
        assert!(has(&module, "g", MATH));
        assert!(has(&module, "foreignObject", MATH));
        assert!(has(&module, "g", HTML));
        assert!(has(&module, "g", SVG));
        assert!(has(&module, "math", SVG));
    }

    #[test]
    fn annotation_xml_encoding_selects_child_namespace_without_dropping_children() {
        for encoding in [
            "text/html",
            "TEXT/HTML",
            "application/xhtml+xml",
            "Application/XHTML+XML",
        ] {
            let module = parse_svelte_module(&format!(
                "<math><annotation-xml encoding=\"{encoding}\"><p/><svg><a/></svg></annotation-xml></math>"
            ))
            .unwrap();
            assert!(has(&module, "annotation-xml", MATH));
            assert!(has(&module, "p", HTML), "{encoding}");
            assert!(has(&module, "a", SVG));
        }
        for source in [
            "<math><annotation-xml encoding=\"application/x-tex\"><mi/><svg><a/></svg></annotation-xml></math>",
            "<math><annotation-xml><mi/><svg><a/></svg></annotation-xml></math>",
            "<math><annotation-xml encoding><mi/><svg><a/></svg></annotation-xml></math>",
        ] {
            let module = parse_svelte_module(source).unwrap();
            assert!(has(&module, "mi", MATH), "{source}");
            assert!(has(&module, "a", SVG), "{source}");
        }
        for source in [
            "<math><annotation-xml encoding={e}><p/></annotation-xml></math>",
            "<math><annotation-xml encoding=\"text/{e}\"><p/></annotation-xml></math>",
            "<math><annotation-xml {encoding}><p/></annotation-xml></math>",
            "<math><annotation-xml {...props}><p/></annotation-xml></math>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
        assert!(parse_svelte_module(
            "<div><annotation-xml encoding={e}><p/></annotation-xml></div>"
        )
        .is_ok());
    }

    #[test]
    fn mathml_namespace_does_not_cross_component_siblings_or_regress_html_svg() {
        let module =
            parse_svelte_module("<math><mi/></math><Card/><p/><svg><a/></svg><b/>").unwrap();
        assert!(has(&module, "p", HTML));
        assert!(has(&module, "b", HTML));
        assert!(has(&module, "a", SVG));
        let module =
            parse_svelte_module("<svg><foreignObject><p/><math><mi/></math></foreignObject></svg>")
                .unwrap();
        assert!(has(&module, "p", HTML));
        assert!(has(&module, "mi", MATH));
        let module = parse_svelte_module("<math><mi/></math><mglyph/>").unwrap();
        assert!(has(&module, "mglyph", HTML));
        assert!(has(&module, "mi", MATH));
    }

    #[test]
    fn dynamic_xmlns_emits_a_private_marker_beside_the_authored_attribute() {
        let module = parse_svelte_module("<svelte:element this={a} xmlns={ns}/>").unwrap();
        let marker = module.element_namespace_attribute.as_deref().unwrap();
        assert_eq!(marker, "__crepusElementNamespace0");
        assert_eq!(
            module.template_jsx,
            format!(
                "<>{} {marker}={{(ns\n)}} xmlns={{ns}} /></>",
                opening(&module, HTML, "a")
            )
        );
        let module = parse_svelte_module("<svelte:element this={a} {xmlns}/>").unwrap();
        let marker = module.element_namespace_attribute.as_deref().unwrap();
        assert!(module.template_jsx.contains(&format!("{marker}={{xmlns}}")));
        let module = parse_svelte_module("<svelte:element this={a} xmlns=\"{ns}\"/>").unwrap();
        let marker = module.element_namespace_attribute.as_deref().unwrap();
        assert!(module
            .template_jsx
            .contains(&format!(" {marker}={{ns}} xmlns={{ns}}")));
        assert!(!module.template_jsx.contains("String("));
        let module = parse_svelte_module("<svelte:element this={a} xmlns=\"u:{ns}\"/>").unwrap();
        let marker = module.element_namespace_attribute.as_deref().unwrap();
        assert!(module.template_jsx.contains(&format!(" {marker}={{")));
        assert!(module.attribute_coercion_function.is_some());
        assert!(module.template_jsx.contains(&opening(&module, HTML, "a")));
    }

    #[test]
    fn static_svg_xmlns_gives_children_the_svg_namespace() {
        let module = parse_svelte_module(
            "<svelte:element this={a} xmlns=\"http://www.w3.org/2000/svg\"><b/></svelte:element>",
        )
        .unwrap();
        assert!(module.element_namespace_attribute.is_none());
        assert!(module.template_jsx.contains(&opening(&module, SVG, "a")));
        let witness = module.intrinsic_namespace_attribute.as_deref().unwrap();
        assert!(module
            .template_jsx
            .contains(&format!("<b {witness}=\"{SVG}\"")));
    }

    #[test]
    fn static_non_svg_xmlns_follows_the_official_html_reset_without_a_marker() {
        for xmlns in [HTML, "http://example.com/x", ""] {
            let module = parse_svelte_module(&format!(
                "<svg><svelte:element this={{a}} xmlns=\"{xmlns}\"><b/></svelte:element></svg>"
            ))
            .unwrap();
            assert!(module.element_namespace_attribute.is_none());
            assert!(module.template_jsx.contains(&opening(&module, HTML, "a")));
        }
        let module =
            parse_svelte_module(&format!("<svelte:element this={{a}} xmlns=\"{HTML}\"/>")).unwrap();
        assert!(module.template_jsx.contains(&opening(&module, HTML, "a")));
        assert!(module.element_namespace_attribute.is_none());
    }

    #[test]
    fn valueless_xmlns_uses_the_boolean_namespace_value() {
        let module = parse_svelte_module("<svelte:element this={tag} xmlns/>").unwrap();
        let marker = module.element_namespace_attribute.as_deref().unwrap();
        assert!(module.template_jsx.contains(&format!("{marker}={{true}}")));
        assert!(module.template_jsx.contains(" xmlns={true}"));
    }

    #[test]
    fn xmlns_duplicates_and_empty_expressions_are_errors() {
        for source in [
            "<svelte:element this={a} xmlns=\"x\" xmlns=\"y\"/>",
            "<svelte:element this={a} xmlns={n} xmlns=\"y\"/>",
            "<svelte:element this={a} {xmlns} xmlns={n}/>",
            "<svelte:element this={a} xmlns={}/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn namespace_marker_is_fresh_and_absent_without_dynamic_xmlns() {
        let module = parse_svelte_module(
            "<script>const __crepusElementNamespace0=1;</script><svelte:element this={a} xmlns={n}/>",
        )
        .unwrap();
        assert_eq!(
            module.element_namespace_attribute.as_deref(),
            Some("__crepusElementNamespace1")
        );
        let module = parse_svelte_module("<svelte:element this={a} class=\"c\"/>").unwrap();
        assert!(module.element_namespace_attribute.is_none());
        assert!(!module.template_jsx.contains("ElementNamespace"));
    }

    #[test]
    fn other_svelte_tags_keep_their_diagnostics() {
        for source in [
            "<svelte:self/>",
            "<svelte:component this={C}/>",
            "<svelte:window/>",
            "<svelte:fragment/>",
            "<svelte:element-x this={a}/>",
            "<svelte:/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn scoped_css_is_retained_for_a_dynamic_element() {
        let module = parse_with_style(
            "<svelte:element this={t} class=\"inner\"/><style>.inner{color:red}</style>",
        )
        .unwrap();
        assert!(module
            .style
            .as_deref()
            .is_some_and(|css| css.contains(".inner")));
        assert!(module.element_component.is_some());
        assert!(parse_with_style("<svelte:element this={t}/>").is_ok());
        assert!(
            parse_svelte_module("<svelte:element this={t}/><style>p{color:red}</style>").is_err()
        );
    }

    #[test]
    fn markers_are_fresh_private_and_absent_without_dynamic_elements() {
        let module = parse_svelte_module(
            "<script>const __CrepusElement0=1;const __crepusElementTag0=2;</script><svelte:element this={a}/>",
        )
        .unwrap();
        assert_eq!(
            module.element_component.as_deref(),
            Some("__CrepusElement1")
        );
        assert_eq!(
            module.element_tag_attribute.as_deref(),
            Some("__crepusElementTag1")
        );
        let module = parse_svelte_module("<p>x</p>").unwrap();
        assert!(module.element_component.is_none());
        assert!(module.element_tag_attribute.is_none());
    }

    #[test]
    fn class_directives_emit_one_helper_call_with_shorthand_and_expression_flags() {
        let module = parse_svelte_module(
            "<p class=\"x {y}\" class:on={ok} class:b=\"{!z}\" class:c class:a-b={q, r}>t</p>",
        )
        .unwrap();
        let helper = module.class_directive_helper.as_deref().unwrap();
        let coerce = module.attribute_coercion_function.as_deref().unwrap();
        assert_eq!(helper, "__CrepusClassDirectives0");
        let expected = format!(
            " className={{{helper}((\"\" + \"x \" + {coerce}((y))\n),null,{{[\"on\"]:(ok\n),[\"b\"]:(!z\n),[\"c\"]:c,[\"a-b\"]:(q, r\n)}})}}"
        );
        assert!(
            module.template_jsx.contains(&expected),
            "{}",
            module.template_jsx
        );
        assert_eq!(module.template_jsx.matches("className=").count(), 1);
    }

    #[test]
    fn class_directive_base_is_undefined_when_absent_and_preserves_every_form() {
        let module = parse_svelte_module("<div class:a/>").unwrap();
        let helper = module.class_directive_helper.as_deref().unwrap();
        assert!(
            module.template_jsx.contains(&format!(
                " className={{{helper}(undefined,null,{{[\"a\"]:a}})}} />"
            )),
            "{}",
            module.template_jsx
        );
        for (source, base) in [
            ("<p class=\"a &amp; b\" class:x/>", "\"a & b\""),
            ("<p class class:x/>", "\"\""),
            ("<p class={a, b} class:x/>", "(a, b\n)"),
            ("<p class=\"{a}\" class:x/>", "(a\n)"),
            ("<p class:x class=\"plain\"/>", "\"plain\""),
        ] {
            let module = parse_svelte_module(source).unwrap();
            let helper = module.class_directive_helper.as_deref().unwrap();
            assert!(
                module.template_jsx.contains(&format!(
                    " className={{{helper}({base},null,{{[\"x\"]:x}})}}"
                )),
                "{source}: {}",
                module.template_jsx
            );
        }
    }

    #[test]
    fn class_directive_static_and_interpolated_entities_share_reference_rules() {
        for (authored, expected) in [
            ("&copy;", "©"),
            ("&amp;", "&"),
            ("&#65;", "A"),
            ("&#x41;", "A"),
            ("&copy", "©"),
            ("&copyx", "&copyx"),
            ("&copy=", "&copy="),
            ("&bogus;", "&bogus;"),
            ("&#0;", "&#0;"),
            ("&#128;", "€"),
            ("&#xD800;", "\0"),
            ("&amp;copy;", "&copy;"),
        ] {
            for source in [
                format!("<p class=\"{authored}\" class:active={{flag}}/>"),
                format!("<p class=\"{{value}}{authored}\" class:active={{flag}}/>"),
            ] {
                let module = parse_svelte_module(&source).unwrap();
                let encoded = serde_json::to_string(expected).unwrap();
                assert!(
                    module.template_jsx.contains(&encoded),
                    "{source}: {}",
                    module.template_jsx
                );
            }
        }
    }

    #[test]
    fn class_directives_keep_authored_base_attribute_position() {
        let module = parse_svelte_module(
            r#"<div id="before" class="base" title="after" class:active={flag}/>"#,
        )
        .unwrap();
        let jsx = module.template_jsx;
        assert!(jsx.find("id=").unwrap() < jsx.find("className=").unwrap());
        assert!(jsx.find("className=").unwrap() < jsx.find("title=").unwrap());
    }

    #[test]
    fn class_directives_apply_to_dynamic_elements_and_keep_other_attributes() {
        let module = parse_svelte_module(
            "<svelte:element this={t} class=\"box\" class:a={b} onclick={go} style=\"color:red\"/>",
        )
        .unwrap();
        let helper = module.class_directive_helper.as_deref().unwrap();
        let jsx = &module.template_jsx;
        assert!(jsx.contains(&format!(
            " className={{{helper}(\"box\",null,{{[\"a\"]:(b\n)}})}}"
        )));
        assert!(jsx.contains(" onClick={go}"), "{jsx}");
        assert!(jsx.contains(" style=\"color:red\""), "{jsx}");
        assert!(module.element_component.is_some());
    }

    #[test]
    fn flags_keep_source_order_and_use_computed_quoted_keys() {
        let module =
            parse_svelte_module("<p class:z={1} class:__proto__={x} class:a={2}/>").unwrap();
        assert!(
            module
                .template_jsx
                .contains("{[\"z\"]:(1\n),[\"__proto__\"]:(x\n),[\"a\"]:(2\n)}"),
            "{}",
            module.template_jsx
        );
    }

    #[test]
    fn class_without_directives_is_unchanged_and_exposes_no_helper() {
        let module = parse_svelte_module("<p class=\"a\" class:/>");
        assert!(module.is_err());
        let module = parse_svelte_module("<p class=\"a {b}\"/><i class={c}/>").unwrap();
        assert!(module.class_directive_helper.is_none());
        assert!(!module.template_jsx.contains("__CrepusClassDirectives"));
        assert!(module.template_jsx.contains(" className={\"\" + \"a \" + "));
        assert!(module.template_jsx.contains(" className={c}"));
        let module = parse_svelte_module("<p class=\"a\"/>").unwrap();
        assert!(module.template_jsx.contains(" className=\"a\""));
    }

    #[test]
    fn class_directive_helper_is_fresh_private_and_unique_per_module() {
        let module = parse_svelte_module(
            "<script>const __CrepusClassDirectives0=1;</script><p class:a/><i class:b/>",
        )
        .unwrap();
        let helper = module.class_directive_helper.as_deref().unwrap();
        assert_eq!(helper, "__CrepusClassDirectives1");
        assert_eq!(
            module.template_jsx.matches(&format!("{helper}(")).count(),
            2
        );
    }

    #[test]
    fn invalid_class_directives_are_rejected() {
        for source in [
            "<p class:a class:a/>",
            "<p class:a class:a={b}/>",
            "<p class:a=\"true\"/>",
            "<p class:a=\"\"/>",
            "<p class:a=true/>",
            "<p class:a=\"x {b}\"/>",
            "<p class:a=\"{b}{c}\"/>",
            "<p class:a={}/>",
            "<p class:a=\"{}\"/>",
            "<p class:/>",
            "<p class:={x}/>",
            "<p class:a|local/>",
            "<p class:a-b/>",
            "<p class:if/>",
            "<Child class:a/>",
            "<Child class:a={b}/>",
            "<ns.Child class:a/>",
            "<p class:a class=\"x\" class=\"y\"/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod group_contract_tests {
    use super::parse_svelte_module;
    #[test]
    fn groups_retain_targets_and_allocate_hygienic_markers() {
        let source = "<script>const __crepusGroupBinding0=0;</script><input type=\"checkbox\" bind:group={rows[i].selected}/><input type=\"radio\" bind:group/>";
        let module = parse_svelte_module(source).unwrap();
        let marker = module.group_binding_attribute.unwrap();
        assert_ne!(marker, "__crepusGroupBinding0");
        assert!(module
            .template_jsx
            .contains(&format!("{marker}={{rows[i].selected}}")));
        assert!(module.template_jsx.contains(&format!("{marker}={{group}}")));
    }
    #[test]
    fn invalid_group_elements_types_duplicates_and_values_are_rejected() {
        for source in [
            "<div bind:group={x}/>",
            "<input bind:group={x}/>",
            "<input {...props} bind:group={x}/>",
            "<input type=\"text\" bind:group={x}/>",
            "<input type={kind} bind:group={x}/>",
            "<input type=\"radio\" bind:group={x} bind:group={y}/>",
            "<input type=\"checkbox\" bind:group=\"x\"/>",
        ] {
            assert!(parse_svelte_module(source).is_err(), "{source}");
        }
        assert!(parse_svelte_module("<div/>")
            .unwrap()
            .group_binding_attribute
            .is_none());
    }
}

#[cfg(test)]
mod intrinsic_spread_admission_tests {
    use super::parse_svelte_module;

    #[test]
    fn ordinary_intrinsic_spreads_preserve_layers_and_authored_order() {
        let output = parse_svelte_module(
            "<button title=\"before\" {...first} disabled={busy} {...second}>text</button>",
        )
        .unwrap()
        .template_jsx;
        assert!(output.contains("{...(first)}"), "{output}");
        assert!(output.contains("{...(second)}"), "{output}");
        assert!(output.find("before").unwrap() < output.find("first").unwrap());
        assert!(output.find("first").unwrap() < output.find("busy").unwrap());
        assert!(output.find("busy").unwrap() < output.find("second").unwrap());
    }

    #[test]
    fn form_intrinsic_spreads_are_preserved() {
        for source in [
            "<input {...props}/>",
            "<input type=\"checkbox\" {...props} bind:group={items}/>",
            "<input type=\"radio\" {...props} bind:group={choice}/>",
            "<select {...props}/>",
            "<option {...props}/>",
            "<textarea {...props}/>",
        ] {
            assert!(parse_svelte_module(source)
                .unwrap()
                .template_jsx
                .contains("{...(props)}"));
        }
    }

    #[test]
    fn unverified_intrinsic_spread_combinations_remain_explicit_errors() {
        for source in [
            "<title {...props}/>",
            "<svelte:element this={tag} {...props}/>",
            "<svg><path {...props}/></svg>",
            "<math><mi {...props}/></math>",
            "<custom-element {...props}/>",
            "<button {...props} class:active={flag}/>",
            "<button use:action {...props}/>",
            "<button {...props} {@attach attachment}/>",
            "<button {...props} bind:this={node}/>",
            "<button {...props} xmlns=\"x\"/>",
            "<button {...props} is=\"custom-button\"/>",
        ] {
            let error = parse_svelte_module(source).unwrap_err().to_string();
            assert!(
                error.contains("spread") || error.contains("unsupported authored Svelte tag"),
                "{source}: {error}"
            );
        }
    }
}
