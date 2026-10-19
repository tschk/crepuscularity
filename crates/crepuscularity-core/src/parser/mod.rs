/// Runtime parser for the crepuscularity template DSL.
/// Mirrors the compile-time proc-macro parser but operates on strings at runtime.
mod angular;
mod astro;
mod indent;
mod jsx;
mod svelte;
mod vue;

use std::collections::HashMap;

use crate::ast::*;

pub use astro::{
    parse_astro_module_template, parse_astro_module_template_with_expression_validator,
    split_astro_source, AstroFrontmatter, AstroSource,
};
pub use indent::{parse_when_attribute_suffix, unescape_crepus_text_literal};
pub use vue::parse_vue_native_file;
pub use vue::{
    parse_vue_sfc, parse_vue_sfc_with_generics, parse_vue_sfc_with_metadata, VueScriptBlock,
    VueSfc, VueSfcMetadata, VueStyleBlock,
};

pub(crate) use indent::{collect_lines, parse_nodes};
pub(crate) use jsx::parse_jsx_template;
pub use svelte::entities as svelte_entities;
pub(crate) use svelte::parse_svelte_template;
pub use svelte::{
    parse_svelte_component, parse_svelte_module, parse_svelte_module_with_boundary,
    parse_svelte_module_with_styles_and_boundary, SvelteComponent, SvelteModule, SvelteSnippet,
};

#[derive(Debug, Clone)]
pub(crate) struct RawParseError {
    pub message: String,
    pub byte_offset: Option<usize>,
}

/// Encode a literal value as a string expression without interpreting escapes.
pub(crate) fn quote_expression_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Protect known literal class tokens from the shared interpolation representation.
pub(crate) fn literal_class_token(value: &str) -> String {
    if value.contains('{') {
        format!("{{{}}}", quote_expression_string(value))
    } else {
        value.to_owned()
    }
}

/// Byte index of the delimiter matching the one at the start of `src`,
/// ignoring delimiters that appear inside string literals.
pub(crate) fn match_delimiter(src: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in src.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Run `f` over every top-level byte position of `src`, skipping string
/// literals and bracketed groups.
pub(crate) fn scan_top_level(src: &str, mut f: impl FnMut(usize, char) -> bool) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in src.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            _ if depth == 0 && f(i, c) => return Some(i),
            _ => {}
        }
    }
    None
}

/// A diagnostic prefix that never splits a UTF-8 character.
pub(crate) fn diagnostic_prefix(src: &str, max_chars: usize) -> &str {
    let end = src
        .char_indices()
        .nth(max_chars)
        .map_or(src.len(), |(i, _)| i);
    &src[..end]
}

pub(crate) fn subslice_byte_offset(full: &str, tail: &str) -> usize {
    let fp = full.as_ptr() as usize;
    let tp = tail.as_ptr() as usize;
    debug_assert!(tp >= fp && tp <= fp.saturating_add(full.len()));
    tp.saturating_sub(fp)
}

// ── Multi-component files ─────────────────────────────────────────────────────

/// Metadata and default prop values for one component parsed from TOML frontmatter.
#[derive(Debug, Clone, Default)]
pub struct ComponentMeta {
    /// Description string (from `description = "..."` in TOML).
    pub description: Option<String>,
    /// Default prop values as evaluable expression strings.
    /// `[Card.defaults]` section: `title = "Untitled"` → `"title" → "\"Untitled\""`
    pub defaults: HashMap<String, String>,
}

/// A named component extracted from a multi-component file.
#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub nodes: Vec<Node>,
    pub meta: ComponentMeta,
}

/// Parsed multi-component `.crepus` file.
///
/// A multi-component file starts with an optional `+++...+++` TOML frontmatter
/// block, followed by one or more component sections introduced by `--- Name`.
///
/// ```text
/// +++
/// [Card]
/// description = "A simple card"
///
/// [Card.defaults]
/// title = "Untitled"
/// subtitle = ""
///
/// [Button]
/// description = "A clickable button"
///
/// [Button.defaults]
/// label = "Click me"
/// variant = "primary"
/// +++
///
/// --- Card
/// div rounded-lg border p-4 mb-2
///   div font-bold text-lg
///     {title}
///   div text-sm text-gray-400
///     {subtitle}
///   slot
///
/// --- Button
/// $: default variant = "primary"
/// button px-4 py-2 rounded
///   {label}
/// ```
///
/// Components are then included with `include components.crepus#Card title="Hello"`.
#[derive(Debug)]
pub struct ComponentFile {
    pub components: HashMap<String, ComponentDef>,
}

/// Parse a multi-component `.crepus` file into a [`ComponentFile`].
#[tracing::instrument(skip(content), fields(len = content.len()))]
pub fn parse_component_file(content: &str) -> Result<ComponentFile, crate::error::CrepusError> {
    parse_component_file_inner(content).map_err(|e| crate::error::CrepusError::parse(e.message))
}

pub(crate) fn parse_component_file_inner(content: &str) -> Result<ComponentFile, RawParseError> {
    let (frontmatter_str, body, body_byte_in_file) = split_frontmatter_parts(content);
    let trimmed = content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .trim_start();
    if frontmatter_str.is_none() && trimmed.starts_with("+++") {
        return Err(RawParseError {
            message: "unclosed component frontmatter".to_string(),
            byte_offset: Some(subslice_byte_offset(content, trimmed)),
        });
    }

    let mut meta_map: HashMap<String, ComponentMeta> = HashMap::new();
    if let Some(toml_src) = frontmatter_str {
        let toml_block_start = subslice_byte_offset(content, toml_src);
        let value: toml::Value =
            toml::from_str(toml_src).map_err(|e: toml::de::Error| RawParseError {
                message: format!("TOML parse error in frontmatter: {e}"),
                byte_offset: e.span().map(|r| toml_block_start + r.start),
            })?;

        if let toml::Value::Table(table) = value {
            for (comp_name, comp_val) in &table {
                if let toml::Value::Table(comp_table) = comp_val {
                    let mut meta = ComponentMeta::default();

                    if let Some(description) = comp_table.get("description") {
                        let desc = description.as_str().ok_or_else(|| RawParseError {
                            message: format!(
                                "component {comp_name}.description must be a TOML string"
                            ),
                            byte_offset: Some(toml_block_start),
                        })?;
                        meta.description = Some(desc.to_string());
                    }

                    if let Some(defaults) = comp_table.get("defaults") {
                        let defs = defaults.as_table().ok_or_else(|| RawParseError {
                            message: format!("component {comp_name}.defaults must be a TOML table"),
                            byte_offset: Some(toml_block_start),
                        })?;
                        for (k, v) in defs {
                            let expression =
                                toml_value_to_expr(v).map_err(|message| RawParseError {
                                    message: format!(
                                        "invalid component default {comp_name}.{k}: {message}"
                                    ),
                                    byte_offset: Some(toml_block_start),
                                })?;
                            meta.defaults.insert(k.clone(), expression);
                        }
                    }

                    meta_map.insert(comp_name.clone(), meta);
                }
            }
        }
    }

    let sections = split_component_body_sections(body);
    let mut components = HashMap::new();

    for (name, section_content, sec_start_in_body) in sections {
        if components.contains_key(&name) {
            return Err(RawParseError {
                message: format!("duplicate component name {name:?}"),
                byte_offset: Some(body_byte_in_file + sec_start_in_body),
            });
        }
        let nodes = parse_template_raw(&section_content).map_err(|mut e| {
            if let Some(off) = e.byte_offset {
                e.byte_offset = Some(body_byte_in_file + sec_start_in_body + off);
            }
            if e.byte_offset.is_some() {
                e.message = format!("component {name:?}: {}", e.message);
            }
            e
        })?;
        let meta = meta_map.remove(&name).unwrap_or_default();
        components.insert(name, ComponentDef { nodes, meta });
    }

    Ok(ComponentFile { components })
}

pub fn split_frontmatter_parts(content: &str) -> (Option<&str>, &str, usize) {
    let trimmed = content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .trim_start();
    if !trimmed.starts_with("+++") {
        return (None, content, 0);
    }
    let after_open = &trimmed[3..];
    if let Some(close_pos) = after_open
        .match_indices("\n+++")
        .map(|(position, _)| position)
        .find(|&position| after_open[position + 1..].lines().next() == Some("+++"))
    {
        let raw_toml = &after_open[..close_pos];
        let rest = &after_open[close_pos + 4..];
        let body_start = subslice_byte_offset(content, rest);
        (Some(raw_toml), rest, body_start)
    } else {
        (None, content, 0)
    }
}

pub fn split_component_body_sections(body: &str) -> Vec<(String, String, usize)> {
    let mut sections: Vec<(String, String, usize)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();
    let mut content_start_byte: Option<usize> = None;
    let mut pending_content_start = 0usize;
    let mut in_text = false;

    let mut byte_pos = 0usize;
    while byte_pos <= body.len() {
        let line_start = byte_pos;
        if line_start >= body.len() {
            break;
        }
        let nl = body[line_start..].find('\n');
        let line_end = nl.map(|n| line_start + n).unwrap_or(body.len());
        let raw_line = &body[line_start..line_end];
        byte_pos = if nl.is_some() {
            line_end + 1
        } else {
            body.len()
        };

        let line_content = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed = if line_start == 0 {
            line_content
                .strip_prefix('\u{feff}')
                .unwrap_or(line_content)
        } else {
            line_content
        }
        .trim();

        let literal_line = current_name.is_some() && (in_text || trimmed.starts_with('"'));
        if literal_line {
            let text = if in_text { trimmed } else { &trimmed[1..] };
            in_text = true;
            let mut escaped = false;
            for character in text.chars() {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_text = false;
                    break;
                }
            }
        }
        let section_name = if literal_line {
            None
        } else {
            trimmed
                .strip_prefix("---")
                .filter(|rest| rest.starts_with(char::is_whitespace))
        };

        if let Some(name) = section_name {
            if let Some(prev) = current_name.take() {
                let joined = current_lines.join("\n");
                let start = content_start_byte.unwrap_or(pending_content_start);
                sections.push((prev, joined, start));
                current_lines.clear();
            }
            current_name = Some(name.trim().to_string());
            content_start_byte = None;
            pending_content_start = byte_pos;
        } else if current_name.is_some() {
            if content_start_byte.is_none() {
                content_start_byte = Some(line_start);
            }
            current_lines.push(line_content);
        }
    }

    if let Some(name) = current_name {
        let joined = current_lines.join("\n");
        let start = content_start_byte.unwrap_or(pending_content_start);
        sections.push((name, joined, start));
    }

    sections
}

/// Convert a TOML scalar to an expression string usable by the evaluator.
fn toml_value_to_expr(v: &toml::Value) -> Result<String, &'static str> {
    Ok(match v {
        toml::Value::String(s) => quote_expression_string(s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => {
            if !f.is_finite() {
                return Err("float values must be finite");
            }
            let literal = f.to_string();
            // Display omits the decimal point for integral floats. Preserve
            // their type so large finite values don't become i64 literals.
            if !literal.contains('.') {
                format!("{literal}.0")
            } else {
                literal
            }
        }
        toml::Value::Boolean(b) => b.to_string(),
        _ => return Err("only string, integer, float, and boolean values are supported"),
    })
}

#[tracing::instrument(skip(template), fields(len = template.len()))]
pub fn parse_template(template: &str) -> Result<Vec<Node>, crate::error::CrepusError> {
    parse_template_with_path(template, None)
}

/// Parse a template, using the optional file path to select its frontend.
/// Without a frontend-specific path, content selects JSX or indentation syntax.
pub fn parse_template_with_path(
    template: &str,
    path: Option<&std::path::Path>,
) -> Result<Vec<Node>, crate::error::CrepusError> {
    parse_template_raw_with_path(template, path)
        .map_err(|e| crate::error::CrepusError::parse(e.message))
}

/// Keep inline spaces while removing formatting indentation at line boundaries.
pub(crate) fn markup_text_region(text: &str, nested: bool) -> &str {
    if !nested {
        return text.trim();
    }
    let trimmed_start = text.trim_start();
    let leading = &text[..text.len() - trimmed_start.len()];
    let trimmed_end = text.trim_end();
    let trailing = &text[trimmed_end.len()..];
    let first = if leading.contains(['\n', '\r']) {
        leading.len()
    } else {
        0
    };
    let last = if trailing.contains(['\n', '\r']) {
        trimmed_end.len()
    } else {
        text.len()
    };
    if first >= last {
        &text[..0]
    } else {
        &text[first..last]
    }
}

/// Spaces before a tag/block belong to nested markup, but root padding does not.
pub(crate) fn markup_space_prefix(src: &str, nested: bool) -> Option<crate::ast::Node> {
    let prefix = &src[..src.len() - src.trim_start().len()];
    (nested && !prefix.is_empty() && !prefix.contains(['\n', '\r']))
        .then(|| crate::ast::Node::Text(vec![crate::ast::TextPart::Literal(prefix.into())]))
}

pub(crate) fn parse_template_raw(template: &str) -> Result<Vec<Node>, RawParseError> {
    parse_template_raw_with_path(template, None)
}

pub(crate) fn parse_template_raw_with_path(
    template: &str,
    path: Option<&std::path::Path>,
) -> Result<Vec<Node>, RawParseError> {
    // Vue's SFC splitter already handles one leading BOM.
    let source = if is_vue_mode(path) {
        template
    } else {
        template.strip_prefix('\u{feff}').unwrap_or(template)
    };
    let offset = template.len() - source.len();
    parse_template_raw_with_path_inner(source, path).map_err(|mut error| {
        error.byte_offset = error.byte_offset.map(|byte| byte.saturating_add(offset));
        error
    })
}

fn parse_template_raw_with_path_inner(
    template: &str,
    path: Option<&std::path::Path>,
) -> Result<Vec<Node>, RawParseError> {
    if is_vue_mode(path) {
        vue::parse_vue_file(template)
    } else if is_svelte_mode(path) {
        parse_svelte_template(template)
    } else if is_astro_mode(path) {
        astro::parse_astro_template(template)
    } else if is_angular_mode(path) {
        angular::parse_angular_template(template)
    } else if is_jsx_mode(template, path) {
        parse_jsx_template(template)
    } else {
        let dec = crate::preprocess::strip_indent_decorators(template);
        let lines = collect_lines(&dec.body)?;
        let (mut nodes, consumed) = parse_nodes(&lines, 0, 0)?;
        if consumed < lines.len() {
            return Err(RawParseError {
                message: format!("unexpected control-flow line '{}'", lines[consumed].1),
                byte_offset: None,
            });
        }
        crate::preprocess::expand_class_aliases_in_nodes(&mut nodes, &dec.class_aliases);
        Ok(nodes)
    }
}

/// Whether dispatch selects a markup frontend rather than indentation syntax.
/// Use this before interpreting DSL component-section delimiters, which can be
/// ordinary text in markup templates.
pub fn is_markup_mode(template: &str, path: Option<&std::path::Path>) -> bool {
    is_vue_mode(path)
        || is_svelte_mode(path)
        || is_astro_mode(path)
        || is_angular_mode(path)
        || is_jsx_mode(template.strip_prefix('\u{feff}').unwrap_or(template), path)
}

/// Returns true when the file path has the `.svelte` extension, activating the
/// Svelte template frontend.
pub(crate) fn is_svelte_mode(path: Option<&std::path::Path>) -> bool {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svelte"))
}

/// Returns true when the file path has the `.vue` extension, activating the
/// Vue single-file-component frontend.
pub(crate) fn is_vue_mode(path: Option<&std::path::Path>) -> bool {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("vue"))
        .unwrap_or(false)
}

/// Returns true when the file path has the `.astro` extension, activating the
/// Astro template frontend.
pub(crate) fn is_astro_mode(path: Option<&std::path::Path>) -> bool {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("astro"))
}

/// Returns true when the file path names an Angular component template.
///
/// Angular templates are plain `.html`, which the dispatcher cannot claim
/// wholesale, so the frontend is keyed on the Angular CLI's own naming
/// convention (`foo.component.html`), on the explicit `.ng.html` opt-in, and on
/// a bare `.ng` extension.
pub(crate) fn is_angular_mode(path: Option<&std::path::Path>) -> bool {
    let Some(path) = path else {
        return false;
    };
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ng"))
    {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase())
        .is_some_and(|n| n.ends_with(".component.html") || n.ends_with(".ng.html"))
}

/// Returns true when the first non-blank, non-comment, non-`$:` line starts with `<`
/// or when the file path has a JSX extension (`.csx`, `.jsx`, `.tsx`).
/// This activates the JSX/HTML tag-based input syntax.
pub fn is_jsx_mode(template: &str, path: Option<&std::path::Path>) -> bool {
    if let Some(path) = path {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_lowercase();
            if ext == "csx" || ext == "jsx" || ext == "tsx" {
                return true;
            }
        }
    }

    skip_jsx_preamble(template).starts_with('<')
}

/// Return the template body after leading blank, hash-comment, and `$:` lines.
pub fn skip_jsx_preamble(template: &str) -> &str {
    let mut offset = 0;
    for line in template.split_inclusive('\n') {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("$:") {
            return template[offset..].trim_start();
        }
        offset += line.len();
    }
    &template[template.len()..]
}
#[cfg(test)]
mod tests {
    use super::indent::strip_structural_indent;
    use super::*;

    #[test]
    fn quoted_delimiters_survive_frontend_dispatch() {
        for filename in ["App.jsx", "App.svelte", "App.astro"] {
            let nodes = parse_template_with_path(
                r#"<p title={"}"} />"#,
                Some(std::path::Path::new(filename)),
            )
            .expect("quoted brace in attribute");
            let Node::Element(el) = &nodes[0] else {
                panic!("expected element");
            };
            assert!(el
                .bindings
                .iter()
                .any(|binding| binding.prop == "title" && binding.value == r#""}""#));
        }
        let condition = r#"value == "a\")b""#;
        let source = format!("@if ({condition}) {{ <p /> }}");
        let nodes = parse_template_with_path(&source, Some(std::path::Path::new("App.ng")))
            .expect("escaped quote in Angular block head");
        assert!(matches!(&nodes[0], Node::If(block) if block.condition == condition));
    }

    #[test]
    fn unicode_in_malformed_markup_returns_errors() {
        let tail = format!("</{}>", "界".repeat(20));
        for (source, filename) in [
            (format!("<template>{tail}</template>"), "page.vue"),
            (tail.clone(), "page.astro"),
            (tail.clone(), "page.component.html"),
            (format!("<div>{tail}"), "page.jsx"),
            (format!("{{#if ok}}{tail}"), "page.svelte"),
            (format!("@if (ok) {{{tail}"), "page.component.html"),
            (format!("{{<div />{tail}}}"), "page.astro"),
        ] {
            assert!(
                parse_template_with_path(&source, Some(std::path::Path::new(filename))).is_err(),
                "expected parse error for {filename}: {source}"
            );
        }
        let source = "界".repeat(10);
        assert!(jsx::attrs::jsx_brace_expr(&source, &source).is_err());
    }

    #[test]
    fn strip_structural_indent_basic() {
        assert_eq!(strip_structural_indent("    hello", 4), "hello");
        assert_eq!(strip_structural_indent("    hello", 2), "  hello");
        assert_eq!(strip_structural_indent("    hello", 0), "    hello");
        assert_eq!(strip_structural_indent("hi", 4), "hi");
        assert_eq!(strip_structural_indent("", 4), "");
    }

    fn text_from_node(node: &Node) -> String {
        let Node::Text(parts) = node else {
            panic!("expected Text node, got: {node:?}")
        };
        parts
            .iter()
            .map(|p| match p {
                crate::ast::TextPart::Literal(s) => s.clone(),
                crate::ast::TextPart::Expr(e) => format!("{{{e}}}"),
            })
            .collect()
    }

    #[test]
    fn parse_when_attribute_suffix_braced_condition_with_equals() {
        let (c, v) = parse_when_attribute_suffix(r#"{a == b}="x y z""#).unwrap();
        assert_eq!(c, "a == b");
        assert_eq!(v, r#""x y z""#);
    }

    #[test]
    fn parse_when_attribute_suffix_simple_ident() {
        let (c, v) = parse_when_attribute_suffix(r#"active=bg-red-500"#).unwrap();
        assert_eq!(c, "active");
        assert_eq!(v, "bg-red-500");
    }

    #[test]
    fn when_attribute_indent_expands_to_conditional_classes() {
        let nodes = parse_template(
            r#"div base when:{flag}="a b" when:{!flag}="c"
  "hi""#,
        )
        .unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element");
        };
        assert_eq!(el.classes, vec!["base".to_string()]);
        assert_eq!(el.conditional_classes.len(), 3);
        assert_eq!(el.conditional_classes[0].class, "a");
        assert_eq!(el.conditional_classes[0].condition, "flag");
        assert_eq!(el.conditional_classes[1].class, "b");
        assert_eq!(el.conditional_classes[2].class, "c");
        assert_eq!(el.conditional_classes[2].condition, "!flag");
    }

    #[test]
    fn when_attribute_jsx_expands_to_conditional_classes() {
        let nodes =
            parse_template(r#"<div class="base" when:{active}="font-bold text-white"></div>"#)
                .unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element");
        };
        assert_eq!(el.classes, vec!["base".to_string()]);
        assert_eq!(el.conditional_classes.len(), 2);
        assert_eq!(el.conditional_classes[0].class, "font-bold");
        assert_eq!(el.conditional_classes[0].condition, "active");
        assert_eq!(el.conditional_classes[1].class, "text-white");
    }

    #[test]
    fn jsx_class_and_class_name_are_class_tokens() {
        let nodes =
            parse_template(r#"<div class="flex gap-4" className="text-white bg-zinc-950"></div>"#)
                .unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element");
        };
        assert_eq!(
            el.classes,
            vec![
                "flex".to_string(),
                "gap-4".to_string(),
                "text-white".to_string(),
                "bg-zinc-950".to_string()
            ]
        );
    }

    #[test]
    fn embed_indent_parses_src_adapter_and_props() {
        let nodes =
            parse_template(r#"embed ./islands/wave.ts adapter="module" title="Wave" count={n}"#)
                .unwrap();
        let Node::Embed(embed) = &nodes[0] else {
            panic!("expected embed");
        };
        assert_eq!(embed.src, "./islands/wave.ts");
        assert_eq!(embed.adapter.as_deref(), Some("module"));
        assert_eq!(embed.props.len(), 2);
        assert_eq!(
            embed.props[0],
            ("title".to_string(), "\"Wave\"".to_string())
        );
        assert_eq!(embed.props[1], ("count".to_string(), "n".to_string()));
    }

    #[test]
    fn island_jsx_parses_src_adapter_and_props() {
        let nodes = parse_template(
            r#"<island src="./islands/wave.ts" adapter="module" title="Wave" count={n} />"#,
        )
        .unwrap();
        let Node::Embed(embed) = &nodes[0] else {
            panic!("expected embed");
        };
        assert_eq!(embed.src, "./islands/wave.ts");
        assert_eq!(embed.adapter.as_deref(), Some("module"));
        assert_eq!(embed.props.len(), 2);
        assert_eq!(
            embed.props[0],
            ("title".to_string(), "\"Wave\"".to_string())
        );
        assert_eq!(embed.props[1], ("count".to_string(), "n".to_string()));
    }

    #[test]
    fn multiline_string_preserves_inner_indent() {
        // `"` line is at indent=4 (4 leading spaces). Continuation "  indented line"
        // has 2 spaces → after stripping 4 structural spaces it becomes "indented line"
        // (only 4 available, so strip up to the string length → "indented line").
        // Actually strip min(4, available) → strips 2 available → "indented line".
        let template = "pre\n  \"line one\n  indented line\n    more indented\"";
        let nodes = parse_template(template).unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element")
        };
        let text = text_from_node(&el.children[0]);
        assert!(text.contains("indented line"), "got: {text:?}");
        // "  indented line" with structural indent 2 → strips 2 → "indented line" (0 extra spaces)
        let cont_indent = text
            .lines()
            .nth(1)
            .map(|l| l.len() - l.trim_start().len())
            .unwrap_or(0);
        assert_eq!(
            cont_indent, 0,
            "continuation at same level should have 0 leading spaces, got: {text:?}"
        );
        // "    more indented" with structural indent 2 → strips 2 → "  more indented" (2 extra)
        let deep_indent = text
            .lines()
            .nth(2)
            .map(|l| l.len() - l.trim_start().len())
            .unwrap_or(0);
        assert_eq!(
            deep_indent, 2,
            "deeper line should have 2 preserved leading spaces, got: {text:?}"
        );
    }

    #[test]
    fn multiline_string_preserves_extra_indent() {
        // structural indent of the `"` line is 2; continuation "    extra" has 4 spaces.
        // After stripping 2 structural spaces: "  extra" (2 extra spaces remain).
        let template = "pre\n  \"first\n    extra\"";
        let nodes = parse_template(template).unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element")
        };
        let text = text_from_node(&el.children[0]);
        assert!(
            text.contains("  extra"),
            "extra indent should be preserved, got: {text:?}"
        );
    }

    #[test]
    fn quoted_line_unescapes_backslash_sequences() {
        let nodes = parse_template("pre\n  \"a\\nb\\tc\\\\d\"").unwrap();
        let Node::Element(el) = &nodes[0] else {
            panic!("expected element");
        };
        let text = text_from_node(&el.children[0]);
        assert_eq!(text, "a\nb\tc\\d");
    }

    #[test]
    fn unescape_crepus_text_literal_accepts_common_escapes() {
        assert_eq!(
            unescape_crepus_text_literal(r#"line\n\t\"quote""#),
            "line\n\t\"quote\""
        );
    }

    #[test]
    fn parse_component_file_frontmatter_with_dotted_defaults() {
        let source = r#"+++
[Panel.defaults]
warning = "sandboxed"
show_source = true
+++

--- Panel
div
  "{warning}""#;
        let file = parse_component_file(source).expect("frontmatter should parse");
        let panel = file.components.get("Panel").expect("Panel component");
        assert_eq!(
            panel.meta.defaults.get("warning").map(|e| e.to_string()),
            Some("\"sandboxed\"".to_string())
        );
    }

    #[test]
    fn parse_component_file_no_frontmatter() {
        let source = r#"--- Comp1
div
  "Hello"
--- Comp2
span
  "World""#;
        let file = parse_component_file(source).expect("should parse");
        assert_eq!(file.components.len(), 2);
        assert!(file.components.contains_key("Comp1"));
        assert!(file.components.contains_key("Comp2"));
        let comp1 = &file.components["Comp1"];
        assert_eq!(comp1.nodes.len(), 1);
        assert!(comp1.meta.description.is_none());
        assert!(comp1.meta.defaults.is_empty());
    }

    #[test]
    fn parse_component_file_with_description() {
        let source = r#"+++
[Button]
description = "A simple button component"
+++

--- Button
button
  "Click me""#;
        let file = parse_component_file(source).expect("should parse");
        let btn = file.components.get("Button").expect("Button component");
        assert_eq!(
            btn.meta.description.as_deref(),
            Some("A simple button component")
        );
    }

    #[test]
    fn parse_component_file_invalid_toml() {
        let source = r#"+++
[Button]
invalid toml
+++

--- Button
button"#;
        let err = parse_component_file(source).expect_err("should fail");
        assert!(err.to_string().contains("TOML parse error in frontmatter"));
    }

    #[test]
    fn parse_component_file_invalid_component_syntax() {
        let source = r#"--- Button
<unclosed>"#;
        let err = parse_component_file(source).expect_err("should fail");
        assert!(err.to_string().contains("component \"Button\":"));
    }

    #[test]
    fn split_component_body_sections_basic() {
        let sections = split_component_body_sections("--- Comp1\nline1\nline2");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "line1\nline2");
        assert_eq!(sections[0].2, 10);
    }

    #[test]
    fn split_component_body_sections_multiple() {
        let sections = split_component_body_sections("--- Comp1\nline1\n--- Comp2\nline2");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "line1");
        assert_eq!(sections[0].2, 10);
        assert_eq!(sections[1].0, "Comp2");
        assert_eq!(sections[1].1, "line2");
        assert_eq!(sections[1].2, 26);
    }

    #[test]
    fn split_component_body_sections_empty_body() {
        let sections = split_component_body_sections("--- Comp1\n");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "");
        assert_eq!(sections[0].2, 10);
    }

    #[test]
    fn split_component_body_sections_ignored_text() {
        let sections = split_component_body_sections("ignored text\n--- Comp1\nline1");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "line1");
        assert_eq!(sections[0].2, 23);
    }

    #[test]
    fn split_component_body_sections_trimmed_name() {
        let sections = split_component_body_sections("---   Comp1  \nline1");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "line1");
        assert_eq!(sections[0].2, 14);
    }

    #[test]
    fn split_component_body_sections_crlf() {
        let sections = split_component_body_sections("--- Comp1\r\nline1\r\n--- Comp2\r\nline2");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "line1");
        assert_eq!(sections[0].2, 11);
        assert_eq!(sections[1].0, "Comp2");
        assert_eq!(sections[1].1, "line2");
        assert_eq!(sections[1].2, 29);
    }

    #[test]
    fn split_component_body_sections_no_newline_at_end() {
        let sections = split_component_body_sections("--- Comp1");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Comp1");
        assert_eq!(sections[0].1, "");
        assert_eq!(sections[0].2, 9);
    }

    #[test]
    fn split_frontmatter_parts_basic() {
        let content = "+++\nfoo = 'bar'\n+++\nbody content";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, Some("\nfoo = 'bar'"));
        assert_eq!(rest, "\nbody content");
        assert_eq!(offset, 19);
    }

    #[test]
    fn split_frontmatter_parts_with_leading_whitespace() {
        let content = "  \n\n+++\nfoo = 'bar'\n+++\nbody content";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, Some("\nfoo = 'bar'"));
        assert_eq!(rest, "\nbody content");
        assert_eq!(offset, 23);
    }

    #[test]
    fn split_frontmatter_parts_no_frontmatter() {
        let content = "just some text";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, None);
        assert_eq!(rest, "just some text");
        assert_eq!(offset, 0);
    }

    #[test]
    fn split_frontmatter_parts_missing_closing_tag() {
        let content = "+++\nfoo = 'bar'\nbody content";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, None);
        assert_eq!(rest, "+++\nfoo = 'bar'\nbody content");
        assert_eq!(offset, 0);
    }

    #[test]
    fn split_frontmatter_parts_empty() {
        let content = "+++\n+++\nbody";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, Some(""));
        assert_eq!(rest, "\nbody");
        assert_eq!(offset, 7);
    }

    #[test]
    fn split_frontmatter_parts_only_open() {
        let content = "+++";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, None);
        assert_eq!(rest, "+++");
        assert_eq!(offset, 0);
    }

    #[test]
    fn split_frontmatter_parts_crlf() {
        let content = "+++\r\nfoo = 'bar'\r\n+++\r\nbody content";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, Some("\r\nfoo = 'bar'\r"));
        assert_eq!(rest, "\r\nbody content");
        assert_eq!(offset, 21);
    }

    #[test]
    fn split_frontmatter_parts_multiple_blocks() {
        let content = "+++\nfoo = 'bar'\n+++\n+++\nbaz = 'qux'\n+++";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, Some("\nfoo = 'bar'"));
        assert_eq!(rest, "\n+++\nbaz = 'qux'\n+++");
        assert_eq!(offset, 19);
    }

    #[test]
    fn split_frontmatter_parts_no_newline_before_close() {
        let content = "+++foo+++";
        let (toml, rest, offset) = split_frontmatter_parts(content);
        assert_eq!(toml, None);
        assert_eq!(rest, "+++foo+++");
        assert_eq!(offset, 0);
    }
}
