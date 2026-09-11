use crepuscularity_core::tailwind::{
    parse_arbitrary_length_token, parse_length_token, LengthToken,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use syn::{parse_macro_input, LitStr};

/// Global counter for generating unique element IDs within a compilation unit.
static ELEM_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

// ============================================================
// Entry point
// ============================================================

/// Declarative UI macro with Tailwind-like syntax for GPUI.
///
/// Write indentation-based templates that compile to GPUI builder chains.
/// Requires `cx: &mut Context<Self>` in scope for event handlers.
///
/// # Syntax
///
/// ```ignore
/// view! {r#"
/// div w-full h-full bg-black flex items-center
///     span text-2xl font-bold text-white
///         "{greeting}"
///     button bg-white text-black rounded-lg px-4 py-2 @click=say_hello
///         "Click me"
/// "#}
/// ```
///
/// ## Reactive declarations
/// `$: let name = {expr}` — computed binding available in the template
///
/// ## Control flow
/// ```ignore
/// if {condition}
///     ...
/// else if {condition2}
///     ...
/// else
///     ...
///
/// match {expr}
///     {Pattern::A} =>
///         ...
///     {Pattern::B(x)} =>
///         ...
///     _ =>
///         ...
///
/// for item in {iterator}
///     ...
/// ```
///
/// ## Conditional classes
/// `class:hidden={condition}` — applies one utility class when condition is true
///
/// `when:{condition}="class1 class2 …"` — applies several classes under the same condition (fewer repetition than many `class:` lines)
///
/// ## Event modifiers
/// `@keydown|Enter=handler` — only fires when Enter is pressed
/// `@keydown|ctrl+s=handler` — ctrl+s combination
///
/// ## Events
/// `@click`, `@input`, `@change`, `@keydown`, `@keyup`, `@hover`, `@mousedown`, `@mouseup`
///
/// ## Bindings
/// `bind:value={field}` — two-way value binding
///
/// ## State prefixes
/// `hover:`, `focus:`, `active:`
///
/// ## Comments
/// Lines starting with `#` are ignored.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    generate_view(&lit.value(), lit.span()).into()
}

/// Compile a `.crepus` template file into a GPUI view at build time.
///
/// Usage:
///
/// ```ignore
/// view_file!("ui/sidebar.crepus")
/// ```
#[proc_macro]
pub fn view_file(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let path_value = path.value();
    let (file_str, component_name) = match path_value.split_once('#') {
        Some((file, comp)) => (file, Some(comp.to_string())),
        None => (path_value.as_str(), None),
    };
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let full = std::path::Path::new(&manifest_dir).join(file_str);
    let template = std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("Failed to read template file `{}`: {}", full.display(), e));
    let template = match component_name {
        Some(name) => extract_component_section(&template, &name),
        None => template,
    };
    let base_dir = full.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let expanded = expand_includes_text(&template, &base_dir, &full);
    generate_view(&expanded, path.span()).into()
}

fn generate_view(template: &str, span: Span) -> TokenStream2 {
    let core_nodes = match crepuscularity_core::parser::parse_template(template) {
        Ok(nodes) => nodes,
        Err(e) => return syn::Error::new(span, e.to_string()).to_compile_error(),
    };
    let nodes = match core_nodes
        .iter()
        .map(core_node_to_macro)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(nodes) => nodes,
        Err(message) => return syn::Error::new(span, message).to_compile_error(),
    };

    match generate_root(&nodes) {
        Ok(tokens) => tokens,
        Err(message) => syn::Error::new(span, message).to_compile_error(),
    }
}

fn expand_includes_text(
    template: &str,
    base_dir: &std::path::Path,
    source_path: &std::path::Path,
) -> String {
    let mut cache = std::collections::HashMap::new();
    expand_includes_inner(template, base_dir, source_path, &mut cache)
}

fn expand_includes_inner(
    template: &str,
    base_dir: &std::path::Path,
    _source_path: &std::path::Path,
    cache: &mut std::collections::HashMap<std::path::PathBuf, std::rc::Rc<String>>,
) -> String {
    let mut output = String::with_capacity(template.len() + 1024);
    for line in template.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("include ") {
            let (path_str, _props) = rest.split_once(' ').unwrap_or((rest, ""));
            let path_str = path_str.trim().trim_matches('"');
            if path_str.is_empty() {
                output.push_str(line);
                output.push('\n');
                continue;
            }
            let (file_str, component_name) = match path_str.split_once('#') {
                Some((file, comp)) => (file, Some(comp)),
                None => (path_str, None),
            };
            let include_path = base_dir.join(file_str);
            let include_dir = include_path.parent().unwrap_or(base_dir).to_path_buf();

            if !cache.contains_key(&include_path) {
                let text = std::fs::read_to_string(&include_path).unwrap_or_else(|e| {
                    panic!(
                        "Failed to read included template file `{}`: {}",
                        include_path.display(),
                        e
                    )
                });
                cache.insert(include_path.clone(), std::rc::Rc::new(text));
            }
            let included = cache.get(&include_path).unwrap().clone();

            let section = match component_name {
                Some(name) => extract_component_section(&included, name),
                None => included.to_string(),
            };
            let expanded = expand_includes_inner(&section, &include_dir, &include_path, cache);
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            for included_line in expanded.lines() {
                if included_line.is_empty() {
                    output.push('\n');
                } else {
                    output.push_str(indent);
                    output.push_str(included_line);
                    output.push('\n');
                }
            }
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn extract_component_section(content: &str, name: &str) -> String {
    let (_, body, _) = crepuscularity_core::parser::split_frontmatter_parts(content);
    let sections = crepuscularity_core::parser::split_component_body_sections(body);
    sections
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, text, _)| text)
        .unwrap_or_else(|| panic!("Component `{name}` not found in multi-component file"))
}

/// Generate typed DOM accessors for all `#id` shorthand IDs reachable from a `.crepus` entry file.
///
/// ```ignore
/// let crepus = crepuscularity_web::crepus_refs!("../index.crepus");
/// crepus.hero.text("Updated")?;
/// ```
#[proc_macro]
pub fn crepus_refs(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let rel_path = lit.value();
    let span = lit.span();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let entry_path = PathBuf::from(manifest_dir).join(&rel_path);

    match collect_ids_from_path(&entry_path, "crepus_refs!") {
        Ok(ids) => {
            let fields: Vec<TokenStream2> = ids
                .iter()
                .filter_map(|id| {
                    make_rust_ident(id).map(|ident| {
                        quote! { pub #ident: ::crepuscularity_web::dom::StaticElementRef }
                    })
                })
                .collect();
            let inits: Vec<TokenStream2> = ids
                .iter()
                .filter_map(|id| {
                    make_rust_ident(id).map(|ident| {
                        quote! { #ident: ::crepuscularity_web::dom::StaticElementRef::new(#id) }
                    })
                })
                .collect();

            quote! {{
                #[allow(non_camel_case_types)]
                struct __CrepusRefs {
                    #(#fields),*
                }

                __CrepusRefs {
                    #(#inits),*
                }
            }}
            .into()
        }
        Err(message) => syn::Error::new(span, message).to_compile_error().into(),
    }
}

/// Generate a TUI template wrapper with jQuery-like handles for all `#id`s.
///
/// ```ignore
/// let mut ui = crepuscularity_tui::template_refs!("ui/ui.crepus")?;
/// ui.input.content = "input contents".to_string();
/// ui.title.text("My App");
/// ui.draw_full(frame)?;
/// ```
#[proc_macro]
pub fn template_refs(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let rel_path = lit.value();
    let span = lit.span();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let entry_path = PathBuf::from(manifest_dir).join(&rel_path);

    match build_tui_template_refs(&entry_path) {
        Ok(tokens) => tokens.into(),
        Err(message) => syn::Error::new(span, message).to_compile_error().into(),
    }
}

fn build_tui_template_refs(entry_path: &Path) -> Result<TokenStream2, String> {
    let ids = collect_ids_from_path(entry_path, "template_refs!")?;
    let canonical = std::fs::canonicalize(entry_path).unwrap_or_else(|_| entry_path.to_path_buf());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("template_refs! could not read {}: {e}", canonical.display()))?;
    let path = canonical.to_string_lossy().to_string();

    let fields: Vec<TokenStream2> = ids
        .iter()
        .filter_map(|id| {
            make_rust_ident(id).map(|ident| {
                quote! { pub #ident: ::crepuscularity_tui::ElementRef }
            })
        })
        .collect();
    let inits: Vec<TokenStream2> = ids
        .iter()
        .filter_map(|id| {
            make_rust_ident(id).map(|ident| {
                quote! { #ident: ::crepuscularity_tui::ElementRef::new(#id) }
            })
        })
        .collect();
    let syncs: Vec<TokenStream2> = ids
        .iter()
        .filter_map(|id| {
            make_rust_ident(id).map(|ident| {
                let key = ident.to_string();
                quote! {
                    self.template.set(#key, self.#ident.content.clone());
                }
            })
        })
        .collect();
    let matches: Vec<TokenStream2> = ids
        .iter()
        .filter_map(|id| {
            make_rust_ident(id).map(|ident| {
                quote! { #id => Some(&mut self.#ident) }
            })
        })
        .collect();

    Ok(quote! {{
        #[allow(non_camel_case_types)]
        struct __CrepusTuiTemplateRefs {
            template: ::crepuscularity_tui::Template,
            #(#fields),*
        }

        impl __CrepusTuiTemplateRefs {
            pub fn set(
                &mut self,
                key: impl Into<String>,
                value: impl Into<::crepuscularity_tui::TemplateValue>,
            ) -> &mut Self {
                self.template.set(key, value);
                self
            }

            pub fn get(&mut self, id: &str) -> Option<&mut ::crepuscularity_tui::ElementRef> {
                match id {
                    #(#matches,)*
                    _ => None,
                }
            }

            pub fn find(&mut self, selector: &str) -> Option<&mut ::crepuscularity_tui::ElementRef> {
                self.get(selector.strip_prefix('#').unwrap_or(selector))
            }

            pub fn template(&self) -> &::crepuscularity_tui::Template {
                &self.template
            }

            pub fn template_mut(&mut self) -> &mut ::crepuscularity_tui::Template {
                &mut self.template
            }

            pub fn sync_refs(&mut self) -> &mut Self {
                #(#syncs)*
                self
            }

            pub fn draw(
                &mut self,
                frame: &mut ::crepuscularity_tui::ratatui::Frame,
                area: ::crepuscularity_tui::ratatui::layout::Rect,
            ) -> Result<(), String> {
                self.sync_refs();
                self.template.draw(frame, area)
            }

            pub fn draw_full(
                &mut self,
                frame: &mut ::crepuscularity_tui::ratatui::Frame,
            ) -> Result<(), String> {
                self.sync_refs();
                self.template.draw_full(frame)
            }
        }

        Ok::<__CrepusTuiTemplateRefs, String>(__CrepusTuiTemplateRefs {
            template: ::crepuscularity_tui::Template::from_source_with_path(#source, #path),
            #(#inits),*
        })
    }})
}

fn collect_ids_from_path(entry_path: &Path, macro_name: &str) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let mut ids = BTreeSet::new();
    collect_ids_recursive(entry_path, macro_name, &mut seen, &mut ids)?;
    Ok(ids.into_iter().collect())
}

fn collect_ids_recursive(
    path: &Path,
    macro_name: &str,
    seen: &mut BTreeSet<PathBuf>,
    ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical.clone()) {
        return Ok(());
    }

    let content = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("{macro_name} could not read {}: {e}", canonical.display()))?;
    if let Ok(file) = crepuscularity_core::parse_component_file(&content) {
        if !file.components.is_empty() {
            for component in file.components.values() {
                collect_ids_from_nodes(
                    &component.nodes,
                    canonical.parent(),
                    macro_name,
                    seen,
                    ids,
                )?;
            }
            return Ok(());
        }
    }
    let nodes = crepuscularity_core::parse_template(&content)
        .map_err(|e| format!("{macro_name} parse error in {}: {e}", canonical.display()))?;
    collect_ids_from_nodes(&nodes, canonical.parent(), macro_name, seen, ids)
}

fn collect_ids_from_nodes(
    nodes: &[crepuscularity_core::ast::Node],
    base_dir: Option<&Path>,
    macro_name: &str,
    seen: &mut BTreeSet<PathBuf>,
    ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    use crepuscularity_core::ast::Node;

    for node in nodes {
        match node {
            Node::Element(el) => {
                if let Some(id) = &el.id {
                    ids.insert(id.clone());
                }
                collect_ids_from_nodes(&el.children, base_dir, macro_name, seen, ids)?;
            }
            Node::If(block) => {
                collect_ids_from_nodes(&block.then_children, base_dir, macro_name, seen, ids)?;
                if let Some(else_children) = &block.else_children {
                    collect_ids_from_nodes(else_children, base_dir, macro_name, seen, ids)?;
                }
            }
            Node::For(block) => {
                collect_ids_from_nodes(&block.body, base_dir, macro_name, seen, ids)?
            }
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_ids_from_nodes(&arm.body, base_dir, macro_name, seen, ids)?;
                }
            }
            Node::Include(inc) => {
                let include_path = inc
                    .path
                    .split_once('#')
                    .map(|(p, _)| p)
                    .unwrap_or(&inc.path);
                let resolved = if let Some(base) = base_dir {
                    base.join(include_path)
                } else {
                    PathBuf::from(include_path)
                };
                collect_ids_recursive(&resolved, macro_name, seen, ids)?;
                collect_ids_from_nodes(&inc.slot, base_dir, macro_name, seen, ids)?;
            }
            Node::Text(_)
            | Node::RawText(_)
            | Node::RawHtml(_)
            | Node::LetDecl(_)
            | Node::Embed(_) => {}
        }
    }

    Ok(())
}

fn make_rust_ident(id: &str) -> Option<syn::Ident> {
    let id = id.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
    let id = if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    };
    if id.is_empty() {
        None
    } else if syn::parse_str::<syn::Ident>(&id).is_ok() {
        Some(syn::Ident::new(&id, Span::call_site()))
    } else {
        Some(syn::Ident::new(&format!("_{id}"), Span::call_site()))
    }
}

// ============================================================
// AST
// ============================================================

#[derive(Debug, Clone)]
enum Node {
    Element(Element),
    Text(Vec<TextPart>),
    RawExpr(String),
    If(IfBlock),
    For(ForBlock),
    Match(MatchBlock),
    LetDecl(LetDecl),
}

#[derive(Debug, Clone)]
struct Element {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    conditional_classes: Vec<ConditionalClass>,
    event_handlers: Vec<EventHandler>,
    bindings: Vec<Binding>,
    children: Vec<Node>,
}

#[derive(Debug, Clone)]
struct ConditionalClass {
    class: String,
    condition: String,
}

#[derive(Debug, Clone)]
struct EventHandler {
    event: String,
    modifiers: Vec<String>,
    handler: String,
}

#[derive(Debug, Clone)]
struct Binding {
    prop: String,
    value: String,
}

#[derive(Debug, Clone)]
enum TextPart {
    Literal(String),
    Expr(String),
}

#[derive(Debug, Clone)]
struct IfBlock {
    condition: String,
    then_children: Vec<Node>,
    else_children: Option<Vec<Node>>,
}

#[derive(Debug, Clone)]
struct ForBlock {
    pattern: String,
    iterator: String,
    body: Vec<Node>,
}

#[derive(Debug, Clone)]
struct MatchBlock {
    expr: String,
    arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
struct MatchArm {
    pattern: String,
    body: Vec<Node>,
}

#[derive(Debug, Clone)]
struct LetDecl {
    name: String,
    expr: String,
}

// ============================================================
// Core AST → macro AST conversion
// ============================================================

fn core_node_to_macro(node: &crepuscularity_core::ast::Node) -> Result<Node, String> {
    use crepuscularity_core::ast;
    match node {
        ast::Node::Element(el) => Ok(Node::Element(Element {
            tag: el.tag.clone(),
            id: el.id.clone(),
            classes: el.classes.clone(),
            conditional_classes: el
                .conditional_classes
                .iter()
                .map(|cc| ConditionalClass {
                    class: cc.class.clone(),
                    condition: cc.condition.clone(),
                })
                .collect(),
            event_handlers: el
                .event_handlers
                .iter()
                .map(|eh| EventHandler {
                    event: eh.event.clone(),
                    modifiers: eh.modifiers.clone(),
                    handler: eh.handler.clone(),
                })
                .collect(),
            bindings: el
                .bindings
                .iter()
                .map(|b| Binding {
                    prop: b.prop.clone(),
                    value: b.value.clone(),
                })
                .collect(),
            children: el
                .children
                .iter()
                .map(core_node_to_macro)
                .collect::<Result<Vec<_>, _>>()?,
        })),
        ast::Node::Text(parts) => Ok(Node::Text(
            parts
                .iter()
                .map(|p| match p {
                    ast::TextPart::Literal(s) => TextPart::Literal(s.clone()),
                    ast::TextPart::Expr(s) => TextPart::Expr(s.clone()),
                })
                .collect(),
        )),
        ast::Node::RawText(expr) => Ok(Node::RawExpr(expr.clone())),
        ast::Node::If(block) => Ok(Node::If(IfBlock {
            condition: block.condition.clone(),
            then_children: block
                .then_children
                .iter()
                .map(core_node_to_macro)
                .collect::<Result<Vec<_>, _>>()?,
            else_children: block
                .else_children
                .as_ref()
                .map(|cs| {
                    cs.iter()
                        .map(core_node_to_macro)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        })),
        ast::Node::For(block) => Ok(Node::For(ForBlock {
            pattern: block.pattern.clone(),
            iterator: block.iterator.clone(),
            body: block
                .body
                .iter()
                .map(core_node_to_macro)
                .collect::<Result<Vec<_>, _>>()?,
        })),
        ast::Node::Match(block) => Ok(Node::Match(MatchBlock {
            expr: block.expr.clone(),
            arms: block
                .arms
                .iter()
                .map(|arm| -> Result<MatchArm, String> {
                    Ok(MatchArm {
                        pattern: arm.pattern.clone(),
                        body: arm
                            .body
                            .iter()
                            .map(core_node_to_macro)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        })),
        ast::Node::LetDecl(decl) => Ok(Node::LetDecl(LetDecl {
            name: decl.name.clone(),
            expr: decl.expr.clone(),
        })),
        ast::Node::RawHtml(_) | ast::Node::Include(_) | ast::Node::Embed(_) => {
            Err("node type not supported in view! macro".to_string())
        }
    }
}

fn extract_braced_expr_str(s: &str) -> Option<String> {
    if !s.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[1..i].trim().to_string());
                }
            }
            _ => {}
        }
    }
    None
}

// ============================================================
// Code generator
// ============================================================

fn generate_root(nodes: &[Node]) -> Result<TokenStream2, String> {
    // Separate top-level let decls from other nodes
    let mut let_stmts: Vec<TokenStream2> = Vec::new();
    let mut element_nodes: Vec<&Node> = Vec::new();

    for node in nodes {
        match node {
            Node::LetDecl(decl) => {
                let_stmts.push(generate_let_decl(decl)?);
            }
            other => element_nodes.push(other),
        }
    }

    let main_expr = match element_nodes.len() {
        0 => quote! { ::gpui::div() },
        1 => generate_node(element_nodes[0])?,
        _ => {
            let child_calls = generate_child_calls_refs(&element_nodes)?;
            quote! { ::gpui::div() #(#child_calls)* }
        }
    };

    if let_stmts.is_empty() {
        Ok(main_expr)
    } else {
        Ok(quote! {
            {
                #(#let_stmts)*
                #main_expr
            }
        })
    }
}

fn generate_let_decl(decl: &LetDecl) -> Result<TokenStream2, String> {
    let name: syn::Ident =
        syn::parse_str(&decl.name).map_err(|e| format!("Invalid let name `{}`: {e}", decl.name))?;
    let expr: syn::Expr =
        syn::parse_str(&decl.expr).map_err(|e| format!("Invalid let expr `{}`: {e}", decl.expr))?;
    Ok(quote! { let #name = #expr; })
}

fn generate_node(node: &Node) -> Result<TokenStream2, String> {
    match node {
        Node::Element(element) => generate_element(element),
        Node::Text(parts) => generate_text(parts),
        Node::RawExpr(expr_string) => {
            let expr: syn::Expr = syn::parse_str(expr_string)
                .map_err(|error| format!("Invalid expression `{expr_string}`: {error}"))?;
            Ok(quote! { #expr })
        }
        Node::If(if_block) => generate_if_expr(if_block),
        Node::For(for_block) => generate_for_expr(for_block),
        Node::Match(match_block) => generate_match_expr(match_block),
        Node::LetDecl(decl) => generate_let_decl(decl),
    }
}

fn generate_child_calls(nodes: &[Node]) -> Result<Vec<TokenStream2>, String> {
    nodes.iter().map(generate_child_call).collect()
}

fn generate_child_calls_refs(nodes: &[&Node]) -> Result<Vec<TokenStream2>, String> {
    nodes.iter().map(|n| generate_child_call(n)).collect()
}

fn generate_child_call(node: &Node) -> Result<TokenStream2, String> {
    match node {
        Node::If(if_block) => generate_if_child_call(if_block),
        Node::For(for_block) => generate_for_child_call(for_block),
        Node::Match(match_block) => generate_match_child_call(match_block),
        Node::LetDecl(decl) => {
            // Let decls inside elements: hoist into a .child block? Not supported elegantly.
            // Emit as a statement that won't actually add a child.
            let stmt = generate_let_decl(decl)?;
            Ok(quote! { /* $: let inside element is not supported */ ; #stmt })
        }
        other => {
            let expr = generate_node(other)?;
            Ok(quote! { .child(#expr) })
        }
    }
}

fn generate_element(element: &Element) -> Result<TokenStream2, String> {
    let tag_expr = element_tag_to_tokens(&element.tag);

    let class_methods: Vec<TokenStream2> = element
        .classes
        .iter()
        .filter_map(|class| map_class(class))
        .collect();

    let conditional_class_calls: Vec<TokenStream2> = element
        .conditional_classes
        .iter()
        .map(generate_conditional_class)
        .collect::<Result<_, _>>()?;

    let handler_calls: Vec<TokenStream2> = element
        .event_handlers
        .iter()
        .map(generate_event_handler)
        .collect::<Result<_, _>>()?;

    let binding_calls: Vec<TokenStream2> = element
        .bindings
        .iter()
        .map(generate_binding)
        .collect::<Result<_, _>>()?;

    let child_calls: Vec<TokenStream2> = element
        .children
        .iter()
        .map(generate_child_call)
        .collect::<Result<_, _>>()?;

    // on_click (and other StatefulInteractiveElement methods) require an ElementId.
    let id_call = if let Some(id) = &element.id {
        quote! { .id(#id) }
    } else if element.event_handlers.iter().any(|h| h.event == "click") {
        let generated_id = ELEM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) as usize;
        quote! { .id(#generated_id as usize) }
    } else {
        quote! {}
    };

    Ok(quote! {
        #tag_expr
        #id_call
        #(#class_methods)*
        #(#conditional_class_calls)*
        #(#handler_calls)*
        #(#binding_calls)*
        #(#child_calls)*
    })
}

fn generate_conditional_class(cc: &ConditionalClass) -> Result<TokenStream2, String> {
    let condition: syn::Expr = syn::parse_str(&cc.condition)
        .map_err(|e| format!("Invalid condition in class:{}: {e}", cc.class))?;
    let style = map_class_to_style(&cc.class)
        .ok_or_else(|| format!("Unknown class `{}` in conditional class", cc.class))?;
    Ok(quote! {
        .when(#condition, |__el| __el #style)
    })
}

fn generate_text(parts: &[TextPart]) -> Result<TokenStream2, String> {
    if parts.is_empty() {
        return Ok(quote! { "" });
    }

    if parts.len() == 1 {
        return match &parts[0] {
            TextPart::Literal(text) => Ok(quote! { #text }),
            TextPart::Expr(expr_string) => {
                let expr: syn::Expr = syn::parse_str(expr_string)
                    .map_err(|error| format!("Invalid expression `{expr_string}`: {error}"))?;
                Ok(quote! { format!("{}", #expr) })
            }
        };
    }

    let mut format_string = String::new();
    let mut expr_tokens: Vec<TokenStream2> = Vec::new();

    for part in parts {
        match part {
            TextPart::Literal(text) => {
                format_string.push_str(&text.replace('{', "{{").replace('}', "}}"));
            }
            TextPart::Expr(expr_string) => {
                format_string.push_str("{}");
                let expr: syn::Expr = syn::parse_str(expr_string)
                    .map_err(|error| format!("Invalid expression `{expr_string}`: {error}"))?;
                expr_tokens.push(quote! { #expr });
            }
        }
    }

    Ok(quote! { format!(#format_string #(, #expr_tokens)*) })
}

fn generate_if_child_call(if_block: &IfBlock) -> Result<TokenStream2, String> {
    let condition: syn::Expr = syn::parse_str(&if_block.condition)
        .map_err(|error| format!("Invalid if condition `{}`: {error}", if_block.condition))?;

    let then_calls = generate_child_calls(&if_block.then_children)?;

    match &if_block.else_children {
        Some(else_children) if if_block.then_children.len() == 1 && else_children.len() == 1 => {
            let then_expr = generate_node(&if_block.then_children[0])?;
            let else_expr = generate_node(&else_children[0])?;
            Ok(quote! {
                .child(if #condition {
                    ::gpui::IntoElement::into_any_element(#then_expr)
                } else {
                    ::gpui::IntoElement::into_any_element(#else_expr)
                })
            })
        }
        Some(else_children) => {
            let else_calls = generate_child_calls(else_children)?;
            Ok(quote! {
                .when(#condition, |__el| __el #(#then_calls)*)
                .when(!(#condition), |__el| __el #(#else_calls)*)
            })
        }
        None => Ok(quote! {
            .when(#condition, |__el| __el #(#then_calls)*)
        }),
    }
}

fn generate_if_expr(if_block: &IfBlock) -> Result<TokenStream2, String> {
    let condition: syn::Expr = syn::parse_str(&if_block.condition)
        .map_err(|error| format!("Invalid if condition: {error}"))?;

    let then_calls = generate_child_calls(&if_block.then_children)?;

    match &if_block.else_children {
        Some(else_children) if if_block.then_children.len() == 1 && else_children.len() == 1 => {
            let then_expr = generate_node(&if_block.then_children[0])?;
            let else_expr = generate_node(&else_children[0])?;
            Ok(quote! {
                if #condition {
                    ::gpui::IntoElement::into_any_element(#then_expr)
                } else {
                    ::gpui::IntoElement::into_any_element(#else_expr)
                }
            })
        }
        Some(else_children) => {
            let else_calls = generate_child_calls(else_children)?;
            Ok(quote! {
                ::gpui::div()
                    .when(#condition, |__el| __el #(#then_calls)*)
                    .when(!(#condition), |__el| __el #(#else_calls)*)
            })
        }
        None => Ok(quote! {
            ::gpui::div().when(#condition, |__el| __el #(#then_calls)*)
        }),
    }
}

fn generate_for_child_call(for_block: &ForBlock) -> Result<TokenStream2, String> {
    let pattern: TokenStream2 = for_block
        .pattern
        .parse()
        .map_err(|error| format!("Invalid for pattern `{}`: {error}", for_block.pattern))?;
    let iterator: syn::Expr = syn::parse_str(&for_block.iterator)
        .map_err(|error| format!("Invalid for iterator `{}`: {error}", for_block.iterator))?;

    let body_expr = match for_block.body.len() {
        0 => quote! { ::gpui::div() },
        1 => generate_node(&for_block.body[0])?,
        _ => {
            let child_calls = generate_child_calls(&for_block.body)?;
            quote! { ::gpui::div() #(#child_calls)* }
        }
    };

    Ok(quote! {
        .children((#iterator).map(|#pattern| #body_expr))
    })
}

fn generate_for_expr(for_block: &ForBlock) -> Result<TokenStream2, String> {
    let child_call = generate_for_child_call(for_block)?;
    Ok(quote! {
        ::gpui::div() #child_call
    })
}

fn generate_match_child_call(match_block: &MatchBlock) -> Result<TokenStream2, String> {
    let expr: syn::Expr = syn::parse_str(&match_block.expr)
        .map_err(|e| format!("Invalid match expr `{}`: {e}", match_block.expr))?;

    let arms: Vec<TokenStream2> = match_block
        .arms
        .iter()
        .map(generate_match_arm)
        .collect::<Result<_, _>>()?;

    Ok(quote! {
        .child(match #expr {
            #(#arms),*
        })
    })
}

fn generate_match_expr(match_block: &MatchBlock) -> Result<TokenStream2, String> {
    let expr: syn::Expr = syn::parse_str(&match_block.expr)
        .map_err(|e| format!("Invalid match expr `{}`: {e}", match_block.expr))?;

    let arms: Vec<TokenStream2> = match_block
        .arms
        .iter()
        .map(generate_match_arm)
        .collect::<Result<_, _>>()?;

    Ok(quote! {
        match #expr {
            #(#arms),*
        }
    })
}

fn generate_match_arm(arm: &MatchArm) -> Result<TokenStream2, String> {
    let pattern: TokenStream2 = arm
        .pattern
        .parse()
        .map_err(|e| format!("Invalid match pattern `{}`: {e}", arm.pattern))?;

    let body = match arm.body.len() {
        0 => quote! { ::gpui::IntoElement::into_any_element(::gpui::div()) },
        1 => {
            let expr = generate_node(&arm.body[0])?;
            quote! { ::gpui::IntoElement::into_any_element(#expr) }
        }
        _ => {
            let calls = generate_child_calls(&arm.body)?;
            quote! { ::gpui::IntoElement::into_any_element(::gpui::div() #(#calls)*) }
        }
    };

    Ok(quote! { #pattern => { #body } })
}

fn generate_event_handler(handler: &EventHandler) -> Result<TokenStream2, String> {
    let handler_expr = &handler.handler;
    let is_closure = handler_expr.starts_with('{') && handler_expr.ends_with('}');

    // Helper: parse closure or method reference
    let make_listener = |event_type: &str| -> Result<TokenStream2, String> {
        if is_closure {
            let closure: syn::Expr = syn::parse_str(&handler_expr[1..handler_expr.len() - 1])
                .map_err(|e| format!("Invalid {event_type} closure: {e}"))?;
            Ok(closure.into_token_stream())
        } else {
            let method: syn::Ident = syn::parse_str(handler_expr)
                .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
            Ok(quote! { cx.listener(Self::#method) })
        }
    };

    match handler.event.as_str() {
        "click" => {
            let listener = make_listener("click")?;
            Ok(quote! { .on_click(#listener) })
        }
        "input" | "change" => {
            let listener = make_listener("input")?;
            Ok(quote! { .on_input(#listener) })
        }
        "keydown" => {
            if handler.modifiers.is_empty() {
                let listener = make_listener("keydown")?;
                Ok(quote! { .on_key_down(#listener) })
            } else {
                generate_keydown_with_modifiers(handler)
            }
        }
        "keyup" => {
            if handler.modifiers.is_empty() {
                let listener = make_listener("keyup")?;
                Ok(quote! { .on_key_up(#listener) })
            } else {
                generate_keyup_with_modifiers(handler)
            }
        }
        "hover" => {
            let listener = make_listener("hover")?;
            Ok(quote! { .on_hover(#listener) })
        }
        "mousedown" => {
            let listener = make_listener("mousedown")?;
            Ok(quote! { .on_mouse_down(::gpui::MouseButton::Left, #listener) })
        }
        "mouseup" => {
            let listener = make_listener("mouseup")?;
            Ok(quote! { .on_mouse_up(::gpui::MouseButton::Left, #listener) })
        }
        "mousemove" => {
            let listener = make_listener("mousemove")?;
            Ok(quote! { .on_mouse_move(#listener) })
        }
        "scroll" => {
            let listener = make_listener("scroll")?;
            Ok(quote! { .on_scroll_wheel(#listener) })
        }
        _ => Ok(quote! {}),
    }
}

fn generate_keydown_with_modifiers(handler: &EventHandler) -> Result<TokenStream2, String> {
    let handler_expr = &handler.handler;
    let is_closure = handler_expr.starts_with('{') && handler_expr.ends_with('}');

    // Build key filter from modifiers
    // Modifiers can be: "Enter", "Escape", "ctrl+s", "shift+Tab", etc.
    let key_conditions: Vec<TokenStream2> = handler
        .modifiers
        .iter()
        .map(|modifier| {
            let lower = modifier.to_lowercase();
            if lower.contains('+') {
                // ctrl+s, shift+tab, etc.
                let parts: Vec<&str> = lower.split('+').collect();
                let key = parts.last().unwrap_or(&"");
                let mut conds = vec![quote! { __ev.keystroke.key == #key }];
                for part in &parts[..parts.len() - 1] {
                    match *part {
                        "ctrl" | "cmd" => conds.push(quote! { __ev.keystroke.modifiers.command }),
                        "shift" => conds.push(quote! { __ev.keystroke.modifiers.shift }),
                        "alt" | "opt" => conds.push(quote! { __ev.keystroke.modifiers.alt }),
                        _ => {}
                    }
                }
                quote! { (#(#conds)&&*) }
            } else {
                // Simple key: Enter, Escape, Tab, etc.
                let key = match lower.as_str() {
                    "enter" => "enter",
                    "escape" | "esc" => "escape",
                    "tab" => "tab",
                    "backspace" => "backspace",
                    "delete" => "delete",
                    "space" => "space",
                    "arrowup" | "up" => "up",
                    "arrowdown" | "down" => "down",
                    "arrowleft" | "left" => "left",
                    "arrowright" | "right" => "right",
                    other => other,
                };
                quote! { __ev.keystroke.key == #key }
            }
        })
        .collect();

    let call_body = if is_closure {
        let closure: syn::Expr = syn::parse_str(&handler_expr[1..handler_expr.len() - 1])
            .map_err(|e| format!("Invalid keydown closure: {e}"))?;
        quote! { (#closure)(__ev, __window, __cx) }
    } else {
        let method: syn::Ident = syn::parse_str(handler_expr)
            .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
        quote! { Self::#method(this, __ev, __window, __cx) }
    };

    let condition = if key_conditions.is_empty() {
        quote! { true }
    } else {
        quote! { #(#key_conditions)||* }
    };

    Ok(quote! {
        .on_key_down(cx.listener(|this, __ev: &::gpui::KeyDownEvent, __window, __cx| {
            if #condition {
                #call_body
            }
        }))
    })
}

fn generate_keyup_with_modifiers(handler: &EventHandler) -> Result<TokenStream2, String> {
    let handler_expr = &handler.handler;
    let is_closure = handler_expr.starts_with('{') && handler_expr.ends_with('}');

    let key_conditions: Vec<TokenStream2> = handler
        .modifiers
        .iter()
        .map(|modifier| {
            let lower = modifier.to_lowercase();
            let key = match lower.as_str() {
                "enter" => "enter",
                "escape" | "esc" => "escape",
                "tab" => "tab",
                other => other,
            };
            quote! { __ev.keystroke.key == #key }
        })
        .collect();

    let call_body = if is_closure {
        let closure: syn::Expr = syn::parse_str(&handler_expr[1..handler_expr.len() - 1])
            .map_err(|e| format!("Invalid keyup closure: {e}"))?;
        quote! { (#closure)(__ev, __window, __cx) }
    } else {
        let method: syn::Ident = syn::parse_str(handler_expr)
            .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
        quote! { Self::#method(this, __ev, __window, __cx) }
    };

    let condition = if key_conditions.is_empty() {
        quote! { true }
    } else {
        quote! { #(#key_conditions)||* }
    };

    Ok(quote! {
        .on_key_up(cx.listener(|this, __ev: &::gpui::KeyUpEvent, __window, __cx| {
            if #condition {
                #call_body
            }
        }))
    })
}

fn generate_binding(binding: &Binding) -> Result<TokenStream2, String> {
    let value: syn::Expr = syn::parse_str(&binding.value)
        .map_err(|e| format!("Invalid binding value `{}`: {e}", binding.value))?;

    match binding.prop.as_str() {
        "id" => Ok(quote! { .id(#value) }),
        "value" => Ok(quote! { .value(#value) }),
        "checked" => Ok(quote! { .checked(#value) }),
        "disabled" => Ok(quote! { .when(#value, |el| el.cursor_not_allowed().opacity(0.5)) }),
        _ => Ok(quote! {}),
    }
}

fn element_tag_to_tokens(tag: &str) -> TokenStream2 {
    match tag {
        "div" | "section" | "article" | "main" | "header" | "footer" | "nav" | "aside"
        | "figure" | "ul" | "ol" | "li" => quote! { ::gpui::div() },
        "span" | "p" | "label" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "strong" | "em"
        | "small" => quote! { ::gpui::div() },
        "button" => quote! { ::gpui::div().cursor_pointer() },
        "input" => quote! { ::gpui::div() },
        "img" | "image" => quote! { ::gpui::div() },
        _ => {
            if tag
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                if let Ok(path) = syn::parse_str::<syn::Path>(tag) {
                    quote! { #path::new() }
                } else {
                    quote! { ::gpui::div() }
                }
            } else {
                quote! { ::gpui::div() }
            }
        }
    }
}

// Needed for make_listener closure
trait IntoTokenStreamHelper {
    fn into_token_stream(self) -> TokenStream2;
}
impl IntoTokenStreamHelper for syn::Expr {
    fn into_token_stream(self) -> TokenStream2 {
        quote! { #self }
    }
}

// ============================================================
// Tailwind → GPUI mapper
// ============================================================

fn map_class(class: &str) -> Option<TokenStream2> {
    // Target-conditional classes: gpui:h-2, tui:h-1, web:p-4
    // GPUI macro always targets gpui — strip matching prefixes, skip others.
    if let Some((prefix, rest)) = class.split_once(':') {
        match prefix {
            "gpui" | "gui" => return map_class(rest),
            "tui" | "web" | "ios" | "android" | "macos" | "darwin" | "windows" | "win32"
            | "linux" => return None,
            _ => {}
        }
    }

    if let Some(rest) = class.strip_prefix("hover:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .hover(|__s| __s #style) });
    }
    if let Some(rest) = class.strip_prefix("focus:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .focus(|__s| __s #style) });
    }
    if let Some(rest) = class.strip_prefix("active:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .active(|__s| __s #style) });
    }

    map_class_to_style(class)
}

fn gpui_text_run_class(class: &str) -> Option<TokenStream2> {
    match class {
        "uppercase" => Some(quote! { .text_transform(::gpui::TextTransform::Uppercase) }),
        "lowercase" => Some(quote! { .text_transform(::gpui::TextTransform::Lowercase) }),
        "capitalize" => Some(quote! { .text_transform(::gpui::TextTransform::Capitalize) }),
        "normal-case" => Some(quote! { .text_transform(::gpui::TextTransform::None) }),
        "tracking-tighter" => Some(quote! { .letter_spacing(::gpui::px(-2.0)) }),
        "tracking-tight" => Some(quote! { .letter_spacing(::gpui::px(-1.0)) }),
        "tracking-normal" => Some(quote! { .letter_spacing(::gpui::px(0.0)) }),
        "tracking-wide" => Some(quote! { .letter_spacing(::gpui::px(1.5)) }),
        "tracking-wider" => Some(quote! { .letter_spacing(::gpui::px(3.0)) }),
        "tracking-widest" => Some(quote! { .letter_spacing(::gpui::px(5.0)) }),
        _ => None,
    }
}

fn map_class_to_style(class: &str) -> Option<TokenStream2> {
    if let Some(tokens) = gpui_text_run_class(class) {
        return Some(tokens);
    }

    // ---------- Static classes ----------
    let static_result: Option<TokenStream2> = match class {
        // Layout / display
        "flex" => Some(quote! { .flex() }),
        "flex-col" => Some(quote! { .flex_col() }),
        "flex-row" => Some(quote! { .flex_row() }),
        "flex-wrap" => Some(quote! { .flex_wrap() }),
        "flex-nowrap" => Some(quote! { .flex_nowrap() }),
        "flex-1" => Some(quote! { .flex_1() }),
        "grow" => Some(quote! { .grow() }),
        "grow-0" => Some(quote! { .grow_0() }),
        "shrink" => Some(quote! { .shrink() }),
        "shrink-0" => Some(quote! { .flex_shrink_0() }),
        "inline-flex" | "inline" => Some(quote! { .flex() }),
        "block" => Some(quote! { .block() }),
        "hidden" => Some(quote! { .hidden() }),
        "invisible" => Some(quote! { .invisible() }),
        "visible" => Some(quote! { .visible() }),
        "grid" => Some(quote! { .grid() }),
        "absolute" => Some(quote! { .absolute() }),
        "relative" => Some(quote! { .relative() }),
        "fixed" => Some(quote! { .fixed() }),
        "sticky" => Some(quote! { .sticky() }),
        "overflow-hidden" => Some(quote! { .overflow_hidden() }),
        "overflow-auto" => Some(quote! { .overflow_y_auto().overflow_x_auto() }),
        "overflow-x-auto" => Some(quote! { .overflow_x_auto() }),
        "overflow-y-auto" => Some(quote! { .overflow_y_auto() }),
        "overflow-x-hidden" => Some(quote! { .overflow_x_hidden() }),
        "overflow-y-hidden" => Some(quote! { .overflow_y_hidden() }),
        "overflow-scroll" => Some(quote! { .overflow_y_scroll() }),
        "overflow-y-scroll" => Some(quote! { .overflow_y_scroll() }),

        // Sizing shortcuts
        "w-full" => Some(quote! { .w_full() }),
        "h-full" => Some(quote! { .h_full() }),
        "w-screen" => Some(quote! { .w_full() }),
        "h-screen" => Some(quote! { .h_full() }),
        "w-auto" => Some(quote! { .w_auto() }),
        "h-auto" => Some(quote! { .h_auto() }),
        "min-w-0" => Some(quote! { .min_w_0() }),
        "min-h-0" => Some(quote! { .min_h_0() }),
        "w-px" => Some(quote! { .w_px() }),
        "h-px" => Some(quote! { .h_px() }),
        "w-0" => Some(quote! { .w(::gpui::px(0.)) }),
        "h-0" => Some(quote! { .h(::gpui::px(0.)) }),

        // Flexbox alignment
        "justify-center" => Some(quote! { .justify_center() }),
        "justify-start" => Some(quote! { .justify_start() }),
        "justify-end" => Some(quote! { .justify_end() }),
        "justify-between" => Some(quote! { .justify_between() }),
        "justify-around" => Some(quote! { .justify_around() }),
        "items-center" => Some(quote! { .items_center() }),
        "items-start" => Some(quote! { .items_start() }),
        "items-end" => Some(quote! { .items_end() }),
        "items-stretch" => None, // no items_stretch() in GPUI 0.2.x
        "items-baseline" => Some(quote! { .items_baseline() }),
        // self-* (align-self) not available as methods in GPUI 0.2.x
        "self-stretch" | "self-center" | "self-start" | "self-end" | "self-auto" => None,
        "content-center" => Some(quote! { .content_center() }),
        "content-start" => Some(quote! { .content_start() }),
        "content-end" => Some(quote! { .content_end() }),

        // Colors — background
        "bg-black" => Some(quote! { .bg(::gpui::black()) }),
        "bg-white" => Some(quote! { .bg(::gpui::white()) }),
        "bg-transparent" => Some(quote! { .bg(::gpui::transparent_black()) }),

        // Colors — text
        "text-white" => Some(quote! { .text_color(::gpui::white()) }),
        "text-black" => Some(quote! { .text_color(::gpui::black()) }),
        "text-transparent" => Some(quote! { .text_color(::gpui::transparent_black()) }),

        // Colors — border
        "border-white" => Some(quote! { .border_color(::gpui::white()) }),
        "border-black" => Some(quote! { .border_color(::gpui::black()) }),

        // Border width
        "border" => Some(quote! { .border_1() }),
        "border-0" => Some(quote! { .border_0() }),
        "border-2" => Some(quote! { .border_2() }),
        "border-4" => Some(quote! { .border_4() }),
        "border-8" => Some(quote! { .border_8() }),
        "border-t" => Some(quote! { .border_t_1() }),
        "border-b" => Some(quote! { .border_b_1() }),
        "border-l" => Some(quote! { .border_l_1() }),
        "border-r" => Some(quote! { .border_r_1() }),
        "outline-none" | "outline-0" => None,

        // Border radius
        "rounded-none" => Some(quote! { .rounded_none() }),
        "rounded-xs" => Some(quote! { .rounded_xs() }),
        "rounded-sm" => Some(quote! { .rounded_sm() }),
        "rounded" | "rounded-md" => Some(quote! { .rounded_md() }),
        "rounded-lg" => Some(quote! { .rounded_lg() }),
        "rounded-xl" => Some(quote! { .rounded_xl() }),
        "rounded-2xl" => Some(quote! { .rounded_2xl() }),
        "rounded-3xl" => Some(quote! { .rounded_3xl() }),
        "rounded-full" => Some(quote! { .rounded_full() }),
        "rounded-t-lg" => Some(quote! { .rounded_t_lg() }),
        "rounded-b-lg" => Some(quote! { .rounded_b_lg() }),

        // Typography — weight
        "font-thin" => Some(quote! { .font_weight(::gpui::FontWeight::THIN) }),
        "font-light" => Some(quote! { .font_weight(::gpui::FontWeight::LIGHT) }),
        "font-normal" => Some(quote! { .font_weight(::gpui::FontWeight::NORMAL) }),
        "font-medium" => Some(quote! { .font_weight(::gpui::FontWeight::MEDIUM) }),
        "font-semibold" => Some(quote! { .font_weight(::gpui::FontWeight::SEMIBOLD) }),
        "font-bold" => Some(quote! { .font_weight(::gpui::FontWeight::BOLD) }),
        "font-extrabold" => Some(quote! { .font_weight(::gpui::FontWeight::EXTRA_BOLD) }),
        "font-black" => Some(quote! { .font_weight(::gpui::FontWeight::BLACK) }),
        "font-italic" => Some(quote! { .font_style(::gpui::FontStyle::Italic) }),

        // Typography — scale
        "text-xs" => Some(quote! { .text_size(::gpui::rems(0.75)) }),
        "text-sm" => Some(quote! { .text_size(::gpui::rems(0.875)) }),
        "text-base" => Some(quote! { .text_size(::gpui::rems(1.)) }),
        "text-lg" => Some(quote! { .text_size(::gpui::rems(1.125)) }),
        "text-xl" => Some(quote! { .text_size(::gpui::rems(1.25)) }),
        "text-2xl" => Some(quote! { .text_size(::gpui::rems(1.5)) }),
        "text-3xl" => Some(quote! { .text_size(::gpui::rems(1.875)) }),
        "text-4xl" => Some(quote! { .text_size(::gpui::rems(2.25)) }),
        "text-5xl" => Some(quote! { .text_size(::gpui::rems(3.)) }),
        "text-6xl" => Some(quote! { .text_size(::gpui::rems(3.75)) }),
        "text-7xl" => Some(quote! { .text_size(::gpui::rems(4.5)) }),
        "text-8xl" => Some(quote! { .text_size(::gpui::rems(6.)) }),
        "text-9xl" => Some(quote! { .text_size(::gpui::rems(8.)) }),

        // Typography — alignment
        "text-left" => Some(quote! { .text_align(::gpui::TextAlign::Left) }),
        "text-center" => Some(quote! { .text_align(::gpui::TextAlign::Center) }),
        "text-right" => Some(quote! { .text_align(::gpui::TextAlign::Right) }),

        // Typography — line height
        "leading-none" => Some(quote! { .line_height(::gpui::relative(1.)) }),
        "leading-tight" => Some(quote! { .line_height(::gpui::relative(1.25)) }),
        "leading-snug" => Some(quote! { .line_height(::gpui::relative(1.375)) }),
        "leading-normal" => Some(quote! { .line_height(::gpui::relative(1.5)) }),
        "leading-relaxed" => Some(quote! { .line_height(::gpui::relative(1.625)) }),
        "leading-loose" => Some(quote! { .line_height(::gpui::relative(2.)) }),

        // Typography — misc
        "truncate" => Some(quote! { .truncate() }),
        "whitespace-nowrap" => Some(quote! { .whitespace_nowrap() }),
        "whitespace-normal" => Some(quote! { .whitespace_normal() }),
        "whitespace-pre" => Some(quote! { .whitespace_pre() }),

        // Cursor
        "cursor-pointer" => Some(quote! { .cursor_pointer() }),
        "cursor-default" => Some(quote! { .cursor_default() }),
        "cursor-text" => Some(quote! { .cursor_text() }),
        "cursor-not-allowed" => Some(quote! { .cursor_not_allowed() }),
        "cursor-crosshair" => Some(quote! { .cursor_crosshair() }),
        "cursor-grab" => Some(quote! { .cursor_grab() }),
        "cursor-grabbing" => Some(quote! { .cursor_grabbing() }),
        "cursor-col-resize" => Some(quote! { .cursor_col_resize() }),
        "cursor-row-resize" => Some(quote! { .cursor_row_resize() }),

        // Selection / misc
        "select-none" => Some(quote! { .select_none() }),
        "pointer-events-none" => Some(quote! { .pointer_events_none() }),

        // Explicit ignore list (not supported in GPUI)
        "transition-colors"
        | "transition"
        | "duration-200"
        | "ease-in-out"
        | "appearance-none"
        | "focus-visible:outline-none"
        | "focus-visible:ring-0" => None,

        _ => None,
    };

    if static_result.is_some() {
        return static_result;
    }

    parse_dynamic_class(class)
}

fn parse_dynamic_class(class: &str) -> Option<TokenStream2> {
    // Spacing / sizing: w-N, h-N, p-N, m-N, gap-N, top-N, etc.
    const SPACING_PREFIXES: &[(&str, &str)] = &[
        ("w-", "w"),
        ("h-", "h"),
        ("size-", "size"),
        ("min-w-", "min_w"),
        ("min-h-", "min_h"),
        ("max-w-", "max_w"),
        ("max-h-", "max_h"),
        ("p-", "p"),
        ("px-", "px"),
        ("py-", "py"),
        ("pt-", "pt"),
        ("pb-", "pb"),
        ("pl-", "pl"),
        ("pr-", "pr"),
        ("m-", "m"),
        ("mx-", "mx"),
        ("my-", "my"),
        ("mt-", "mt"),
        ("mb-", "mb"),
        ("ml-", "ml"),
        ("mr-", "mr"),
        ("gap-", "gap"),
        ("gap-x-", "gap_x"),
        ("gap-y-", "gap_y"),
        ("top-", "top"),
        ("right-", "right"),
        ("bottom-", "bottom"),
        ("left-", "left"),
        ("inset-", "inset"),
    ];

    for (prefix, method) in SPACING_PREFIXES {
        if let Some(rest) = class.strip_prefix(prefix) {
            if let Some(tokens) = length_value_to_tokens(rest) {
                let method_ident = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method_ident(#tokens) });
            }
        }
    }

    // text-[Npx] — arbitrary font size
    if let Some(rest) = class.strip_prefix("text-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(tokens) = arbitrary_length(inner) {
                return Some(quote! { .text_size(#tokens) });
            }
        }
    }

    // tracking-[Npx] / tracking-[Nrem]
    if let Some(rest) = class.strip_prefix("tracking-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(tokens) = arbitrary_length(inner) {
                return Some(quote! { .letter_spacing(#tokens) });
            }
        }
    }

    // font-['Family_Name'] or font-[Family_Name] — font family
    if let Some(rest) = class.strip_prefix("font-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            let family = inner.trim_matches('\'').trim_matches('"').replace('_', " ");
            return Some(quote! { .font_family(#family) });
        }
    }

    // opacity-N (0–100)
    if let Some(rest) = class.strip_prefix("opacity-") {
        if let Ok(n) = rest.parse::<f32>() {
            let value = n / 100.0;
            return Some(quote! { .opacity(#value) });
        }
    }

    // z-N
    if let Some(rest) = class.strip_prefix("z-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(quote! { .z_index(#n) });
        }
    }

    // Full Tailwind color palette: bg-{family}-{shade}, text-{family}-{shade}, border-{family}-{shade}
    for (prefix, method) in &[
        ("bg-", "bg"),
        ("text-", "text_color"),
        ("border-", "border_color"),
    ] {
        if let Some(rest) = class.strip_prefix(prefix) {
            // Dynamic color: bg-{theme.bg}, text-{theme.text}, border-{theme.border}
            if let Some(expr_str) = extract_braced_expr_str(rest) {
                let expr: syn::Expr = syn::parse_str(&expr_str)
                    .map_err(|error| format!("Invalid color expression `{expr_str}`: {error}"))
                    .ok()?;
                let method_ident = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method_ident(::gpui::rgb(#expr)) });
            }

            // Arbitrary color: bg-[#rrggbb]
            if rest.starts_with('[') && rest.ends_with(']') {
                let inner = &rest[1..rest.len() - 1];
                if let Some(hex_str) = inner.strip_prefix('#') {
                    if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                        let method_ident = syn::Ident::new(method, Span::call_site());
                        return Some(quote! { .#method_ident(::gpui::rgb(#hex)) });
                    }
                }
            }

            // Named color family: {family}-{shade}
            if let Some(dash_pos) = rest.rfind('-') {
                let family = &rest[..dash_pos];
                let shade = &rest[dash_pos + 1..];
                if let Some(hex) = tailwind_color(family, shade) {
                    let method_ident = syn::Ident::new(method, Span::call_site());
                    return Some(quote! { .#method_ident(::gpui::rgb(#hex)) });
                }
            }
        }
    }

    None
}

fn length_token_to_tokens(token: LengthToken) -> TokenStream2 {
    match token {
        LengthToken::Auto => quote! { ::gpui::Length::Auto },
        LengthToken::Fraction(f) => quote! { ::gpui::relative(#f) },
        LengthToken::Px(n) => quote! { ::gpui::px(#n) },
        LengthToken::Rems(n) => quote! { ::gpui::rems(#n) },
    }
}

fn length_value_to_tokens(value: &str) -> Option<TokenStream2> {
    parse_length_token(value).map(length_token_to_tokens)
}

fn arbitrary_length(inner: &str) -> Option<TokenStream2> {
    parse_arbitrary_length_token(inner).map(length_token_to_tokens)
}

// ============================================================
// Tailwind CSS v4 color palette (via crepuscularity-core)
// ============================================================

fn tailwind_color(family: &str, shade: &str) -> Option<u32> {
    crepuscularity_core::tailwind::lookup_color_u32(&format!("{family}-{shade}"))
}

/// Parse-check a `.crepus` template at compile time and expand to its source `&str`.
///
/// Equivalent to `include_str!(…)` plus a build-time parse error if the template is invalid.
#[proc_macro]
pub fn embedded_template(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let path_value = path.value();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let full = std::path::Path::new(&manifest_dir).join(&path_value);
    let source = std::fs::read_to_string(&full).unwrap_or_else(|e| {
        panic!("embedded_template!: could not read {}: {e}", full.display());
    });
    if let Err(err) = crepuscularity_core::parser::parse_template(&source) {
        panic!(
            "embedded_template!: invalid .crepus at {}: {err}",
            full.display()
        );
    }
    quote! {{
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #path_value))
    }}
    .into()
}
