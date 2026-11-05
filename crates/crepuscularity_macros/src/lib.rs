use crepuscularity_core::tailwind::{
    parse_arbitrary_length_token, parse_length_token, parse_opacity_percentage, resolve_css_color,
    LengthToken,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};
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
/// Named method handlers and filtered key handlers require a GPUI view context named `cx`.
/// Untyped filtered closures receive that view context; explicitly typed callbacks may use `App`.
/// Bound closures receive their event, window, and context from GPUI.
/// Callback expressions, including factory calls and blocks, are evaluated at most once per element build.
/// Bare handler names refer to component methods; parenthesize a callback variable to pass its value.
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
/// Declarations follow sibling order and stay within their parent scope.
/// `$: default` requires a runtime template context and is rejected by `view!`;
/// resolve optional values in Rust before invoking the macro.
///
/// ## Control flow
/// ```text
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
/// Elements with key-down or key-up handlers automatically participate in the tab order.
///
/// ## Events
/// `@click`, `@keydown`, `@keyup`, `@hover`, `@mousedown`, `@mouseup`
/// `@input` and `@change` require a custom component providing `on_input`.
///
/// ## Bindings
/// `bind:id={expression}` — element identity
/// `bind:value` and `bind:checked` require custom component builder methods; they do not write back.
/// Built-in input tags render containers rather than editable controls.
/// `bind:disabled={condition}` — suppresses generated handlers and automatic Tab navigation, with disabled styling.
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
/// File includes are expanded as text. Explicit include props, selected named
/// component defaults, and authored include slots produce compilation errors.
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
    let (expanded, dependencies) = expand_includes_text(&template, &base_dir, &full);
    let dependencies = template_dependencies(&dependencies);
    let view = generate_view(&expanded, path.span());
    quote! {{ #dependencies #view }}.into()
}

// shortcut: symlink retargets need a forced rebuild until Cargo preserves alias dependencies.
fn template_dependencies(paths: &BTreeSet<PathBuf>) -> TokenStream2 {
    let paths = paths.iter().map(|path| path.to_string_lossy());
    quote! { #(const _: &str = ::core::include_str!(#paths);)* }
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
) -> (String, BTreeSet<PathBuf>) {
    let mut dependencies = BTreeSet::from([source_path.to_path_buf()]);
    let mut cache = std::collections::HashMap::new();
    let expanded = expand_includes_text_with_depth(
        template,
        base_dir,
        source_path,
        0,
        &mut dependencies,
        &mut cache,
    );
    (expanded, dependencies)
}

fn expand_includes_text_with_depth(
    template: &str,
    base_dir: &std::path::Path,
    source_path: &std::path::Path,
    depth: usize,
    dependencies: &mut BTreeSet<PathBuf>,
    cache: &mut std::collections::HashMap<PathBuf, std::rc::Rc<String>>,
) -> String {
    if depth > 64 {
        panic!(
            "Include depth limit (64) exceeded in `{}`",
            source_path.display()
        );
    }
    {
        let nodes = crepuscularity_core::parse_template(template).unwrap_or_else(|error| {
            panic!(
                "Invalid view_file! template `{}`: {error}",
                source_path.display()
            )
        });
        let mut pending: Vec<_> = nodes.iter().collect();
        while let Some(node) = pending.pop() {
            use crepuscularity_core::ast::Node;
            match node {
                Node::Include(include) if !include.slot.is_empty() => panic!(
                    "view_file! does not support authored include slots for `{}` in `{}`",
                    include.path,
                    source_path.display()
                ),
                Node::Element(element) => pending.extend(&element.children),
                Node::If(block) => {
                    pending.extend(&block.then_children);
                    if let Some(children) = &block.else_children {
                        pending.extend(children);
                    }
                }
                Node::For(block) => pending.extend(&block.body),
                Node::Match(block) => {
                    for arm in &block.arms {
                        pending.extend(&arm.body);
                    }
                }
                _ => {}
            }
        }
    }
    let mut output = String::new();
    let mut in_text = false;
    for line in template.lines() {
        let trimmed = line.trim_start();
        if in_text || trimmed.starts_with('"') {
            let content = if in_text { trimmed } else { &trimmed[1..] };
            in_text = true;
            let mut escaped = false;
            for character in content.chars() {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_text = false;
                    break;
                }
            }
            output.push_str(line);
            output.push('\n');
            continue;
        }
        if trimmed == "include"
            || trimmed
                .strip_prefix("include")
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            let nodes = crepuscularity_core::parse_template(trimmed).unwrap_or_else(|e| {
                panic!(
                    "Invalid include directive in `{}`: {e}",
                    source_path.display()
                )
            });
            let crepuscularity_core::ast::Node::Include(include) = &nodes[0] else {
                panic!("Expected include directive in `{}`", source_path.display());
            };
            if !include.props.is_empty() {
                panic!(
                    "view_file! does not support include props for `{}` in `{}`",
                    include.path,
                    source_path.display()
                );
            }
            let path_str = include.path.as_str();
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
            dependencies.insert(include_path.clone());
            let section = match component_name {
                Some(name) => extract_component_section(&included, name),
                None => included.to_string(),
            };
            let expanded = expand_includes_text_with_depth(
                &section,
                &include_dir,
                &include_path,
                depth + 1,
                dependencies,
                cache,
            );
            let indent = &line[..line.len() - line.trim_start().len()];
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
    let file = crepuscularity_core::parser::parse_component_file(content)
        .unwrap_or_else(|e| panic!("Invalid multi-component file: {e}"));
    if file
        .components
        .get(name)
        .is_some_and(|component| !component.meta.defaults.is_empty())
    {
        panic!("view_file! does not support defaults for component `{name}`");
    }
    let (_, body, _) = crepuscularity_core::parser::split_frontmatter_parts(content);
    let sections = crepuscularity_core::parser::split_component_body_sections(body);
    sections
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, text, _)| text)
        .unwrap_or_else(|| panic!("Component `{name}` not found in multi-component file"))
}

#[cfg(test)]
mod component_section_tests {
    #[test]
    fn multiline_text_does_not_expand_include_lines() {
        for source in [
            "div\n  \"Literal\n  include nonexistent.crepus\n  still literal\"",
            "div\n  \"Literal \\\" quote\n  include nonexistent.crepus\n  still literal\"",
            "\"Literal\n\\\"escaped quote\ninclude nonexistent.crepus\nclosed\"\nspan",
        ] {
            let (expanded, _) = super::expand_includes_text(
                source,
                std::path::Path::new("."),
                std::path::Path::new("entry.crepus"),
            );
            assert_eq!(expanded, format!("{source}\n"));
            crepuscularity_core::parse_template(&expanded).unwrap();
        }
    }
    #[test]
    #[should_panic(expected = "view_file! does not support defaults for component `Card`")]
    fn named_default_props_are_not_silently_discarded() {
        super::extract_component_section(
            "+++\n[Card.defaults]\ntitle = \"default-title\"\n+++\n--- Card\n\"{title}\"",
            "Card",
        );
    }

    #[test]
    fn authored_include_slots_are_rejected_inside_all_control_bodies() {
        for source in [
            "div\n  include missing.crepus\n    \"provided\"",
            "if true\n  include missing.crepus\n    \"provided\"",
            "if false\n  \"first\"\nelse\n  include missing.crepus\n    \"provided\"",
            "for item in items\n  include missing.crepus\n    \"provided\"",
            "match value\n  _ =>\n    include missing.crepus\n      \"provided\"",
        ] {
            let panic = std::panic::catch_unwind(|| {
                super::expand_includes_text(
                    source,
                    std::path::Path::new("."),
                    std::path::Path::new("slots.crepus"),
                )
            })
            .unwrap_err();
            let message = panic.downcast_ref::<String>().unwrap();
            assert!(
                message.contains("view_file! does not support authored include slots"),
                "{message}"
            );
            assert!(message.contains("slots.crepus"), "{message}");
        }
    }

    #[test]
    fn terminal_storage_field_avoids_template_ids() {
        let path =
            std::env::temp_dir().join(format!("crepus-storage-{}.crepus", std::process::id()));
        std::fs::write(&path, "div #template\nspan #__crepus_template").unwrap();
        let result = super::build_tui_template_refs(&path);
        std::fs::remove_file(&path).unwrap();
        let tokens = result.unwrap();
        let source = tokens.to_string();
        assert!(source.contains("__crepus_template_ : :: crepuscularity_tui :: Template"));
        assert!(source.contains("pub template : :: crepuscularity_tui :: ElementRef"));
        syn::parse2::<syn::Expr>(tokens).unwrap();
    }
    use super::{
        collect_ids_from_path, expand_includes_text, extract_component_section, make_rust_ident,
    };

    #[test]
    fn accessor_names_are_valid_rust_identifiers() {
        for id in ["_", "²", "雪", "type", "9items", "a-b", ""] {
            if let Some(ident) = make_rust_ident(id) {
                assert!(syn::parse_str::<syn::Ident>(&ident.to_string()).is_ok());
            }
        }
    }

    #[test]
    fn named_includes_collect_only_selected_components() {
        let dir = std::env::temp_dir().join(format!("crepus-named-refs-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let entry = dir.join("entry.crepus");
        std::fs::write(
            dir.join("ui.crepus"),
            "--- Card\nspan #card\n--- Button\nbutton #button\n--- Unused\ninclude missing.crepus",
        )
        .unwrap();
        std::fs::write(&entry, "include ui.crepus#Card\ninclude ui.crepus#Button").unwrap();
        let result = collect_ids_from_path(&entry, "crepus_refs!");
        std::fs::write(&entry, "include ui.crepus#Missing").unwrap();
        let missing = collect_ids_from_path(&entry, "crepus_refs!");
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(result.unwrap().0, ["button", "card"]);
        assert!(missing
            .unwrap_err()
            .contains("component \"Missing\" not found"));
    }

    #[test]
    fn reference_collection_uses_filename_frontend() {
        let path =
            std::env::temp_dir().join(format!("crepus-macro-frontend-{}.vue", std::process::id()));
        std::fs::write(
            &path,
            "<script>const ignored = 1 < 2;</script><template><div id=\"root\" /></template>",
        )
        .unwrap();
        let result = collect_ids_from_path(&path, "crepus_refs!");
        std::fs::write(
            &path,
            "<template><div id=\"a-b\" /><div id=\"a_b\" /></template>",
        )
        .unwrap();
        let collision = collect_ids_from_path(&path, "crepus_refs!");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap().0, ["root"]);
        assert!(collision.unwrap_err().contains("same Rust accessor"));
    }

    #[test]
    fn reference_collection_keeps_markup_section_markers_as_text() {
        for (extension, source) in [
            (
                "vue",
                "<template><div id=\"root\">\n--- Card\n</div></template>",
            ),
            ("svelte", "<div id=\"root\">\n--- Card\n</div>"),
        ] {
            let path = std::env::temp_dir().join(format!(
                "crepus-macro-literal-sections-{}.{extension}",
                std::process::id()
            ));
            // These are ordinary template text, accepted by the runtime frontend.
            crepuscularity_core::parse_template_with_path(source, Some(&path)).unwrap();
            std::fs::write(&path, source).unwrap();
            let result = collect_ids_from_path(&path, "crepus_refs!");
            std::fs::remove_file(&path).unwrap();
            assert_eq!(result.unwrap().0, ["root"], "{extension}");
        }
    }

    #[test]
    fn reference_collection_retains_dsl_component_discovery() {
        let path = std::env::temp_dir().join(format!(
            "crepus-macro-dsl-sections-{}.crepus",
            std::process::id()
        ));
        std::fs::write(&path, "--- First\ndiv id=first\n--- Second\ndiv id=second").unwrap();
        let result = collect_ids_from_path(&path, "crepus_refs!");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap().0, ["first", "second"]);
    }

    #[test]
    fn reference_collection_preserves_component_errors() {
        let path = std::env::temp_dir().join(format!(
            "crepus-macro-refs-{}-{}.crepus",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "+++\n[Card]\ndescription = 42\n+++\n--- Card\nspan #lost",
        )
        .unwrap();
        // Diagnostics name the canonical source, which can expand Windows short temp paths.
        let diagnostic_path = path.canonicalize().unwrap();
        let result = collect_ids_from_path(&path, "crepus_refs!");
        std::fs::write(&path, "--- Card\nspan #card\n--- Button\nbutton #button").unwrap();
        let valid = collect_ids_from_path(&path, "crepus_refs!");
        std::fs::remove_file(&path).unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("crepus_refs!"));
        assert!(error.contains("description must be"));
        assert!(
            error.contains(&diagnostic_path.display().to_string()),
            "{error}"
        );
        assert_eq!(valid.unwrap().0, ["button", "card"]);
    }

    #[test]
    fn include_cycles_are_bounded_and_siblings_can_repeat() {
        let dir = std::env::temp_dir().join(format!(
            "crepus-macro-includes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("child.crepus");
        std::fs::write(&file, "include child.crepus").unwrap();
        let result = std::panic::catch_unwind(|| {
            expand_includes_text("include child.crepus", &dir, &dir.join("entry.crepus"))
        });
        std::fs::write(&file, "span").unwrap();
        let expanded = expand_includes_text(
            "include child.crepus\ninclude child.crepus",
            &dir,
            &dir.join("entry.crepus"),
        );
        std::fs::write(dir.join("child view.crepus"), "button").unwrap();
        let quoted = expand_includes_text(
            "include\t\"child view.crepus\"",
            &dir,
            &dir.join("entry.crepus"),
        );
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(quoted.0, "button\n");
        let panic = result.unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        assert!(message.contains("Include depth limit (64) exceeded"));
        assert_eq!(expanded.0, "span\nspan\n");
    }

    #[test]
    #[should_panic(expected = "duplicate component name")]
    fn rejects_duplicate_names() {
        extract_component_section("--- Card\nspan\n--- Card\nbutton", "Card");
    }

    #[test]
    fn selects_named_section() {
        assert_eq!(
            extract_component_section("--- Card\nspan\n--- Button\nbutton", "Button"),
            "button"
        );
    }
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
        Ok((ids, dependencies)) => {
            let dependencies = template_dependencies(&dependencies);
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
                #dependencies
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
    let (ids, dependencies) = collect_ids_from_path(entry_path, "template_refs!")?;
    let dependencies = template_dependencies(&dependencies);
    let mut storage_name = "__crepus_template".to_string();
    while ids
        .iter()
        .any(|id| make_rust_ident(id).is_some_and(|ident| ident == storage_name))
    {
        storage_name.push('_');
    }
    let template_field = syn::Ident::new(&storage_name, Span::call_site());
    let canonical = std::fs::canonicalize(entry_path).unwrap_or_else(|_| entry_path.to_path_buf());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("template_refs! could not read {}: {e}", canonical.display()))?;
    // Reload the authored path, while includes retain the resolved file's directory.
    let path = std::path::absolute(entry_path)
        .map_err(|e| {
            format!(
                "template_refs! could not resolve {}: {e}",
                entry_path.display()
            )
        })?
        .to_string_lossy()
        .to_string();
    let base_dir = match canonical.parent() {
        Some(parent) => {
            let parent = parent.to_string_lossy();
            quote! { Some(::std::path::PathBuf::from(#parent)) }
        }
        None => quote! { None },
    };

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
                    self.#template_field.set(#key, self.#ident.content.clone());
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
        #dependencies
        #[allow(non_camel_case_types)]
        struct __CrepusTuiTemplateRefs {
            #template_field: ::crepuscularity_tui::Template,
            #(#fields),*
        }

        impl __CrepusTuiTemplateRefs {
            pub fn set(
                &mut self,
                key: impl Into<String>,
                value: impl Into<::crepuscularity_tui::TemplateValue>,
            ) -> &mut Self {
                self.#template_field.set(key, value);
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
                &self.#template_field
            }

            pub fn template_mut(&mut self) -> &mut ::crepuscularity_tui::Template {
                &mut self.#template_field
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
                self.#template_field.draw(frame, area)
            }

            pub fn draw_full(
                &mut self,
                frame: &mut ::crepuscularity_tui::ratatui::Frame,
            ) -> Result<(), String> {
                self.sync_refs();
                self.#template_field.draw_full(frame)
            }
        }

        Ok::<__CrepusTuiTemplateRefs, String>(__CrepusTuiTemplateRefs {
            #template_field: {
                let mut template = ::crepuscularity_tui::Template::from_source_with_path(#source, #path);
                template.context_mut().base_dir = #base_dir;
                template
            },
            #(#inits),*
        })
    }})
}

fn collect_ids_from_path(
    entry_path: &Path,
    macro_name: &str,
) -> Result<(Vec<String>, BTreeSet<PathBuf>), String> {
    let mut seen = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut dependencies = BTreeSet::new();
    collect_ids_recursive(
        entry_path,
        None,
        macro_name,
        &mut seen,
        &mut ids,
        &mut dependencies,
    )?;
    let mut fields = std::collections::BTreeMap::new();
    for id in &ids {
        let ident = make_rust_ident(id)
            .ok_or_else(|| format!("{macro_name} ID {id:?} cannot form a valid Rust accessor"))?;
        if let Some(previous) = fields.insert(ident.to_string(), id) {
            return Err(format!(
                "{macro_name} IDs {previous:?} and {id:?} generate the same Rust accessor {ident}"
            ));
        }
    }
    Ok((ids.into_iter().collect(), dependencies))
}

fn collect_ids_recursive(
    path: &Path,
    component_name: Option<&str>,
    macro_name: &str,
    seen: &mut BTreeSet<(PathBuf, Option<String>, Option<PathBuf>)>,
    ids: &mut BTreeSet<String>,
    dependencies: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    dependencies.insert(path.to_path_buf());
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let frontend = component_name.is_none().then(|| {
        crepuscularity_core::include_paths::include_frontend_path(
            &canonical,
            &path.to_string_lossy(),
        )
    });
    if !seen.insert((
        canonical.clone(),
        component_name.map(str::to_owned),
        frontend.clone(),
    )) {
        return Ok(());
    }

    let content = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("{macro_name} could not read {}: {e}", canonical.display()))?;
    if component_name.is_some()
        || !crepuscularity_core::parser::is_markup_mode(&content, frontend.as_deref())
    {
        let file = crepuscularity_core::parse_component_file(&content)
            .map_err(|e| format!("{macro_name} parse error in {}: {e}", canonical.display()))?;
        if let Some(name) = component_name {
            let component = file.components.get(name).ok_or_else(|| {
                format!(
                    "{macro_name} component {name:?} not found in {}",
                    canonical.display()
                )
            })?;
            return collect_ids_from_nodes(
                &component.nodes,
                canonical.parent(),
                macro_name,
                seen,
                ids,
                dependencies,
            );
        }
        if !file.components.is_empty() {
            for component in file.components.values() {
                collect_ids_from_nodes(
                    &component.nodes,
                    canonical.parent(),
                    macro_name,
                    seen,
                    ids,
                    dependencies,
                )?;
            }
            return Ok(());
        }
    }
    let nodes = crepuscularity_core::parse_template_with_path(&content, frontend.as_deref())
        .map_err(|e| format!("{macro_name} parse error in {}: {e}", canonical.display()))?;
    collect_ids_from_nodes(
        &nodes,
        canonical.parent(),
        macro_name,
        seen,
        ids,
        dependencies,
    )
}

fn collect_ids_from_nodes(
    nodes: &[crepuscularity_core::ast::Node],
    base_dir: Option<&Path>,
    macro_name: &str,
    seen: &mut BTreeSet<(PathBuf, Option<String>, Option<PathBuf>)>,
    ids: &mut BTreeSet<String>,
    dependencies: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    use crepuscularity_core::ast::Node;

    for node in nodes {
        match node {
            Node::Element(el) => {
                if let Some(id) = &el.id {
                    ids.insert(id.clone());
                }
                collect_ids_from_nodes(
                    &el.children,
                    base_dir,
                    macro_name,
                    seen,
                    ids,
                    dependencies,
                )?;
            }
            Node::If(block) => {
                collect_ids_from_nodes(
                    &block.then_children,
                    base_dir,
                    macro_name,
                    seen,
                    ids,
                    dependencies,
                )?;
                if let Some(else_children) = &block.else_children {
                    collect_ids_from_nodes(
                        else_children,
                        base_dir,
                        macro_name,
                        seen,
                        ids,
                        dependencies,
                    )?;
                }
            }
            Node::For(block) => {
                collect_ids_from_nodes(&block.body, base_dir, macro_name, seen, ids, dependencies)?
            }
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_ids_from_nodes(
                        &arm.body,
                        base_dir,
                        macro_name,
                        seen,
                        ids,
                        dependencies,
                    )?;
                }
            }
            Node::Include(inc) => {
                let (include_path, component_name) = match inc.path.split_once('#') {
                    Some((path, name)) => (path, Some(name)),
                    None => (inc.path.as_str(), None),
                };
                let resolved = if let Some(base) = base_dir {
                    base.join(include_path)
                } else {
                    PathBuf::from(include_path)
                };
                collect_ids_recursive(
                    &resolved,
                    component_name,
                    macro_name,
                    seen,
                    ids,
                    dependencies,
                )?;
                collect_ids_from_nodes(&inc.slot, base_dir, macro_name, seen, ids, dependencies)?;
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
    } else {
        syn::parse_str::<syn::Ident>(&id)
            .or_else(|_| syn::parse_str::<syn::Ident>(&format!("_{id}")))
            .ok()
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
    origin: crepuscularity_core::ast::BindingOrigin,
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
                    origin: b.origin,
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
        ast::Node::LetDecl(decl) => {
            if decl.is_default {
                return Err("$: default requires a runtime template context; view! cannot test whether a Rust binding exists. Set the default in Rust before invoking view!.".into());
            }
            Ok(Node::LetDecl(LetDecl {
                name: decl.name.clone(),
                expr: decl.expr.clone(),
            }))
        }
        ast::Node::RawHtml(_) | ast::Node::Include(_) | ast::Node::Embed(_) => {
            Err("node type not supported in view! macro".to_string())
        }
    }
}

fn extract_braced_expr_str(s: &str) -> Option<String> {
    Some(s.strip_prefix('{')?.strip_suffix('}')?.trim().to_string())
}

// ============================================================
// Code generator
// ============================================================

fn generate_root(nodes: &[Node]) -> Result<TokenStream2, String> {
    let path = syn::Ident::new("__crepus_loop_path", Span::mixed_site());
    let root = generate_root_with_path(nodes)?;
    Ok(quote! {{ let #path = ::std::string::String::new(); #root }})
}

fn generate_root_with_path(nodes: &[Node]) -> Result<TokenStream2, String> {
    let first_element = nodes
        .iter()
        .position(|node| !matches!(node, Node::LetDecl(_)));
    if first_element.is_some_and(|index| {
        nodes[index + 1..]
            .iter()
            .any(|node| matches!(node, Node::LetDecl(_)))
    }) {
        if nodes
            .iter()
            .filter(|node| !matches!(node, Node::LetDecl(_)))
            .count()
            == 1
        {
            let root = syn::Ident::new("__crepus_root", Span::mixed_site());
            let statements = nodes
                .iter()
                .map(|node| {
                    if let Node::LetDecl(decl) = node {
                        generate_let_decl(decl)
                    } else {
                        generate_node(node).map(|element| quote! { let #root = #element; })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(quote! {{ #(#statements)* #root }});
        }
        let calls = generate_child_calls(nodes)?;
        return Ok(quote! { ::gpui::div() #(#calls)* });
    }
    // Leading declarations share the root expression's scope.
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
    generate_child_calls_refs(&nodes.iter().collect::<Vec<_>>())
}

fn generate_child_calls_refs(nodes: &[&Node]) -> Result<Vec<TokenStream2>, String> {
    if !nodes.iter().any(|node| matches!(node, Node::LetDecl(_))) {
        return nodes.iter().map(|node| generate_child_call(node)).collect();
    }
    let parent = syn::Ident::new("__crepus_parent", Span::mixed_site());
    let mut statements = Vec::new();
    for node in nodes {
        if let Node::LetDecl(decl) = node {
            statements.push(generate_let_decl(decl)?);
        } else {
            let call = generate_child_call(node)?;
            statements.push(quote! { let #parent = #parent #call; });
        }
    }
    Ok(vec![quote! { .map(|#parent| {
        #(#statements)*
        #parent
    }) }])
}

fn generate_child_call(node: &Node) -> Result<TokenStream2, String> {
    match node {
        Node::If(if_block) => generate_if_child_call(if_block),
        Node::For(for_block) => generate_for_child_call(for_block),
        Node::Match(match_block) => generate_match_child_call(match_block),
        Node::LetDecl(_) => Err("Child declarations require a sibling scope".into()),
        other => {
            let expr = generate_node(other)?;
            Ok(quote! { .child(#expr) })
        }
    }
}

fn generate_element(element: &Element) -> Result<TokenStream2, String> {
    for class in element.classes.iter().map(String::as_str).chain(
        element
            .conditional_classes
            .iter()
            .map(|class| class.class.as_str()),
    ) {
        validate_dynamic_color_class(class)?;
    }
    if let Some(binding) = element
        .bindings
        .iter()
        .find(|binding| binding.origin == crepuscularity_core::ast::BindingOrigin::Attribute)
    {
        return Err(format!(
            "GPUI macros do not support HTML attribute binding `attr.{}`",
            binding.prop
        ));
    }
    if !element.tag.chars().next().is_some_and(char::is_uppercase) {
        for binding in &element.bindings {
            if matches!(binding.prop.as_str(), "value" | "checked") {
                return Err(format!(
                    "GPUI bind:{} on <{}> requires a custom editable component",
                    binding.prop, element.tag
                ));
            }
        }
        for handler in &element.event_handlers {
            if matches!(handler.event.as_str(), "input" | "change") {
                return Err(format!(
                    "GPUI @{} on <{}> requires a custom editable component",
                    handler.event, element.tag
                ));
            }
        }
    }
    let disabled = syn::Ident::new("__crepus_disabled", Span::mixed_site());
    let disabled_binding = element
        .bindings
        .iter()
        .rev()
        .find(|binding| binding.prop == "disabled");
    let disabled_setup = disabled_binding.map(|binding| {
        let value: syn::Expr = syn::parse_str(&binding.value)
            .map_err(|error| format!("Invalid disabled binding: {error}"))?;
        let value = if matches!(&value, syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(string) if string.value().is_empty())) {
            quote! { true }
        } else {
            quote! { #value }
        };
        Ok::<_, String>(quote! { let #disabled: bool = #value; })
    }).transpose()?;
    let enabled = if disabled_binding.is_some() {
        quote! { !#disabled }
    } else {
        quote! { true }
    };
    let tag_expr = element_tag_to_tokens(&element.tag);
    let state_style = syn::Ident::new("__s", Span::mixed_site());

    let mut class_methods = Vec::new();
    let mut class_values = Vec::new();
    let mut state_styles: BTreeMap<&str, Vec<TokenStream2>> = BTreeMap::new();
    let mut state_shadow_conditions: BTreeMap<&str, Vec<TokenStream2>> = BTreeMap::new();
    for class in &element.classes {
        if let Some(expression) = class.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            let expression: syn::Expr = syn::parse_str(expression)
                .map_err(|error| format!("Invalid class expression: {error}"))?;
            let name = syn::Ident::new(
                &format!("__crepus_class_list_{}", class_values.len()),
                Span::mixed_site(),
            );
            class_values.push(quote! { let #name = (#expression).to_string(); });
            class_methods.push(quote! {
                .map(|__el| ::crepuscularity_gpui::crepuscularity_runtime::styler::apply_class_list(__el, &#name))
            });
            for state in ["hover", "focus", "active"] {
                state_shadow_conditions.entry(state).or_default().push(quote! {
                    ::crepuscularity_gpui::crepuscularity_runtime::styler::class_list_has_state_shadow(&#name, #state)
                });
                state_styles.entry(state).or_default().push(quote! {
                    let #state_style = ::crepuscularity_gpui::crepuscularity_runtime::styler::apply_class_list_state_style(#state_style, &#name, #state);
                });
            }
            continue;
        }
        let Some(class) = gpui_class(class) else {
            continue;
        };
        if let Some((state, rest)) = class.split_once(':') {
            if matches!(state, "hover" | "focus" | "active") {
                let Some(rest) = gpui_class(rest) else {
                    continue;
                };
                if let Some(style) = map_class_to_style(rest) {
                    if rest == "ring"
                        || rest.starts_with("ring-")
                        || rest == "shadow"
                        || rest.starts_with("shadow-")
                    {
                        state_shadow_conditions
                            .entry(state)
                            .or_default()
                            .push(quote! { true });
                    }
                    state_styles
                        .entry(state)
                        .or_default()
                        .push(quote! { let #state_style = #state_style #style; });
                }
                continue;
            }
        }
        if let Some(method) = map_class(class) {
            class_methods.push(method);
        }
    }
    for class in &element.conditional_classes {
        let Some((state, utility)) =
            gpui_class(&class.class).and_then(|class| class.split_once(':'))
        else {
            continue;
        };
        if !matches!(state, "hover" | "focus" | "active") {
            continue;
        }
        let Some(utility) = gpui_class(utility) else {
            continue;
        };
        let style = map_class_to_style(utility)
            .ok_or_else(|| format!("Unknown class `{}` in conditional class", class.class))?;
        let condition: syn::Expr = syn::parse_str(&class.condition)
            .map_err(|error| format!("Invalid conditional class expression: {error}"))?;
        let enabled = syn::Ident::new(
            &format!("__crepus_state_enabled_{}", class_values.len()),
            Span::mixed_site(),
        );
        class_values.push(quote! { let #enabled = #condition; });
        if utility == "ring"
            || utility.starts_with("ring-")
            || utility == "shadow"
            || utility.starts_with("shadow-")
        {
            state_shadow_conditions
                .entry(state)
                .or_default()
                .push(quote! { #enabled });
        }
        state_styles.entry(state).or_default().push(quote! {
            let #state_style = if #enabled { #state_style #style } else { #state_style };
        });
    }
    let has_focus_style = state_styles.contains_key("focus");
    let focus_enabled = syn::Ident::new("__crepus_focus_enabled", Span::mixed_site());
    let has_active_style = state_styles.contains_key("active");
    let base_shadow = syn::Ident::new("__crepus_base_shadow", Span::mixed_site());
    let mut state_calls = Vec::new();
    for (state, styles) in state_styles {
        let method = syn::Ident::new(state, Span::call_site());
        let conditions = state_shadow_conditions.remove(state).unwrap_or_default();
        let composes = syn::Ident::new("__composes_shadows", Span::mixed_site());
        let focus_update = (state == "focus").then(|| {
            quote! {
                #focus_enabled.set(#state_style != ::gpui::StyleRefinement::default());
            }
        });
        state_calls.push(quote! { .#method(|mut #state_style| {
            let #composes = false #(|| (#conditions))*;
            if #composes { #state_style.box_shadow = #base_shadow.clone(); }
            #(#styles)*
            let mut #state_style = #state_style;
            if #composes && #state_style.box_shadow == #base_shadow { #state_style.box_shadow = None; }
            #focus_update
            #state_style
        }) });
    }
    let state_call = (!state_calls.is_empty()).then(|| {
        let builder = syn::Ident::new("__el", Span::mixed_site());
        let focus_setup =
            has_focus_style.then(|| quote! { let #focus_enabled = ::std::cell::Cell::new(false); });
        let focus_finish = has_focus_style.then(|| {
            quote! {
                .when(#enabled && #focus_enabled.get(), |#builder| #builder.focusable().tab_index(0))
            }
        });
        quote! { .map(|mut #builder| {
            let #base_shadow = ::gpui::Styled::style(&mut #builder).box_shadow.clone();
            #focus_setup
            let #builder = #builder #(#state_calls)*;
            #builder #focus_finish
        }) }
    });

    let conditional_class_calls: Vec<TokenStream2> = element
        .conditional_classes
        .iter()
        .filter(|class| {
            !gpui_class(&class.class)
                .and_then(|class| class.split_once(':'))
                .is_some_and(|(state, _)| matches!(state, "hover" | "focus" | "active"))
        })
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
        .filter(|binding| !matches!(binding.prop.as_str(), "id" | "disabled"))
        .map(generate_binding)
        .collect::<Result<_, _>>()?;

    let child_calls = generate_child_calls(&element.children)?;

    let has_scroll_style = element
        .classes
        .iter()
        .chain(element.conditional_classes.iter().map(|class| &class.class))
        .filter_map(|class| gpui_class(class))
        .any(|class| {
            matches!(
                class.rsplit(':').next(),
                Some(
                    "overflow-auto"
                        | "overflow-scroll"
                        | "overflow-x-auto"
                        | "overflow-x-scroll"
                        | "overflow-y-auto"
                        | "overflow-y-scroll"
                )
            )
        });

    let has_key_handlers = element
        .event_handlers
        .iter()
        .any(|handler| matches!(handler.event.as_str(), "keydown" | "keyup"));
    let keyboard_focus_call =
        has_key_handlers.then(|| quote! { .when(#enabled, |el| el.focusable().tab_index(0)) });
    let handler_calls = if disabled_binding.is_some() && !handler_calls.is_empty() {
        let builder = syn::Ident::new("__el", Span::mixed_site());
        quote! { .when(#enabled, |#builder| #builder #(#handler_calls)*) }
    } else {
        quote! { #(#handler_calls)* }
    };
    let disabled_style = disabled_binding.map(|_| {
        quote! {
            .when(#disabled, |el| el.cursor_not_allowed().opacity(0.5))
        }
    });

    // Interaction handlers, active styles, and scrolling require a stateful element.
    let id_call = if let Some(id) = &element.id {
        quote! { .id(#id) }
    } else if let Some(binding) = element.bindings.iter().find(|binding| binding.prop == "id") {
        generate_binding(binding)?
    } else if element
        .event_handlers
        .iter()
        .any(|h| matches!(h.event.as_str(), "click" | "hover"))
        || has_active_style
        || has_focus_style
        || has_key_handlers
        || has_scroll_style
    {
        let generated_id = ELEM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) as usize;
        let path = syn::Ident::new("__crepus_loop_path", Span::mixed_site());
        quote! { .id(::gpui::SharedString::from(format!("crepus-node-{}{}", #generated_id, #path))) }
    } else {
        quote! {}
    };

    let style_reset_helper = element
        .classes
        .iter()
        .chain(element.conditional_classes.iter().map(|class| &class.class))
        .filter_map(|class| gpui_class(class))
        .any(|class| {
            let utility = class.rsplit(':').next().unwrap_or(class);
            utility == "ring" || utility.starts_with("ring-") || utility == "shadow" || utility.starts_with("shadow-") || matches!(
                class.rsplit(':').next(),
                Some(
                    "self-auto"
                        | "aspect-auto"
                        | "overflow-visible"
                        | "overflow-x-visible"
                        | "overflow-y-visible"
                        | "overflow-auto"
                        | "overflow-scroll"
                        | "overflow-x-auto"
                        | "overflow-x-scroll"
                        | "overflow-y-auto"
                        | "overflow-y-scroll"
                )
            )
        })
        .then(|| {
            quote! {
                trait __CrepusStyleReset: ::gpui::Styled {
                    fn __crepus_shadow_class(self, class: &str) -> Self {
                        ::crepuscularity_gpui::crepuscularity_runtime::styler::apply_class_list(self, class)
                    }
                    fn __crepus_self_auto(mut self) -> Self {
                        self.style().align_self = None;
                        self
                    }
                    fn __crepus_overflow_visible(mut self, x: bool, y: bool) -> Self {
                        if x { self.style().overflow.x = Some(::gpui::Overflow::Visible); }
                        if y { self.style().overflow.y = Some(::gpui::Overflow::Visible); }
                        self
                    }
                    fn __crepus_overflow_scroll(mut self, x: bool, y: bool) -> Self {
                        if x { self.style().overflow.x = Some(::gpui::Overflow::Scroll); }
                        if y { self.style().overflow.y = Some(::gpui::Overflow::Scroll); }
                        self
                    }
                    fn __crepus_aspect_auto(mut self) -> Self {
                        self.style().aspect_ratio = None;
                        self
                    }
                }
                impl<T: ::gpui::Styled> __CrepusStyleReset for T {}
            }
        });

    Ok(quote! {{
        #style_reset_helper
        #disabled_setup
        #(#class_values)*
        #tag_expr
        #id_call
        #keyboard_focus_call
        #(#class_methods)*
        #(#conditional_class_calls)*
        #state_call
        #handler_calls
        #(#binding_calls)*
        #disabled_style
        #(#child_calls)*
    }})
}

fn generate_conditional_class(cc: &ConditionalClass) -> Result<TokenStream2, String> {
    let builder = syn::Ident::new("__el", Span::mixed_site());
    let condition: syn::Expr = syn::parse_str(&cc.condition)
        .map_err(|e| format!("Invalid condition in class:{}: {e}", cc.class))?;
    let Some(class) = gpui_class(&cc.class) else {
        return Ok(quote! {});
    };
    let style = map_class_to_style(class)
        .ok_or_else(|| format!("Unknown class `{}` in conditional class", cc.class))?;
    Ok(quote! {
        .when(#condition, |#builder| #builder #style)
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
    let builder = syn::Ident::new("__el", Span::mixed_site());
    let condition: syn::Expr = syn::parse_str(&if_block.condition)
        .map_err(|error| format!("Invalid if condition `{}`: {error}", if_block.condition))?;

    let then_calls = generate_child_calls(&if_block.then_children)?;

    match &if_block.else_children {
        Some(else_children)
            if if_block.then_children.len() == 1
                && else_children.len() == 1
                && !matches!(if_block.then_children[0], Node::LetDecl(_))
                && !matches!(else_children[0], Node::LetDecl(_)) =>
        {
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
                .map(|#builder| if #condition {
                    #builder #(#then_calls)*
                } else {
                    #builder #(#else_calls)*
                })
            })
        }
        None => Ok(quote! {
            .when(#condition, |#builder| #builder #(#then_calls)*)
        }),
    }
}

fn generate_if_expr(if_block: &IfBlock) -> Result<TokenStream2, String> {
    let builder = syn::Ident::new("__el", Span::mixed_site());
    let condition: syn::Expr = syn::parse_str(&if_block.condition)
        .map_err(|error| format!("Invalid if condition: {error}"))?;

    let then_calls = generate_child_calls(&if_block.then_children)?;

    match &if_block.else_children {
        Some(else_children)
            if if_block.then_children.len() == 1
                && else_children.len() == 1
                && !matches!(if_block.then_children[0], Node::LetDecl(_))
                && !matches!(else_children[0], Node::LetDecl(_)) =>
        {
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
                if #condition {
                    ::gpui::div() #(#then_calls)*
                } else {
                    ::gpui::div() #(#else_calls)*
                }
            })
        }
        None => Ok(quote! {
            ::gpui::div().when(#condition, |#builder| #builder #(#then_calls)*)
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
        1 if !matches!(for_block.body[0], Node::LetDecl(_)) => generate_node(&for_block.body[0])?,
        _ => {
            let child_calls = generate_child_calls(&for_block.body)?;
            quote! { ::gpui::div() #(#child_calls)* }
        }
    };

    let path = syn::Ident::new("__crepus_loop_path", Span::mixed_site());
    let index = syn::Ident::new("__crepus_loop_index", Span::mixed_site());
    // shortcut: positional IDs follow iteration order; use explicit IDs when items reorder.
    Ok(quote! {
        .children((#iterator).enumerate().map(|(#index, #pattern)| {
            let #path = format!("{}:{}", #path, #index);
            #body_expr
        }))
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
        1 if !matches!(arm.body[0], Node::LetDecl(_)) => {
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

fn event_callback_expression(source: &str) -> Result<Option<syn::Expr>, String> {
    let source = source.trim();
    if syn::parse_str::<syn::Ident>(source).is_ok() {
        return Ok(None);
    }
    syn::parse_str::<syn::Expr>(source)
        .map(Some)
        .map_err(|error| format!("Invalid event callback expression: {error}"))
}

fn generate_event_handler(handler: &EventHandler) -> Result<TokenStream2, String> {
    if !handler.modifiers.is_empty() && !matches!(handler.event.as_str(), "keydown" | "keyup") {
        return Err(format!(
            "GPUI does not support modifiers on `{}` events",
            handler.event
        ));
    }
    let handler_expr = &handler.handler;
    let closure = event_callback_expression(handler_expr)?;

    // Helper: parse closure or method reference
    let make_listener = || -> Result<TokenStream2, String> {
        if let Some(closure) = &closure {
            Ok(quote! { #closure })
        } else {
            let method: syn::Ident = syn::parse_str(handler_expr)
                .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
            Ok(quote! { cx.listener(Self::#method) })
        }
    };

    match handler.event.as_str() {
        "click" => {
            let listener = make_listener()?;
            Ok(quote! { .on_click(#listener) })
        }
        "input" | "change" => {
            let listener = make_listener()?;
            Ok(quote! { .on_input(#listener) })
        }
        "keydown" => {
            if handler.modifiers.is_empty() {
                let listener = make_listener()?;
                Ok(quote! { .on_key_down(#listener) })
            } else {
                generate_keydown_with_modifiers(handler)
            }
        }
        "keyup" => {
            if handler.modifiers.is_empty() {
                let listener = make_listener()?;
                Ok(quote! { .on_key_up(#listener) })
            } else {
                generate_keyup_with_modifiers(handler)
            }
        }
        "hover" => {
            let listener = make_listener()?;
            Ok(quote! { .on_hover(#listener) })
        }
        "mousedown" => {
            let listener = make_listener()?;
            Ok(quote! { .on_mouse_down(::gpui::MouseButton::Left, #listener) })
        }
        "mouseup" => {
            let listener = make_listener()?;
            Ok(quote! { .on_mouse_up(::gpui::MouseButton::Left, #listener) })
        }
        "mousemove" => {
            let listener = make_listener()?;
            Ok(quote! { .on_mouse_move(#listener) })
        }
        "scroll" => {
            let listener = make_listener()?;
            Ok(quote! { .on_scroll_wheel(#listener) })
        }
        _ => Err(format!("Unsupported GPUI event `{}`", handler.event)),
    }
}

fn key_conditions(handler: &EventHandler) -> Result<Vec<TokenStream2>, String> {
    handler
        .modifiers
        .iter()
        .filter(|modifier| modifier.as_str() != "capture")
        .map(|chord| {
            if matches!(
                chord.as_str(),
                "prevent" | "stop" | "immediate" | "trusted" | "self" | "once"
            ) {
                return Err(format!("GPUI does not support `{chord}` event modifiers"));
            }
            let lower = chord.to_lowercase();
            let parts: Vec<&str> = lower.split('+').collect();
            let key = match *parts.last().expect("split always has one part") {
                "" => return Err(format!("Key chord `{chord}` requires a key")),
                "esc" => "escape",
                "arrowup" => "up",
                "arrowdown" => "down",
                "arrowleft" => "left",
                "arrowright" => "right",
                key => key,
            };
            let mut conditions = vec![quote! { __ev.keystroke.key == #key }];
            for modifier in &parts[..parts.len() - 1] {
                conditions.push(match *modifier {
                    "ctrl" => quote! { __ev.keystroke.modifiers.control },
                    "cmd" => quote! { __ev.keystroke.modifiers.platform },
                    "shift" => quote! { __ev.keystroke.modifiers.shift },
                    "alt" | "opt" => quote! { __ev.keystroke.modifiers.alt },
                    _ => return Err(format!("Unknown key modifier `{modifier}` in `{chord}`")),
                });
            }
            Ok(quote! { (#(#conditions)&&*) })
        })
        .collect()
}

fn infer_key_callback_parameters(expression: &mut syn::Expr, event: &syn::Type) {
    // Stored closures need explicit reference types to accept every event lifetime.
    let infer_block = |block: &mut syn::Block| {
        if let Some(syn::Stmt::Expr(value, None)) = block.stmts.last_mut() {
            infer_key_callback_parameters(value, event);
        }
    };
    match expression {
        syn::Expr::Closure(closure) => {
            let types: [syn::Type; 3] = [
                syn::parse_quote!(&#event),
                syn::parse_quote!(&mut ::gpui::Window),
                syn::parse_quote!(&mut ::gpui::Context<_>),
            ];
            for (parameter, ty) in closure.inputs.iter_mut().zip(types) {
                match parameter {
                    syn::Pat::Type(pattern) if matches!(*pattern.ty, syn::Type::Infer(_)) => {
                        *pattern.ty = ty
                    }
                    syn::Pat::Type(_) => {}
                    _ => {
                        *parameter = syn::Pat::Type(syn::PatType {
                            attrs: Vec::new(),
                            pat: Box::new(parameter.clone()),
                            colon_token: Default::default(),
                            ty: Box::new(ty),
                        });
                    }
                }
            }
        }
        syn::Expr::Block(block) => infer_block(&mut block.block),
        syn::Expr::If(branch) => {
            infer_block(&mut branch.then_branch);
            if let Some((_, otherwise)) = &mut branch.else_branch {
                infer_key_callback_parameters(otherwise, event);
            }
        }
        syn::Expr::Match(branch) => {
            for arm in &mut branch.arms {
                infer_key_callback_parameters(&mut arm.body, event);
            }
        }
        // shortcut: infer Box::new wrappers; extend when other wrappers need callback inference.
        syn::Expr::Call(call)
            if matches!(call.func.as_ref(), syn::Expr::Path(path)
            if path.path.segments.iter().rev().take(2).map(|segment| segment.ident.to_string()).eq(["new", "Box"])) =>
        {
            if call.args.len() == 1 {
                infer_key_callback_parameters(call.args.first_mut().unwrap(), event);
            }
        }
        syn::Expr::Paren(paren) => infer_key_callback_parameters(&mut paren.expr, event),
        syn::Expr::Group(group) => infer_key_callback_parameters(&mut group.expr, event),
        _ => {}
    }
}

fn generate_keydown_with_modifiers(handler: &EventHandler) -> Result<TokenStream2, String> {
    let handler_expr = &handler.handler;
    let closure = event_callback_expression(handler_expr)?;

    let key_conditions = key_conditions(handler)?;
    let event_method = syn::Ident::new(
        if handler
            .modifiers
            .iter()
            .any(|modifier| modifier == "capture")
        {
            "capture_key_down"
        } else {
            "on_key_down"
        },
        Span::call_site(),
    );

    let listener = syn::Ident::new("__crepus_key_handler", Span::mixed_site());
    let (setup, call_body) = if let Some(mut closure) = closure {
        infer_key_callback_parameters(&mut closure, &syn::parse_quote!(::gpui::KeyDownEvent));
        (
            quote! { let #listener = #closure; },
            quote! { #listener(__ev, __window, __cx) },
        )
    } else {
        let method: syn::Ident = syn::parse_str(handler_expr)
            .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
        (
            quote! {},
            quote! { Self::#method(this, __ev, __window, __cx) },
        )
    };

    let condition = if key_conditions.is_empty() {
        quote! { true }
    } else {
        quote! { #(#key_conditions)||* }
    };

    Ok(quote! {
        .#event_method({
            #setup
            cx.listener(move |this, __ev: &::gpui::KeyDownEvent, __window, __cx| {
                if #condition {
                    #call_body
                }
            })
        })
    })
}

fn generate_keyup_with_modifiers(handler: &EventHandler) -> Result<TokenStream2, String> {
    let handler_expr = &handler.handler;
    let closure = event_callback_expression(handler_expr)?;

    let key_conditions = key_conditions(handler)?;
    let event_method = syn::Ident::new(
        if handler
            .modifiers
            .iter()
            .any(|modifier| modifier == "capture")
        {
            "capture_key_up"
        } else {
            "on_key_up"
        },
        Span::call_site(),
    );

    let listener = syn::Ident::new("__crepus_key_handler", Span::mixed_site());
    let (setup, call_body) = if let Some(mut closure) = closure {
        infer_key_callback_parameters(&mut closure, &syn::parse_quote!(::gpui::KeyUpEvent));
        (
            quote! { let #listener = #closure; },
            quote! { #listener(__ev, __window, __cx) },
        )
    } else {
        let method: syn::Ident = syn::parse_str(handler_expr)
            .map_err(|e| format!("Invalid method name `{handler_expr}`: {e}"))?;
        (
            quote! {},
            quote! { Self::#method(this, __ev, __window, __cx) },
        )
    };

    let condition = if key_conditions.is_empty() {
        quote! { true }
    } else {
        quote! { #(#key_conditions)||* }
    };

    Ok(quote! {
        .#event_method({
            #setup
            cx.listener(move |this, __ev: &::gpui::KeyUpEvent, __window, __cx| {
                if #condition {
                    #call_body
                }
            })
        })
    })
}

fn generate_binding(binding: &Binding) -> Result<TokenStream2, String> {
    let value: syn::Expr = syn::parse_str(&binding.value)
        .map_err(|e| format!("Invalid binding value `{}`: {e}", binding.value))?;

    match binding.prop.as_str() {
        "id" => Ok(quote! { .id(#value) }),
        "value" => Ok(quote! { .value(#value) }),
        "checked" => Ok(quote! { .checked(#value) }),
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

// ============================================================
// Tailwind → GPUI mapper
// ============================================================

fn gpui_class(mut class: &str) -> Option<&str> {
    while let Some((prefix, rest)) = class.split_once(':') {
        match prefix {
            "gpui" | "gui" => class = rest,
            "tui" | "web" | "ios" | "android" | "macos" | "darwin" | "windows" | "win32"
            | "linux" => return None,
            _ => break,
        }
    }
    Some(class)
}

// Static unknown utilities remain no-ops, but admitted color expressions must
// not disappear when Rust syntax parsing fails inside the optional style mapper.
fn validate_dynamic_color_class(class: &str) -> Result<(), String> {
    let Some(mut class) = gpui_class(class) else {
        return Ok(());
    };
    if let Some((state, rest)) = class.split_once(':') {
        if matches!(state, "hover" | "focus" | "active") {
            let Some(rest) = gpui_class(rest) else {
                return Ok(());
            };
            class = rest;
        }
    }
    let Some(rest) = ["bg-", "text-", "border-"]
        .iter()
        .find_map(|prefix| class.strip_prefix(prefix))
    else {
        return Ok(());
    };
    let color =
        if let Some((color, opacity)) = rest.rsplit_once('/').filter(|_| !rest.ends_with('}')) {
            if parse_opacity_percentage(opacity).is_none() {
                return Ok(());
            }
            color
                .strip_prefix('[')
                .and_then(|color| color.strip_suffix(']'))
                .unwrap_or(color)
        } else {
            rest
        };
    if let Some(expression) = extract_braced_expr_str(color) {
        syn::parse_str::<syn::Expr>(&expression)
            .map_err(|error| format!("Invalid color expression `{expression}`: {error}"))?;
    }
    Ok(())
}

fn map_class(class: &str) -> Option<TokenStream2> {
    let state_style = syn::Ident::new("__s", Span::mixed_site());
    let class = gpui_class(class)?;

    if let Some(rest) = class.strip_prefix("hover:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .hover(|#state_style| #state_style #style) });
    }
    if let Some(rest) = class.strip_prefix("focus:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .focus(|#state_style| #state_style #style) });
    }
    if let Some(rest) = class.strip_prefix("active:") {
        let style = map_class_to_style(rest)?;
        return Some(quote! { .active(|#state_style| #state_style #style) });
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
    let class = gpui_class(class)?;
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
        "flex-auto" => Some(quote! { .flex_auto() }),
        "flex-initial" => Some(quote! { .flex_initial() }),
        "flex-none" => Some(quote! { .flex_none() }),
        "flex-row-reverse" => Some(quote! { .flex_row_reverse() }),
        "flex-col-reverse" => Some(quote! { .flex_col_reverse() }),
        "flex-wrap-reverse" => Some(quote! { .flex_wrap_reverse() }),
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
        "aspect-square" => Some(quote! { .aspect_square() }),
        "aspect-video" => Some(quote! { .aspect_ratio(16.0 / 9.0) }),
        "aspect-auto" => Some(quote! { .__crepus_aspect_auto() }),
        "col-span-full" => Some(quote! { .col_span_full() }),
        "row-span-full" => Some(quote! { .row_span_full() }),
        "col-start-auto" => Some(quote! { .col_start_auto() }),
        "col-end-auto" => Some(quote! { .col_end_auto() }),
        "row-start-auto" => Some(quote! { .row_start_auto() }),
        "row-end-auto" => Some(quote! { .row_end_auto() }),
        "absolute" => Some(quote! { .absolute() }),
        "relative" => Some(quote! { .relative() }),
        "overflow-hidden" => Some(quote! { .overflow_hidden() }),
        "overflow-visible" => Some(quote! { .__crepus_overflow_visible(true, true) }),
        "overflow-x-visible" => Some(quote! { .__crepus_overflow_visible(true, false) }),
        "overflow-y-visible" => Some(quote! { .__crepus_overflow_visible(false, true) }),
        "overflow-auto" | "overflow-scroll" => {
            Some(quote! { .__crepus_overflow_scroll(true, true) })
        }
        "overflow-x-auto" | "overflow-x-scroll" => {
            Some(quote! { .__crepus_overflow_scroll(true, false) })
        }
        "overflow-y-auto" | "overflow-y-scroll" => {
            Some(quote! { .__crepus_overflow_scroll(false, true) })
        }
        "overflow-x-hidden" => Some(quote! { .overflow_x_hidden() }),
        "overflow-y-hidden" => Some(quote! { .overflow_y_hidden() }),

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
        "justify-evenly" => Some(quote! { .justify_evenly() }),
        "items-center" => Some(quote! { .items_center() }),
        "items-start" => Some(quote! { .items_start() }),
        "items-end" => Some(quote! { .items_end() }),
        "items-stretch" => Some(quote! { .items_stretch() }),
        "items-baseline" => Some(quote! { .items_baseline() }),
        "self-start" => Some(quote! { .self_start() }),
        "self-end" => Some(quote! { .self_end() }),
        "self-center" => Some(quote! { .self_center() }),
        "self-stretch" => Some(quote! { .self_stretch() }),
        "self-baseline" => Some(quote! { .self_baseline() }),
        "self-auto" => Some(quote! { .__crepus_self_auto() }),
        "content-center" => Some(quote! { .content_center() }),
        "content-start" => Some(quote! { .content_start() }),
        "content-end" => Some(quote! { .content_end() }),
        "content-normal" => Some(quote! { .content_normal() }),
        "content-stretch" => Some(quote! { .content_stretch() }),
        "content-between" => Some(quote! { .content_between() }),
        "content-around" => Some(quote! { .content_around() }),
        "content-evenly" => Some(quote! { .content_evenly() }),

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
        "border-t-0" => Some(quote! { .border_t_0() }),
        "border-t-2" => Some(quote! { .border_t_2() }),
        "border-t-4" => Some(quote! { .border_t_4() }),
        "border-t-8" => Some(quote! { .border_t_8() }),
        "border-b-0" => Some(quote! { .border_b_0() }),
        "border-b-2" => Some(quote! { .border_b_2() }),
        "border-b-4" => Some(quote! { .border_b_4() }),
        "border-b-8" => Some(quote! { .border_b_8() }),
        "border-l-0" => Some(quote! { .border_l_0() }),
        "border-l-2" => Some(quote! { .border_l_2() }),
        "border-l-4" => Some(quote! { .border_l_4() }),
        "border-l-8" => Some(quote! { .border_l_8() }),
        "border-r-0" => Some(quote! { .border_r_0() }),
        "border-r-2" => Some(quote! { .border_r_2() }),
        "border-r-4" => Some(quote! { .border_r_4() }),
        "border-r-8" => Some(quote! { .border_r_8() }),
        "border-dashed" => Some(quote! { .border_dashed() }),

        // Shadows
        "shadow" | "shadow-md" => Some(quote! { .__crepus_shadow_class("shadow-md") }),
        "shadow-2xs" => Some(quote! { .__crepus_shadow_class("shadow-2xs") }),
        "shadow-xs" => Some(quote! { .__crepus_shadow_class("shadow-xs") }),
        "shadow-sm" => Some(quote! { .__crepus_shadow_class("shadow-sm") }),
        "shadow-lg" => Some(quote! { .__crepus_shadow_class("shadow-lg") }),
        "shadow-xl" => Some(quote! { .__crepus_shadow_class("shadow-xl") }),
        "shadow-2xl" => Some(quote! { .__crepus_shadow_class("shadow-2xl") }),
        "shadow-none" => Some(quote! { .__crepus_shadow_class("shadow-none") }),
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
        "font-extralight" => Some(quote! { .font_weight(::gpui::FontWeight::EXTRA_LIGHT) }),
        "font-light" => Some(quote! { .font_weight(::gpui::FontWeight::LIGHT) }),
        "font-normal" => Some(quote! { .font_weight(::gpui::FontWeight::NORMAL) }),
        "font-medium" => Some(quote! { .font_weight(::gpui::FontWeight::MEDIUM) }),
        "font-semibold" => Some(quote! { .font_weight(::gpui::FontWeight::SEMIBOLD) }),
        "font-bold" => Some(quote! { .font_weight(::gpui::FontWeight::BOLD) }),
        "font-extrabold" => Some(quote! { .font_weight(::gpui::FontWeight::EXTRA_BOLD) }),
        "font-black" => Some(quote! { .font_weight(::gpui::FontWeight::BLACK) }),
        "italic" | "font-italic" => Some(quote! { .italic() }),
        "not-italic" => Some(quote! { .not_italic() }),

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

        // Typography — decoration
        "underline" => Some(quote! { .underline() }),
        "line-through" => Some(quote! { .line_through() }),
        "no-underline" => Some(quote! { .text_decoration_none() }),
        "decoration-solid" => Some(quote! { .text_decoration_solid() }),
        "decoration-wavy" => Some(quote! { .text_decoration_wavy() }),
        "decoration-0" => Some(quote! { .text_decoration_0() }),
        "decoration-1" => Some(quote! { .text_decoration_1() }),
        "decoration-2" => Some(quote! { .text_decoration_2() }),
        "decoration-4" => Some(quote! { .text_decoration_4() }),
        "decoration-8" => Some(quote! { .text_decoration_8() }),

        // Typography — line height
        "leading-none" => Some(quote! { .line_height(::gpui::relative(1.)) }),
        "leading-tight" => Some(quote! { .line_height(::gpui::relative(1.25)) }),
        "leading-snug" => Some(quote! { .line_height(::gpui::relative(1.375)) }),
        "leading-normal" => Some(quote! { .line_height(::gpui::relative(1.5)) }),
        "leading-relaxed" => Some(quote! { .line_height(::gpui::relative(1.625)) }),
        "leading-loose" => Some(quote! { .line_height(::gpui::relative(2.)) }),

        // Typography — misc
        "truncate" => Some(quote! { .truncate() }),
        "text-ellipsis" => Some(quote! { .text_ellipsis() }),
        "whitespace-nowrap" => Some(quote! { .whitespace_nowrap() }),
        "whitespace-normal" => Some(quote! { .whitespace_normal() }),
        "whitespace-pre" => None,

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
        "cursor-move" => Some(quote! { .cursor_move() }),
        "cursor-context-menu" => Some(quote! { .cursor_context_menu() }),
        "cursor-vertical-text" => Some(quote! { .cursor_vertical_text() }),
        "cursor-alias" => Some(quote! { .cursor_alias() }),
        "cursor-copy" => Some(quote! { .cursor_copy() }),
        "cursor-no-drop" => Some(quote! { .cursor_no_drop() }),
        "cursor-ew-resize" => Some(quote! { .cursor_ew_resize() }),
        "cursor-ns-resize" => Some(quote! { .cursor_ns_resize() }),
        "cursor-nesw-resize" => Some(quote! { .cursor_nesw_resize() }),
        "cursor-nwse-resize" => Some(quote! { .cursor_nwse_resize() }),
        "cursor-n-resize" => Some(quote! { .cursor_n_resize() }),
        "cursor-e-resize" => Some(quote! { .cursor_e_resize() }),
        "cursor-s-resize" => Some(quote! { .cursor_s_resize() }),
        "cursor-w-resize" => Some(quote! { .cursor_w_resize() }),

        // Explicit ignore list (not supported in GPUI)
        "fixed" | "sticky" | "select-none" | "pointer-events-none" => None,
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
    if class == "ring-0" {
        return Some(quote! { .__crepus_shadow_class("ring-0") });
    }
    let ring_width = match class {
        "ring" => Some(3.0f32),
        "ring-1" => Some(1.0),
        "ring-2" => Some(2.0),
        "ring-4" => Some(4.0),
        "ring-8" => Some(8.0),
        _ => class
            .strip_prefix("ring-[")
            .and_then(|rest| rest.strip_suffix(']'))
            .and_then(parse_arbitrary_length_token)
            .and_then(|length| match length {
                LengthToken::Px(value) => Some(value),
                LengthToken::Rems(value) => Some(value * 16.0),
                _ => None,
            })
            .filter(|width| width.is_finite()),
    };
    if ring_width.is_some() || class == "ring-inset" {
        return Some(quote! { .__crepus_shadow_class(#class) });
    }

    if let Some(rest) = class.strip_prefix("rounded-") {
        let (side, size) = rest.split_once('-').unwrap_or((rest, "md"));
        if matches!(side, "t" | "b" | "l" | "r" | "tl" | "tr" | "bl" | "br")
            && matches!(
                size,
                "none" | "sm" | "md" | "lg" | "xl" | "2xl" | "3xl" | "full"
            )
        {
            let method = syn::Ident::new(&format!("rounded_{side}_{size}"), Span::call_site());
            return Some(quote! { .#method() });
        }
    }

    if let Some(lines) = class
        .strip_prefix("line-clamp-")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|lines| *lines > 0)
    {
        return Some(quote! { .line_clamp(#lines) });
    }

    for (prefix, method) in [
        ("border-t-[", "border_t"),
        ("border-b-[", "border_b"),
        ("border-l-[", "border_l"),
        ("border-r-[", "border_r"),
        ("border-[", "border"),
        ("rounded-[", "rounded"),
        ("rounded-t-[", "rounded_t"),
        ("rounded-b-[", "rounded_b"),
        ("rounded-l-[", "rounded_l"),
        ("rounded-r-[", "rounded_r"),
        ("rounded-tl-[", "rounded_tl"),
        ("rounded-tr-[", "rounded_tr"),
        ("rounded-bl-[", "rounded_bl"),
        ("rounded-br-[", "rounded_br"),
        ("leading-[", "line_height"),
    ] {
        if let Some(inner) = class
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(']'))
        {
            if let Some(length) = parse_arbitrary_length_token(inner).filter(|token|
                matches!(token, LengthToken::Px(value) | LengthToken::Rems(value) if *value >= 0.0)) {
                let tokens = length_token_to_tokens(length);
                let method = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method(#tokens) });
            }
        }
    }

    if let Some(inner) = class
        .strip_prefix("aspect-[")
        .and_then(|rest| rest.strip_suffix(']'))
    {
        let ratio = if let Some((num, den)) = inner.split_once('/') {
            num.parse::<f32>()
                .ok()
                .zip(den.parse::<f32>().ok())
                .filter(|(w, h)| w.is_finite() && *w > 0.0 && h.is_finite() && *h > 0.0)
                .map(|(w, h)| w / h)
        } else {
            inner.parse::<f32>().ok()
        };
        if let Some(ratio) = ratio.filter(|ratio| ratio.is_finite() && *ratio > 0.0) {
            return Some(quote! { .aspect_ratio(#ratio) });
        }
        return None;
    }

    for (prefix, method) in [
        ("grid-cols-", "grid_cols"),
        ("grid-rows-", "grid_rows"),
        ("col-span-", "col_span"),
        ("row-span-", "row_span"),
    ] {
        if let Some(value) = class
            .strip_prefix(prefix)
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| *value > 0)
        {
            let method = syn::Ident::new(method, Span::call_site());
            return Some(quote! { .#method(#value) });
        }
    }
    for (prefix, method) in [
        ("col-start-", "col_start"),
        ("col-end-", "col_end"),
        ("row-start-", "row_start"),
        ("row-end-", "row_end"),
    ] {
        if let Some(value) = class
            .strip_prefix(prefix)
            .and_then(|value| value.parse::<i16>().ok())
            .filter(|value| *value != 0)
        {
            let method = syn::Ident::new(method, Span::call_site());
            return Some(quote! { .#method(#value) });
        }
    }
    // Spacing / sizing: w-N, h-N, p-N, m-N, gap-N, top-N, etc.
    const SPACING_PREFIXES: &[(&str, &str)] = &[
        ("w-", "w"),
        ("h-", "h"),
        ("size-", "size"),
        ("basis-", "flex_basis"),
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
            if let Some(length) = parse_length_token(rest) {
                let signed = matches!(
                    *method,
                    "m" | "mx"
                        | "my"
                        | "mt"
                        | "mb"
                        | "ml"
                        | "mr"
                        | "top"
                        | "right"
                        | "bottom"
                        | "left"
                        | "inset"
                );
                let negative = matches!(length, LengthToken::Px(value) | LengthToken::Rems(value) | LengthToken::Fraction(value) if value < 0.0);
                if negative && !signed {
                    return None;
                }
                if matches!(length, LengthToken::Auto)
                    && (method.starts_with('p') || method.starts_with("gap"))
                {
                    return None;
                }
                let tokens = length_token_to_tokens(length);
                let method_ident = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method_ident(#tokens) });
            }
        }
    }

    // text-[Npx] — arbitrary font size
    if let Some(rest) = class.strip_prefix("text-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(length) = parse_arbitrary_length_token(inner).filter(|token| matches!(token, LengthToken::Px(value) | LengthToken::Rems(value) if *value >= 0.0)) {
                let tokens = length_token_to_tokens(length);
                return Some(quote! { .text_size(#tokens) });
            }
        }
    }

    // tracking-[Npx] / tracking-[Nrem]
    if let Some(rest) = class.strip_prefix("tracking-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            let pixels = match parse_arbitrary_length_token(inner) {
                Some(LengthToken::Px(value)) => Some(value),
                Some(LengthToken::Rems(value)) => Some(value * 16.0),
                _ => None,
            };
            if let Some(pixels) = pixels.filter(|value| value.is_finite()) {
                return Some(quote! { .letter_spacing(::gpui::px(#pixels)) });
            }
        }
    }

    // font-['Family_Name'] or font-[Family_Name] — font family
    if let Some(rest) = class.strip_prefix("font-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            let family = inner.trim_matches('\'').trim_matches('"').replace('_', " ");
            if !family.trim().is_empty() {
                return Some(quote! { .font_family(#family) });
            }
        }
    }

    // opacity-N (0–100)
    if let Some(rest) = class.strip_prefix("opacity-") {
        if let Some(value) = parse_opacity_percentage(rest) {
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
            if let Some((color, opacity)) = rest.rsplit_once('/').filter(|_| !rest.ends_with('}')) {
                let color = color
                    .strip_prefix('[')
                    .and_then(|color| color.strip_suffix(']'))
                    .unwrap_or(color);
                let opacity = parse_opacity_percentage(opacity)?;
                if let Some(expression) = extract_braced_expr_str(color) {
                    let expression: syn::Expr = syn::parse_str(&expression).ok()?;
                    let alpha = (opacity * 255.0) as u32;
                    let method = syn::Ident::new(method, Span::call_site());
                    return Some(
                        quote! { .#method(::gpui::rgba(((#expression & 0x00ffffffu32) << 8) | #alpha)) },
                    );
                }
                let normalized = resolve_css_color(color)?;
                let rgb = u32::from_str_radix(&normalized[1..7], 16).ok()?;
                let alpha = if color.eq_ignore_ascii_case("transparent")
                    || color.eq_ignore_ascii_case("clear")
                {
                    0
                } else {
                    (opacity * 255.0) as u32
                };
                let rgba = (rgb << 8) | alpha;
                let method = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method(::gpui::rgba(#rgba)) });
            }

            // Dynamic color: bg-{theme.bg}, text-{theme.text}, border-{theme.border}
            if let Some(expr_str) = extract_braced_expr_str(rest) {
                let expr: syn::Expr = syn::parse_str(&expr_str)
                    .map_err(|error| format!("Invalid color expression `{expr_str}`: {error}"))
                    .ok()?;
                let method_ident = syn::Ident::new(method, Span::call_site());
                return Some(quote! { .#method_ident(::gpui::rgb(#expr)) });
            }

            let color = rest
                .strip_prefix('[')
                .and_then(|color| color.strip_suffix(']'))
                .unwrap_or(rest);
            if let Some(normalized) = resolve_css_color(color) {
                let hex = u32::from_str_radix(&normalized[1..], 16).ok()?;
                let method = syn::Ident::new(method, Span::call_site());
                return Some(if normalized.len() == 9 {
                    quote! { .#method(::gpui::rgba(#hex)) }
                } else {
                    quote! { .#method(::gpui::rgb(#hex)) }
                });
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

/// Parse-check a `.crepus` template at compile time and expand to its source `&str`.
///
/// Equivalent to `include_str!(…)` plus a build-time parse error if the template is invalid.
/// Relative paths resolve from the caller's manifest directory; absolute paths are retained.
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
    let full = LitStr::new(&full.to_string_lossy(), path.span());
    quote! {{
        include_str!(#full)
    }}
    .into()
}

/// Compile a `.crepus` file into a `&'static crepuscularity_embedded::compiled::CompiledTemplate`.
///
/// Parsing happens here on the build host, so firmware renders with `no_std` + `alloc`.
/// Constructs the compiled renderer cannot evaluate are compile errors.
/// Paths resolve like [`embedded_template!`].
#[proc_macro]
pub fn embedded_document(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let full = std::path::Path::new(&manifest_dir).join(path.value());
    let result = std::fs::read_to_string(&full)
        .map_err(|e| format!("embedded_document!: could not read {}: {e}", full.display()))
        .and_then(|source| embedded_document::expand(&source, &full));
    match result {
        Ok(tokens) => tokens.into(),
        Err(message) => syn::Error::new(path.span(), message)
            .to_compile_error()
            .into(),
    }
}

mod embedded_document;

#[cfg(test)]
mod bound_id_tests {
    use super::*;

    #[test]
    fn bound_id_is_emitted_once_before_click_handler() {
        let nodes = crepuscularity_core::parse_template(
            r#"<button id={button_id} @click={handle_click}>Press</button>"#,
        )
        .unwrap();
        let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
            panic!("expected button");
        };
        let code = generate_element(&element).unwrap().to_string();
        assert_eq!(code.matches(". id (").count(), 1, "{code}");
        assert!(
            code.find(". id (button_id)").unwrap() < code.find(". on_click").unwrap(),
            "{code}"
        );
    }
}

#[cfg(test)]
mod key_chord_tests {
    use super::*;

    #[test]
    fn malformed_chords_error_and_key_aliases_normalize() {
        for chord in [
            "ctl+s",
            "ctrl+",
            "ctrl++s",
            "+s",
            "",
            "prevent",
            "stop",
            "immediate",
            "trusted",
            "self",
            "once",
        ] {
            let handler = EventHandler {
                event: "keydown".into(),
                handler: "handle_key".into(),
                modifiers: vec![chord.into()],
            };
            assert!(
                generate_keydown_with_modifiers(&handler).is_err(),
                "{chord}"
            );
            assert!(generate_keyup_with_modifiers(&handler).is_err(), "{chord}");
        }
        let handler = EventHandler {
            event: "keyup".into(),
            handler: "handle_key".into(),
            modifiers: vec!["ctrl+Esc".into()],
        };
        let code = generate_keyup_with_modifiers(&handler).unwrap().to_string();
        assert!(code.contains("\"escape\""), "{code}");
    }

    #[test]
    fn keydown_and_keyup_use_the_same_chord_conditions() {
        for (chord, field) in [
            ("ctrl+s", "control"),
            ("cmd+s", "platform"),
            ("shift+tab", "shift"),
        ] {
            let handler = EventHandler {
                event: "keydown".into(),
                handler: "handle_key".into(),
                modifiers: vec![chord.into()],
            };
            for code in [
                generate_keydown_with_modifiers(&handler).unwrap(),
                generate_keyup_with_modifiers(&handler).unwrap(),
            ] {
                let code = code.to_string();
                assert!(code.contains(&format!("modifiers . {field}")), "{code}");
                assert!(!code.contains("modifiers . command"), "{code}");
                assert!(!code.contains(&format!("\"{chord}\"")), "{code}");
            }
        }
    }
}

#[cfg(test)]
mod event_validation_tests {
    use super::*;

    #[test]
    fn unsupported_events_and_non_key_modifiers_are_errors() {
        for (event, modifiers) in [
            ("unknown", vec![]),
            ("click", vec!["once".into()]),
            ("hover", vec!["ctrl".into()]),
        ] {
            let handler = EventHandler {
                event: event.into(),
                handler: "handle_event".into(),
                modifiers,
            };
            assert!(generate_event_handler(&handler).is_err(), "{event}");
        }
    }

    #[test]
    fn hover_gets_a_stateful_id_before_its_handler() {
        let nodes =
            crepuscularity_core::parse_template(r#"<div @hover={handle_hover}>Hover</div>"#)
                .unwrap();
        let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
            panic!("expected element");
        };
        let code = generate_element(&element).unwrap().to_string();
        assert_eq!(code.matches(". id (").count(), 1, "{code}");
        assert!(
            code.find(". id (").unwrap() < code.find(". on_hover").unwrap(),
            "{code}"
        );
    }
}

#[cfg(test)]
mod event_closure_tests {
    use super::*;

    #[test]
    fn jsx_and_indentation_closures_reach_event_generation() {
        for (source, method) in [
            (
                r#"<button @click={|_, _, _| {}}>Press</button>"#,
                "on_click",
            ),
            (r#"button @click={|_, _, _| {}}"#, "on_click"),
            (
                r#"<div @keyup|ctrl+s={move |_, _, _| {}}>Key</div>"#,
                "on_key_up",
            ),
        ] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
                panic!("expected element");
            };
            let code = generate_element(&element).unwrap().to_string();
            assert!(code.contains(method), "{code}");
        }
        assert!(event_callback_expression("handle_click").unwrap().is_none());
        assert!(event_callback_expression("{| invalid}").is_err());
        assert!(event_callback_expression("factory(").is_err());
    }
}

#[cfg(test)]
mod dynamic_class_list_tests {
    use super::*;

    #[test]
    fn dynamic_lists_generate_runtime_calls_and_validate_expressions() {
        for source in ["div class={theme}", "<div class={theme} />"] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
                panic!("expected element");
            };
            assert!(generate_element(&element)
                .unwrap()
                .to_string()
                .contains("apply_class_list"));
        }
        let nodes = crepuscularity_core::parse_template("div class={1 +}").unwrap();
        let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
            panic!("expected element");
        };
        assert!(generate_element(&element)
            .unwrap_err()
            .contains("Invalid class expression"));
    }
}

#[cfg(test)]
mod dynamic_color_diagnostic_tests {
    use super::*;

    #[test]
    fn malformed_dynamic_colors_report_errors_in_every_class_path() {
        let nodes = crepuscularity_core::parse_template("div").unwrap();
        for property in ["bg", "text", "border"] {
            for color in ["{1 +}", "{1 +}/50", "[{1 +}]/50"] {
                for state in ["", "hover:", "focus:gpui:", "gpui:active:gui:"] {
                    for conditional in [false, true] {
                        let class = format!("{state}{property}-{color}");
                        let Node::Element(mut element) = core_node_to_macro(&nodes[0]).unwrap()
                        else {
                            panic!("expected element")
                        };
                        if conditional {
                            element.conditional_classes.push(ConditionalClass {
                                class: class.clone(),
                                condition: "enabled".into(),
                            });
                        } else {
                            element.classes.push(class.clone());
                        }
                        let error = generate_element(&element)
                            .expect_err(&format!("{class}, conditional={conditional}"));
                        assert!(error.contains("Invalid color expression"), "{error}");
                    }
                }
            }
        }
    }

    #[test]
    fn valid_dynamic_colors_and_unknown_literals_keep_existing_behavior() {
        for class in ["bg-{color}", "text-{color}/50", "border-[{color}]/50"] {
            let source = format!("div class=\"{class}\"");
            let nodes = crepuscularity_core::parse_template(&source).unwrap();
            let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
                panic!("expected element")
            };
            assert!(generate_element(&element)
                .unwrap()
                .to_string()
                .contains("color"));
        }
        for class in ["unknown-utility", "bg-not-a-color", "web:bg-{1 +}"] {
            let nodes = crepuscularity_core::parse_template("div").unwrap();
            let Node::Element(mut element) = core_node_to_macro(&nodes[0]).unwrap() else {
                panic!("expected element")
            };
            element.classes.push(class.into());
            assert!(generate_element(&element).is_ok(), "{class}");
        }
    }
}

#[cfg(test)]
mod opacity_tests {
    use super::*;

    #[test]
    fn opacity_generation_clamps_and_skips_nonfinite_values() {
        assert!(map_class("opacity-150")
            .unwrap()
            .to_string()
            .contains("1f32"));
        for class in ["opacity-NaN", "opacity-inf", "opacity-1e100"] {
            assert!(map_class(class).is_none());
        }
    }
}

#[cfg(test)]
mod finite_length_tests {
    use super::*;

    #[test]
    fn nonfinite_lengths_do_not_generate_invalid_literals() {
        for class in [
            "w-inf",
            "p-NaN",
            "h-[infpx]",
            "gap-[NaNrem]",
            "w-3e38/1e-38",
        ] {
            assert!(map_class(class).is_none(), "{class}");
        }
        assert!(map_class("w-1/2").is_some());
        assert!(map_class("m-[-12px]").is_some());
    }
}

#[cfg(test)]
mod state_class_tests {
    use super::*;

    #[test]
    fn nested_state_targets_match_static_and_conditional_utilities() {
        let nodes = crepuscularity_core::parse_template("div id=fixture").unwrap();
        for state in ["hover", "focus", "active"] {
            for utility in ["ring-0", "shadow-none", "p-2"] {
                let plain = format!("{state}:{utility}");
                for prefix in ["gpui:", "gui:gpui:"] {
                    let nested = format!("gpui:{state}:{prefix}{utility}");
                    assert_eq!(
                        map_class(&nested).unwrap().to_string(),
                        map_class(&plain).unwrap().to_string()
                    );
                    for conditional in [false, true] {
                        let render = |class: String| {
                            let Node::Element(mut element) = core_node_to_macro(&nodes[0]).unwrap()
                            else {
                                panic!("expected element")
                            };
                            if conditional {
                                element.conditional_classes.push(ConditionalClass {
                                    class,
                                    condition: "enabled".into(),
                                });
                            } else {
                                element.classes.push(class);
                            }
                            generate_element(&element).unwrap().to_string()
                        };
                        assert_eq!(
                            render(nested.clone()),
                            render(plain.clone()),
                            "{nested}, conditional={conditional}"
                        );
                        let foreign = render(format!("{state}:web:{utility}"));
                        assert!(!foreign.contains(&format!(". {state} (")), "{foreign}");
                        assert!(!foreign.contains("__crepus_base_shadow"), "{foreign}");
                    }
                }
            }
        }
    }

    #[test]
    fn multiple_utilities_share_one_state_style_call() {
        let nodes = crepuscularity_core::parse_template(
            "div hover:bg-red-500 gpui:hover:text-white focus:p-2 focus:m-1 active:bg-blue-500 active:text-black web:hover:p-8"
        ).unwrap();
        let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
            panic!("expected element")
        };
        let code = generate_element(&element).unwrap().to_string();
        for state in ["hover", "focus", "active"] {
            assert_eq!(code.matches(&format!(". {state} (")).count(), 1, "{code}");
        }
        assert!(code.contains("text_color"));
    }
}

#[cfg(test)]
mod conditional_target_tests {
    use super::*;

    #[test]
    fn conditional_classes_use_the_same_target_filter_as_static_classes() {
        for class in ["gpui:flex-row", "gui:flex-row", "gpui:gui:flex-row"] {
            let tokens = generate_conditional_class(&ConditionalClass {
                class: class.into(),
                condition: "enabled".into(),
            })
            .unwrap()
            .to_string();
            assert!(tokens.contains("flex_row"), "{tokens}");
        }
        for class in ["web:flex-row", "tui:flex-row", "ios:flex-row"] {
            assert!(generate_conditional_class(&ConditionalClass {
                class: class.into(),
                condition: "enabled".into()
            })
            .unwrap()
            .is_empty());
        }
        assert!(generate_conditional_class(&ConditionalClass {
            class: "gpui:unknown-utility".into(),
            condition: "enabled".into()
        })
        .is_err());
    }
}

#[cfg(test)]
mod tracking_tests {
    use super::*;

    #[test]
    fn tracking_emits_finite_pixels_and_preserves_signed_spacing() {
        for (class, value) in [("tracking-[-0.5rem]", -8.0f32), ("tracking-[2px]", 2.0)] {
            assert_eq!(
                map_class_to_style(class).unwrap().to_string(),
                quote! { .letter_spacing(::gpui::px(#value)) }.to_string()
            );
        }
        for class in [
            "tracking-[50%]",
            "tracking-[auto]",
            "tracking-[1e38rem]",
            "tracking-[-1e38rem]",
            "tracking-[NaNpx]",
            "tracking-[infpx]",
        ] {
            assert!(map_class_to_style(class).is_none(), "{class}");
        }
    }
}

#[cfg(test)]
mod font_family_tests {
    use super::*;

    #[test]
    fn empty_font_families_are_ignored() {
        for class in ["font-[]", "font-['']", "font-[\"\"]", "font-[___]"] {
            assert!(map_class_to_style(class).is_none(), "{class}");
        }
        assert_eq!(
            map_class_to_style("font-['Times_New_Roman']")
                .unwrap()
                .to_string(),
            quote! { .font_family("Times New Roman") }.to_string()
        );
    }
}

#[cfg(test)]
mod braced_color_expression_tests {
    use super::*;
    #[test]
    fn extraction_requires_complete_outer_braces_and_preserves_rust_contents() {
        for expression in [
            r#"{"}".len() as u32}"#,
            r#"{"{".len() as u32}"#,
            "{{ 0x123456u32 }}",
        ] {
            let inner = extract_braced_expr_str(expression).unwrap();
            assert!(syn::parse_str::<syn::Expr>(&inner).is_ok(), "{inner}");
        }
        for expression in ["{color}junk", "{color}/50", "color}", "{color"] {
            assert!(
                extract_braced_expr_str(expression).is_none(),
                "{expression}"
            );
        }
    }
}

#[cfg(test)]
mod default_declaration_tests {
    use super::*;
    #[test]
    fn runtime_defaults_are_rejected_instead_of_overwriting_rust_bindings() {
        for source in ["$: default count = 1", "div\n  $: default count = 1"] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let error = core_node_to_macro(&nodes[0]).err().unwrap();
            assert!(error.contains("runtime template context"));
        }
        let nodes = crepuscularity_core::parse_template("$: let count = 1").unwrap();
        assert!(core_node_to_macro(&nodes[0]).is_ok());
    }
}

#[cfg(test)]
mod keyboard_focus_generation_tests {
    use super::*;
    #[test]
    fn indentation_key_handlers_generate_focus_setup() {
        let nodes = crepuscularity_core::parse_template(r#"div w-[100px] h-[40px] @keydown|ctrl+s={move |_, _, _| { hits.fetch_add(1, Ordering::SeqCst); }}"#).unwrap();
        let crepuscularity_core::ast::Node::Element(element) = &nodes[0] else {
            panic!("expected element: {nodes:?}")
        };
        assert_eq!(element.event_handlers.len(), 1, "{nodes:?}");
        let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
            panic!("expected element")
        };
        let code = generate_element(&element).unwrap().to_string();
        assert!(code.contains("focusable"), "{code}");
    }
}

#[cfg(test)]
mod form_binding_tests {
    use super::*;

    #[test]
    fn built_in_form_operations_report_the_missing_editable_component() {
        for attribute in [
            "bind:value={value}",
            "bind:checked={checked}",
            "@input=change",
            "@change=change",
        ] {
            for source in [
                format!("input {attribute}"),
                format!("<input {attribute} />"),
            ] {
                let nodes = crepuscularity_core::parse_template(&source).unwrap();
                let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
                    panic!("expected element")
                };
                let error = generate_element(&element).unwrap_err();
                assert!(
                    error.contains("requires a custom editable component"),
                    "{error}"
                );
            }
            let nodes =
                crepuscularity_core::parse_template(&format!("InputField {attribute}")).unwrap();
            let Node::Element(element) = core_node_to_macro(&nodes[0]).unwrap() else {
                panic!("expected element")
            };
            assert!(generate_element(&element).is_ok(), "{attribute}");
        }
    }
}

#[cfg(test)]
mod attribute_binding_tests {
    use super::*;

    #[test]
    fn html_attribute_origin_survives_conversion_and_is_rejected_before_gpui_controls() {
        for prop in ["id", "disabled", "value", "checked", "interval", "onclick"] {
            let mut nodes = crepuscularity_core::parse_template("input").unwrap();
            let crepuscularity_core::ast::Node::Element(element) = &mut nodes[0] else {
                panic!("expected element")
            };
            element.bindings.push(crepuscularity_core::ast::Binding {
                origin: crepuscularity_core::ast::BindingOrigin::Attribute,
                prop: prop.into(),
                value: "optional?.field".into(),
            });
            let converted = core_node_to_macro(&nodes[0]).unwrap();
            let Node::Element(element) = &converted else {
                panic!("expected macro element")
            };
            assert_eq!(
                element.bindings[0].origin,
                crepuscularity_core::ast::BindingOrigin::Attribute
            );
            let error = generate_node(&converted).unwrap_err();
            assert_eq!(
                error,
                format!("GPUI macros do not support HTML attribute binding `attr.{prop}`")
            );
        }
    }
}

#[cfg(all(test, unix))]
mod symlink_frontend_tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(name: &str) -> Fixture {
        let path = std::env::temp_dir().join(format!("crepus-refs-{name}-{}", std::process::id()));
        std::fs::create_dir_all(path.join("nested")).unwrap();
        std::os::unix::fs::symlink("nested/source.txt", path.join("view.vue")).unwrap();
        Fixture(path)
    }

    #[test]
    fn tui_refs_keep_the_real_authored_frontend_path_for_reload() {
        let directory = fixture("vue-path");
        let source = directory.0.join("nested/source.txt");
        let body = "<template><span id=\"root\" /></template>";
        std::fs::write(&source, body).unwrap();
        let authored = directory.0.join("view.vue");
        let tokens = build_tui_template_refs(&authored).unwrap();
        let authored = std::path::absolute(&authored).unwrap();
        let authored = authored.to_string_lossy();
        let constructor = quote! {
            ::crepuscularity_tui::Template::from_source_with_path(#body, #authored)
        };
        assert!(
            tokens.to_string().contains(&constructor.to_string()),
            "{tokens}"
        );
        syn::parse2::<syn::Expr>(tokens).unwrap();
    }

    #[test]
    fn reference_collection_parses_symlink_content_as_authored_vue() {
        let directory = fixture("vue-collection");
        let source =
            "<script>const ignored = 1 < 2;</script><template><span id=\"root\" /></template>";
        assert!(
            crepuscularity_core::parse_template_with_path(source, Some(Path::new("view.vue")))
                .is_ok()
        );
        assert!(crepuscularity_core::parse_template_with_path(
            source,
            Some(Path::new("source.txt"))
        )
        .is_err());
        std::fs::write(directory.0.join("nested/source.txt"), source).unwrap();
        let (ids, _) =
            collect_ids_from_path(&directory.0.join("view.vue"), "crepus_refs!").unwrap();
        assert_eq!(ids, ["root"]);
    }

    #[test]
    fn shared_targets_are_checked_for_each_authored_frontend_in_both_orders() {
        let directory = fixture("vue-jsx-aliases");
        let source =
            "<script>const ignored = 1 < 2;</script><template><span id=\"root\" /></template>";
        assert!(
            crepuscularity_core::parse_template_with_path(source, Some(Path::new("view.vue")))
                .is_ok()
        );
        assert!(
            crepuscularity_core::parse_template_with_path(source, Some(Path::new("view.jsx")))
                .is_err()
        );
        std::fs::write(directory.0.join("nested/source.txt"), source).unwrap();
        std::os::unix::fs::symlink("nested/source.txt", directory.0.join("view.jsx")).unwrap();
        let entry = directory.0.join("entry.crepus");
        for body in [
            "include view.vue\ninclude view.jsx",
            "include view.jsx\ninclude view.vue",
        ] {
            std::fs::write(&entry, body).unwrap();
            assert!(
                collect_ids_from_path(&entry, "crepus_refs!").is_err(),
                "{body}"
            );
        }
    }
}

#[cfg(test)]
mod binding_origin_tests {
    use super::*;
    #[test]
    fn attribute_binding_provenance_reaches_explicit_gpui_rejection() {
        let nodes = crepuscularity_core::parse_template_with_path(
            "<input [attr.checked]=\"false\"/>",
            Some(std::path::Path::new("View.ng")),
        )
        .unwrap();
        let node = core_node_to_macro(&nodes[0]).unwrap();
        let error = generate_node(&node).unwrap_err();
        assert!(error.contains("attr.checked"), "{error}");
    }
}
