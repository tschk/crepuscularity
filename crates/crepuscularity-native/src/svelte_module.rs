//! Compile authored Svelte components through the native JSX module compiler.
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{SourceType, Span};

use crate::{MoonshineModule, MoonshineModuleTarget};

#[derive(Default)]
struct SvelteNames(HashSet<String>, HashSet<String>, bool);
struct NamesInSource<'s> {
    names: &'s mut SvelteNames,
    source: &'s str,
}
impl NamesInSource<'_> {
    fn record(&mut self, name: &str, span: Span) {
        self.names.0.insert(name.to_owned());
        if &self.source[span.start as usize..span.end as usize] != name {
            self.names.1.insert(name.to_owned());
        }
    }
}
impl<'a> Visit<'a> for NamesInSource<'_> {
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        self.names.2 |= id.name == "globalThis";
        self.record(id.name.as_str(), id.span);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.record(id.name.as_str(), id.span);
    }
}
impl SvelteNames {
    fn collect(&mut self, source: &str, source_type: SourceType) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, source_type).parse();
        // Later transforms retain validation and diagnostic ownership.
        NamesInSource {
            names: self,
            source,
        }
        .visit_program(&parsed.program);
    }

    fn fresh(&self, source: &str, prefix: &str) -> String {
        (0usize..)
            .map(|index| format!("{prefix}{index}"))
            .find(|name| {
                !source.contains(name.as_str())
                    && !self
                        .0
                        .iter()
                        .any(|occupied| occupied.starts_with(name.as_str()))
            })
            .unwrap()
    }

    fn source_globals(&self, source: &str, typed: bool) -> crate::source_globals::SourceGlobals {
        if self.2 {
            crate::source_globals::SourceGlobals::with_aliases(
                &self.fresh(source, "__crepusSourceRealm"),
                &self.fresh(source, "__crepusSourceTypes"),
                typed,
            )
        } else {
            crate::source_globals::SourceGlobals::default()
        }
    }
}

pub(crate) fn companion_source_globals(source: &str) -> crate::source_globals::SourceGlobals {
    if !source.contains("globalThis") && !source.contains('\\') {
        return crate::source_globals::SourceGlobals::default();
    }
    let mut names = SvelteNames::default();
    names.collect(source, SourceType::ts());
    names.source_globals(source, false)
}

// The core allocator excludes every plain spelling present in authored source.
// Rename only generated bindings and their plain generated references, leaving
// authored escaped references at their original decoded outer binding.
fn rename_generated_inline(
    source: &str,
    previous: &str,
    replacement: &str,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid inline snippet hygiene syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits = Vec::new();
    let mut symbols = HashSet::new();
    for node in semantic.nodes().iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(id) = function.id.as_ref() else {
            continue;
        };
        if id.name == previous && &source[id.span.start as usize..id.span.end as usize] == previous
        {
            symbols.insert(
                id.symbol_id
                    .get()
                    .ok_or("unresolved generated snippet binding")?,
            );
            edits.push(id.span);
        }
    }
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        if id.name != previous || &source[id.span.start as usize..id.span.end as usize] != previous
        {
            continue;
        }
        let reference = semantic.scoping().get_reference(
            id.reference_id
                .get()
                .ok_or("unresolved generated snippet reference")?,
        );
        if reference
            .symbol_id()
            .is_some_and(|symbol| symbols.contains(&symbol))
        {
            edits.push(id.span);
        }
    }
    edits.sort_by_key(|span| (span.start, span.end));
    let mut output = source.to_owned();
    for span in edits.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, replacement);
    }
    Ok(output)
}

/// Compile an authored Svelte component without executing its scripts.
pub fn compile_moonshine_svelte(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<MoonshineModule, String> {
    let mut component = crepuscularity_core::parser::parse_svelte_module_with_styles_and_boundary(
        source,
        crate::svelte_boundary::boundary,
    )
    .map_err(|error| error.to_string())?;
    let pure_javascript = !component.script_is_typescript && !component.module_script_is_typescript;
    let policy = if pure_javascript {
        crate::svelte_javascript::IrPolicy::JavaScript
    } else {
        crate::svelte_javascript::IrPolicy::Typed
    };
    if pure_javascript {
        for script in [
            component.script.as_deref(),
            component.module_script.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            crate::svelte_javascript::validate(script, path)?;
        }
    }
    let style_source = component.style.clone();
    let mut styles = None;
    let snippet_names = component
        .snippets
        .iter()
        .map(|snippet| snippet.name.clone())
        .collect::<Vec<_>>();
    let mut snippet_declarations = component
        .snippets
        .iter()
        .map(|snippet| {
            format!(
                "function {}{}{{return {};}}",
                snippet.name, snippet.parameters, snippet.template_jsx
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if pure_javascript {
        let allocator = Allocator::default();
        for template in [&component.template_jsx, &snippet_declarations] {
            let parsed = Parser::new(&allocator, template, SourceType::jsx()).parse();
            if !parsed.errors.is_empty() {
                return Err("invalid authored JavaScript Svelte template expression".into());
            }
            crate::svelte_javascript::validate_docs(template, &parsed.program)?;
        }
    }
    let mut names = SvelteNames::default();
    for script in [
        component.script.as_deref(),
        component.module_script.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        names.collect(script, SourceType::ts());
    }
    names.collect(&component.template_jsx, SourceType::tsx());
    names.collect(&snippet_declarations, SourceType::tsx());
    if let Some(marker) = component.await_block_component.as_deref() {
        for template in [&component.template_jsx, &snippet_declarations] {
            names.collect(
                &crate::svelte_await::prepare_with_policy(template, marker, policy)?,
                SourceType::tsx(),
            );
        }
    }
    if let Some(marker) = component.each_block_component.as_deref() {
        for template in [&component.template_jsx, &snippet_declarations] {
            for scope in crate::svelte_each::name_sources(template, marker, policy)? {
                names.collect(&scope, SourceType::tsx());
            }
        }
    }
    let fresh = |prefix: &str| names.fresh(source, prefix);
    let source_globals = names.source_globals(source, !pure_javascript);
    for (index, previous) in component
        .inline_snippet_names
        .clone()
        .into_iter()
        .enumerate()
    {
        if !names.1.contains(&previous) {
            continue;
        }
        let replacement = fresh(&format!("__crepusInlineSnippetHygiene{index}_"));
        component.template_jsx =
            rename_generated_inline(&component.template_jsx, &previous, &replacement)?;
        snippet_declarations =
            rename_generated_inline(&snippet_declarations, &previous, &replacement)?;
        component.inline_snippet_names[index] = replacement;
    }
    for (slot, prefix) in [
        (
            &mut component.intrinsic_namespace_attribute,
            "__crepusIntrinsicNamespace",
        ),
        (
            &mut component.intrinsic_binding_attribute,
            "__crepusIntrinsicReference",
        ),
    ] {
        if let Some(previous) = slot.as_deref() {
            let marker = fresh(prefix);
            component.template_jsx = crate::svelte_intrinsics::rename_attribute(
                &component.template_jsx,
                previous,
                &marker,
            )?;
            snippet_declarations = crate::svelte_intrinsics::rename_attribute(
                &snippet_declarations,
                previous,
                &marker,
            )?;
            *slot = Some(marker);
        }
    }
    let name = fresh("__crepusSvelteComponent");
    let props = fresh("__crepusSvelteProps");
    let expose = fresh("__crepusSvelteExpose");
    let exposure = fresh("__crepusSvelteExposure");
    for (slot, prefix) in [
        (&mut component.each_block_component, "__CrepusEachBlock"),
        (&mut component.key_block_component, "__CrepusKeyBlock"),
    ] {
        if let Some(previous) = slot.as_deref() {
            if !names.1.contains(previous) {
                continue;
            }
            let marker = fresh(prefix);
            component.template_jsx =
                crate::svelte_await::rename_marker(&component.template_jsx, previous, &marker)?;
            snippet_declarations =
                crate::svelte_await::rename_marker(&snippet_declarations, previous, &marker)?;
            *slot = Some(marker);
        }
    }
    if let Some(previous) = component.await_block_component.as_deref() {
        let marker = fresh("__CrepusAwaitBlock");
        component.template_jsx =
            crate::svelte_await::rename_marker(&component.template_jsx, previous, &marker)?;
        snippet_declarations =
            crate::svelte_await::rename_marker(&snippet_declarations, previous, &marker)?;
        component.await_block_component = Some(marker);
    }

    let mut imports = source_globals.imports.clone();
    if let Some(key_component) = &component.key_block_component {
        if matches!(target, MoonshineModuleTarget::Dom) {
            let native_for = fresh("__crepusDirectiveNativeFor");
            imports.push_str(&format!("import {{For as {native_for}}} from \"@tschk/moonshine-dom/control\";\nfunction {key_component}<T,U>(props:{{each:()=>readonly T[];children:(item:T,index:()=>number)=>U}}){{return {native_for}(props,read=>read);}}\n"));
        } else {
            imports.push_str(&format!(
                "import {{For as {key_component}}} from \"@tschk/moonshine-dom/control\";\n"
            ));
        }
    }
    if let Some(previous) = component.const_scope_component.as_deref() {
        let marker = fresh("__CrepusConstScope");
        component.template_jsx =
            crate::svelte_await::rename_marker(&component.template_jsx, previous, &marker)?;
        snippet_declarations =
            crate::svelte_await::rename_marker(&snippet_declarations, previous, &marker)?;
        component.const_scope_component = Some(marker);
    }
    let (mut instance, instance_exports) = crate::svelte_exports::transform_with_source_type(
        &component.script.unwrap_or_default(),
        path,
        if pure_javascript {
            SourceType::mjs()
        } else {
            SourceType::ts()
        },
    )?;
    if matches!(target, MoonshineModuleTarget::Dom) {
        imports.push_str(&format!("import {{exposeComponent as {expose}}} from \"@tschk/moonshine-dom/component-instance\";\n"));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &instance, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "Svelte instance script parse failed: {:?}",
            parsed.errors
        ));
    }
    let mut removals = vec![];
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(declaration) => {
                let span = declaration.span;
                imports.push_str(&instance[span.start as usize..span.end as usize]);
                imports.push('\n');
                removals.push((span.start as usize, span.end as usize));
            }
            Statement::LabeledStatement(statement) if statement.label.name == "$" => {
                return Err("legacy Svelte reactive statements are unsupported; use runes".into());
            }
            _ => {}
        }
    }
    for (start, end) in removals.into_iter().rev() {
        instance.replace_range(start..end, "");
    }
    let has_module_script = component.module_script.is_some();
    let module_script = component.module_script.unwrap_or_default();
    let prefix = format!(
        "{module_script}\n{imports}\nexport default function {name}({props}: any = {{}}) {{\n"
    );
    let instance_start = prefix.len();
    let mut synthetic = format!(
        "{prefix}{instance}\n{snippet_declarations}\n{exposure};\nreturn <>{}</>;\n}}",
        component.template_jsx
    );
    let class_directive_helper = component
        .class_directive_helper
        .as_ref()
        .map(|_| fresh("__crepusClassDirectives"));
    if let (Some(marker), Some(helper)) = (
        component.class_directive_helper.as_deref(),
        class_directive_helper.as_deref(),
    ) {
        synthetic = crate::svelte_attributes::class_directives(&synthetic, marker, helper)?;
    }
    let mut scope = None;
    if let Some(css) = style_source.as_deref() {
        match crate::svelte_css::classify(css)? {
            crate::svelte_css::ClassifiedStyle::Global(global) => styles = Some(global),
            crate::svelte_css::ClassifiedStyle::Local(sheet) => {
                let marker = fresh("__crepusSvelteScope");
                // Versioned FNV-1a over authored bytes, independent of output target.
                let hash = format!(
                    "crepus-v1-{:016x}",
                    source
                        .bytes()
                        .fold(0xcbf29ce484222325u64, |hash, byte| (hash ^ u64::from(byte))
                            .wrapping_mul(0x100000001b3))
                );
                let (marked, css) = crate::svelte_css::scoped::prepare(
                    &synthetic,
                    css,
                    &sheet,
                    &marker,
                    &hash,
                    component.element_component.as_deref(),
                )?;
                synthetic = marked;
                styles = Some((!css.trim().is_empty()).then_some(css));
                scope = Some((marker, hash));
            }
        }
    }
    let dynamic_tag_attribute = component
        .element_component
        .as_ref()
        .map(|_| fresh("__crepusDynamicTag"));
    let dynamic_namespace_attribute = component
        .element_namespace_attribute
        .as_ref()
        .map(|_| fresh("__crepusDynamicNamespace"));
    let mut dynamic_map = None;
    if let (Some(marker), Some(selector), Some(tag_attribute)) = (
        component.element_component.as_deref(),
        component.element_tag_attribute.as_deref(),
        dynamic_tag_attribute.as_deref(),
    ) {
        let (lowered, map) = crate::svelte_element::transform_with_namespace(
            &synthetic,
            marker,
            selector,
            tag_attribute,
            component.element_namespace_attribute.as_deref(),
            dynamic_namespace_attribute.as_deref(),
            target,
            &source_globals.realm,
            &fresh,
        )?;
        synthetic = lowered;
        dynamic_map = map;
    }
    if let Some(marker) = component.intrinsic_namespace_attribute.as_deref() {
        let controls = component
            .each_block_component
            .iter()
            .chain(component.await_block_component.iter())
            .chain(component.key_block_component.iter())
            .chain(component.const_scope_component.iter())
            .map(String::as_str)
            .collect::<Vec<_>>();
        synthetic = crate::svelte_intrinsics::resolve_namespaces(&synthetic, marker, &controls)?;
    }
    if let Some(marker) = component.raw_html_component.as_deref() {
        let controls = component
            .each_block_component
            .iter()
            .chain(component.await_block_component.iter())
            .chain(component.key_block_component.iter())
            .chain(component.const_scope_component.iter())
            .map(String::as_str)
            .collect::<Vec<_>>();
        crate::svelte_html::validate_known_namespaces(&synthetic, path, marker, &controls, policy)?;
    }
    let const_runes_mode = if let Some(marker) = component.const_scope_component.as_deref() {
        crate::svelte_const::runes_mode(
            &synthetic,
            marker,
            component.script_is_typescript || component.module_script_is_typescript,
        )?
    } else {
        false
    };
    let mut typed_ranges = Vec::new();
    if component.module_script_is_typescript {
        typed_ranges.push(Span::new(0, module_script.len() as u32));
    }
    if component.script_is_typescript {
        typed_ranges.push(Span::new(
            instance_start as u32,
            (instance_start + instance.len()) as u32,
        ));
    }
    crate::svelte_class::validate_language(&synthetic, path, &typed_ranges, SourceType::tsx())?;
    let exported_props = instance_exports
        .iter()
        .filter_map(crate::svelte_exports::InstanceExport::writable_prop)
        .collect::<Vec<_>>();
    let lower_runes = |input: &str| {
        let (synthetic, store_completion) =
            crate::svelte_store::transform_with_policy(input, path, &name, target, policy)?;
        let store_finalizer = store_completion
            .as_ref()
            .map(|completion| completion.statement());
        let (synthetic, store_finished_in_props, public_props) =
            crate::svelte_props::transform_props_with_policy(
                &synthetic,
                path,
                target,
                (&name, &props),
                &exported_props,
                store_finalizer.as_deref(),
                (policy, &source_globals),
            )?;
        let synthetic =
            crate::svelte_effect::transform_effects_with_policy(&synthetic, path, target, policy)?;
        let transformed = crate::svelte_script::transform_with_policy(
            &synthetic,
            path,
            target,
            &name,
            &props,
            has_module_script,
            (policy, &source_globals),
        )?;
        Ok::<_, String>((
            transformed,
            store_completion,
            store_finished_in_props,
            public_props,
        ))
    };
    let directives = component
        .directives
        .iter()
        .map(|directive| crate::svelte_directives::Directive {
            attribute: directive.attribute().to_owned(),
            action_parameter: directive.action_parameter(),
        })
        .collect::<Vec<_>>();
    if matches!(target, MoonshineModuleTarget::Ssr)
        && (!directives.is_empty() || component.intrinsic_binding_attribute.is_some())
    {
        // Validate erased operands through the same rune pipeline without executing or emitting them.
        lower_runes(&synthetic)?;
        synthetic = crate::svelte_directives::erase(&synthetic, &directives)?;
    }
    let directive_group = if matches!(target, MoonshineModuleTarget::Dom) {
        let group = fresh("__CrepusDirectiveTemplate");
        let functions = std::iter::once(name.as_str())
            .chain(snippet_names.iter().map(String::as_str))
            .chain(component.inline_snippet_names.iter().map(String::as_str))
            .collect::<Vec<_>>();
        let controls = component
            .each_block_component
            .iter()
            .chain(component.key_block_component.iter())
            .chain(component.await_block_component.iter())
            .map(String::as_str)
            .collect::<Vec<_>>();
        synthetic = crate::svelte_directives::mark_groups(
            &synthetic,
            &group,
            &functions,
            &controls,
            component.const_scope_component.as_deref(),
            dynamic_map.as_deref(),
        )?;
        Some(group)
    } else {
        None
    };
    let directive_each = directive_group
        .as_ref()
        .and(component.each_block_component.as_ref())
        .map(|marker| (marker.clone(), fresh("__crepusDirectiveEachRetention")));
    let intrinsic_references = component.intrinsic_binding_attribute.as_ref().map(|raw| {
        crate::svelte_intrinsics::ReferenceMarkers {
            raw: raw.clone(),
            prepared: fresh("__crepusPreparedIntrinsicReference"),
        }
    });
    let binding_markers = [
        component.checked_binding_attribute.as_deref(),
        component.open_binding_attribute.as_deref(),
        component.files_binding_attribute.as_deref(),
        component.group_binding_attribute.as_deref(),
        component.value_binding_attribute.as_deref(),
        component.component_binding_attribute.as_deref(),
        component.intrinsic_binding_attribute.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(
        component
            .component_bindings
            .iter()
            .map(|(marker, _)| marker.as_str()),
    )
    .collect::<Vec<_>>();
    let synthetic = if let Some(marker) = component.await_block_component.as_deref() {
        crate::svelte_await::prepare_with_policy(&synthetic, marker, policy)?
    } else {
        synthetic
    };
    if let Some(marker) = component.const_scope_component.as_deref() {
        crate::svelte_const::validate_payloads(
            &synthetic,
            marker,
            component.each_block_component.as_deref(),
            policy,
        )?;
    }
    let text_markers = component
        .text_component
        .iter()
        .chain(component.title_component.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    let synthetic = {
        let mut prepared = synthetic;
        for marker in &text_markers {
            prepared =
                crate::svelte_text::prepare_call_readers(prepared, marker, target, policy, &fresh)?;
        }
        prepared
    };
    let synthetic = if let Some(marker) = component.each_block_component.as_deref() {
        if let Some((_, retention)) = &directive_each {
            crate::svelte_each::transform_retained_with_references(
                &synthetic,
                marker,
                target,
                &binding_markers,
                Some(retention),
                (policy, &source_globals),
                (intrinsic_references.as_ref(), &text_markers),
            )?
        } else {
            crate::svelte_each::transform_retained_with_references(
                &synthetic,
                marker,
                target,
                &binding_markers,
                None,
                (policy, &source_globals),
                (intrinsic_references.as_ref(), &text_markers),
            )?
        }
    } else {
        synthetic
    };
    let synthetic = if let Some(marker) = component.await_block_component.as_deref() {
        crate::svelte_await::finish_with_policy(
            &synthetic,
            marker,
            &binding_markers,
            directive_group.is_some(),
            policy,
        )?
    } else {
        synthetic
    };
    let helpers = component
        .key_block_component
        .iter()
        .chain(component.each_block_component.iter())
        .chain(component.await_block_component.iter())
        .chain(component.const_scope_component.iter())
        .chain(component.text_component.iter())
        .chain(component.raw_html_component.iter())
        .chain(component.render_component.iter())
        .chain(directive_group.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if let Some(marker) = component.const_scope_component.as_deref() {
        crate::svelte_const::validate_bindings(&synthetic, marker, &binding_markers)?;
    }
    let children_binding = component
        .component_bindings
        .iter()
        .find(|(_, prop)| prop == "children")
        .map(|(marker, _)| marker.as_str());
    let hoisted = [
        component.head_component.as_deref(),
        component.title_component.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let (synthetic, textarea_default_helper) = crate::svelte_text::transform_named(
        &synthetic,
        component.text_component.as_deref().unwrap_or(""),
        component.text_component.as_deref().unwrap_or(""),
        component.textarea_value_attribute.as_deref(),
        &hoisted,
        true,
        target,
    )?;
    let title_reader = fresh("__crepusHeadTitleText");
    let synthetic = if let Some(marker) = component.title_component.as_deref() {
        crate::svelte_text::transform_named(
            &synthetic,
            marker,
            &title_reader,
            None,
            &[],
            false,
            target,
        )?
        .0
    } else {
        synthetic
    };
    let synthetic = crate::svelte_head::transform(
        &synthetic,
        component.head_component.as_deref(),
        component.title_component.as_deref(),
        &title_reader,
        target,
        &fresh,
    )?;
    let synthetic = crate::svelte_children::transform(&synthetic, &helpers, children_binding)?;
    let synthetic = bridge_svelte_helpers(&synthetic, target)?;
    if (!snippet_names.is_empty() || !component.inline_snippet_names.is_empty())
        && !binding_markers.is_empty()
    {
        crate::svelte_snippets::validate_bindings_with_policy(
            &synthetic,
            path,
            &snippet_names,
            &component.inline_snippet_names,
            &binding_markers,
            policy,
        )?;
    }
    let (mut synthetic, capture_helper) = if !component.component_bindings.is_empty() {
        crate::svelte_component_values::prepare_with_policy(
            &synthetic,
            path,
            target,
            &component.component_bindings,
            &name,
            policy,
        )?
    } else {
        (synthetic, None)
    };
    if let Some(markers) = &intrinsic_references {
        synthetic =
            crate::svelte_intrinsics::prepare_references(&synthetic, markers, target, policy)?;
    }
    let component_reference = if let Some(marker) = component.component_binding_attribute {
        let (lowered, mapping) = crate::svelte_component_references::transform_with_policy(
            &synthetic, path, target, &marker, policy,
        )?;
        synthetic = lowered;
        mapping
    } else {
        None
    };
    let grouped_binding_markers = if directive_group.is_some() {
        component
            .checked_binding_attribute
            .iter()
            .chain(component.open_binding_attribute.iter())
            .chain(component.value_binding_attribute.iter())
            .chain(component.files_binding_attribute.iter())
            .chain(component.group_binding_attribute.iter())
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut binding_helpers = capture_helper.into_iter().collect::<Vec<_>>();
    let mut ssr_selection_helpers = Vec::new();

    if matches!(target, MoonshineModuleTarget::Ssr) {
        let (lowered, helper) = crate::svelte_bindings::transform_ssr_options(
            &synthetic,
            path,
            &source_globals,
            policy,
        )?;
        synthetic = lowered;
        ssr_selection_helpers.extend(helper.iter().cloned());
        binding_helpers.extend(helper);
    }
    for (marker, kind) in [
        (
            component.files_binding_attribute,
            crate::svelte_bindings::BindingKind::Files,
        ),
        (
            component.checked_binding_attribute,
            crate::svelte_bindings::BindingKind::Checked,
        ),
        (
            component.open_binding_attribute,
            crate::svelte_bindings::BindingKind::Open,
        ),
        (
            component.value_binding_attribute.clone(),
            crate::svelte_bindings::BindingKind::Select,
        ),
        (
            component.value_binding_attribute,
            crate::svelte_bindings::BindingKind::Value,
        ),
    ] {
        if let Some(marker) = marker {
            let (lowered, helper) = if directive_group.is_some() {
                crate::svelte_bindings::transform_template_with_policy(
                    &synthetic,
                    path,
                    target,
                    &marker,
                    kind,
                    true,
                    (policy, &source_globals),
                )?
            } else {
                crate::svelte_bindings::transform_template_with_policy(
                    &synthetic,
                    path,
                    target,
                    &marker,
                    kind,
                    false,
                    (policy, &source_globals),
                )?
            };
            synthetic = lowered;
            if matches!(target, MoonshineModuleTarget::Ssr)
                && kind == crate::svelte_bindings::BindingKind::Select
            {
                ssr_selection_helpers.extend(helper.iter().cloned());
            }
            binding_helpers.extend(helper);
        }
    }
    if matches!(target, MoonshineModuleTarget::Ssr) {
        if let Some(marker) = component.group_binding_attribute.as_deref() {
            let (lowered, helper) = crate::svelte_groups::ssr(
                &synthetic,
                marker,
                component.intrinsic_namespace_attribute.as_deref(),
            )?;
            synthetic = lowered;
            ssr_selection_helpers.extend(helper.iter().cloned());
            binding_helpers.extend(helper);
        }
    }

    let synthetic = if let Some(namespace) = component
        .intrinsic_namespace_attribute
        .as_deref()
        .filter(|_| !ssr_selection_helpers.is_empty())
    {
        crate::svelte_bindings::erase_factory_client_attributes(
            &synthetic,
            namespace,
            &ssr_selection_helpers,
        )?
    } else {
        synthetic
    };
    let synthetic = crate::svelte_snippets::transform_with_policy(
        &synthetic,
        path,
        target,
        &name,
        (&snippet_names, &component.inline_snippet_names),
        component.render_component.as_deref(),
        policy,
    )?;
    let synthetic = if let Some(marker) = component.const_scope_component.as_deref() {
        crate::svelte_const::lower_with_policy(
            &synthetic,
            marker,
            target,
            const_runes_mode,
            policy,
        )?
    } else {
        synthetic
    };
    let (synthetic, raw_property) = if let Some(marker) = &component.raw_html_component {
        crate::svelte_html::transform_with_policy(
            &synthetic,
            path,
            target,
            marker,
            policy,
            &source_globals,
        )?
    } else {
        (synthetic, None)
    };
    let synthetic = if let Some(marker) = &component.attribute_coercion_function {
        crate::svelte_attributes::transform(&synthetic, marker, target)?
    } else {
        synthetic
    };
    binding_helpers.extend(directive_group.iter().cloned());
    if directive_group.is_some() {
        binding_helpers.extend(component.each_block_component.iter().cloned());
        binding_helpers.extend(component.await_block_component.iter().cloned());
        binding_helpers.extend(component.key_block_component.iter().cloned());
    }

    let ssr_transparent_helpers = component
        .each_block_component
        .iter()
        .chain(component.await_block_component.iter())
        .chain(component.key_block_component.iter())
        .chain(component.render_component.iter())
        .chain(component.text_component.iter())
        .chain(ssr_selection_helpers.iter())
        .cloned()
        .collect::<Vec<_>>();

    binding_helpers.extend(component.const_scope_component.iter().cloned());
    binding_helpers.extend(component.text_component.iter().cloned());
    binding_helpers.extend(component.raw_html_component);
    binding_helpers.extend(component.render_component);
    let (synthetic, spread_metadata) = crate::svelte_spreads::prepare_target(&synthetic, target, component.intrinsic_namespace_attribute.as_deref())?;
    let (synthetic, prop_helper) = match target {
        MoonshineModuleTarget::Dom => {
            let mut static_helpers = binding_helpers.clone();
            static_helpers.extend(component.key_block_component.iter().cloned());
            crate::svelte_component_props::transform_with_policy(
                &synthetic,
                path,
                &static_helpers,
                component_reference
                    .as_ref()
                    .map(|(marker, _)| marker.as_str()),
                policy,
            )?
        }
        MoonshineModuleTarget::Ssr => (synthetic, None),
    };
    let synthetic = if let Some(marker) = component
        .group_binding_attribute
        .as_deref()
        .filter(|_| matches!(target, MoonshineModuleTarget::Dom))
    {
        crate::svelte_groups::allocate(&synthetic, marker, &source_globals)?
    } else {
        synthetic
    };
    let (transformed, store_completion, store_finished_in_props, public_props) =
        lower_runes(&synthetic)?;
    let descriptors = instance_exports
        .iter()
        .map(crate::svelte_exports::InstanceExport::descriptor)
        .collect::<Vec<_>>()
        .join(",");
    let transformed = transformed.replace(
        &format!("{exposure};"),
        &if matches!(target, MoonshineModuleTarget::Dom) {
            format!("{expose}({{{descriptors}}});")
        } else {
            String::new()
        },
    );
    let transformed = match target {
        MoonshineModuleTarget::Dom => crate::svelte_events::transform_events_with_policy(
            &transformed,
            path,
            &binding_helpers,
            policy,
            dynamic_tag_attribute.as_deref(),
            component.intrinsic_namespace_attribute.as_deref(),
            &source_globals,
        )?,
        MoonshineModuleTarget::Ssr => transformed,
    };
    let transformed = if let Some(marker) = component
        .group_binding_attribute
        .as_deref()
        .filter(|_| matches!(target, MoonshineModuleTarget::Dom))
    {
        crate::svelte_groups::install_events(&transformed, marker, &source_globals)?
    } else {
        transformed
    };
    let mut static_components = binding_helpers.clone();
    static_components.extend(component.each_block_component.iter().cloned());
    static_components.extend(component.key_block_component.iter().cloned());
    static_components.extend(component.await_block_component.iter().cloned());
    let transformed = crate::svelte_components::transform_with_policy(
        &transformed,
        path,
        &static_components,
        policy,
        target,
    )?;
    let transformed = match (target, component.key_block_component.as_deref()) {
        (MoonshineModuleTarget::Ssr, Some(marker)) => {
            erase_server_key_expressions(&transformed, marker)?
        }
        _ => transformed,
    };
    let property_keys = component_reference
        .as_ref()
        .map(|(marker, symbol)| (marker.as_str(), symbol.as_str()))
        .into_iter()
        .chain(
            raw_property
                .as_ref()
                .map(|(marker, expression)| (marker.as_str(), expression.as_str())),
        )
        .collect::<Vec<_>>();
    let lazy_prop_markers = spread_metadata
        .markers
        .iter()
        .chain(spread_metadata.element_markers.iter())
        .map(String::as_str)
        .chain(
            component
                .component_bindings
                .iter()
                .map(|(marker, _)| marker.as_str()),
        )
        .collect::<Vec<_>>();
    let mut compiled = crate::moonshine_module::compile_moonshine_svelte_module(
        &transformed,
        &path.with_extension("tsx"),
        target,
        crate::moonshine_module::ModuleHints {
            solid_list_helpers: &[],
            solid_component_imports: &[],
            solid_default_export: false,
            source_globals: Some(&source_globals),
            svelte_ir_policy: policy,
            ordered_svelte_fragments: matches!(target, MoonshineModuleTarget::Dom),
            intrinsic_namespace: component.intrinsic_namespace_attribute.as_deref(),
            dynamic_tag_attribute: dynamic_tag_attribute.as_deref(),
            dynamic_namespace_attribute: dynamic_namespace_attribute.as_deref(),
            dynamic_map: dynamic_map.as_deref(),
            class_directive_helper: class_directive_helper.as_deref(),
            intrinsic_reference: intrinsic_references
                .as_ref()
                .map(|markers| markers.prepared.as_str()),
            ssr_transparent_helpers: &ssr_transparent_helpers,
            const_scope: component.const_scope_component.as_deref(),
            prop_helper: prop_helper.as_deref(),
            directive_group: directive_group.as_deref(),
            directive_each: directive_each
                .as_ref()
                .map(|(marker, retention)| (marker.as_str(), retention.as_str())),
            directive_bindings: &grouped_binding_markers,
            directives: &directives,
            property_keys: &property_keys,
            text_component: component.text_component.as_deref(),
            textarea_default_helper: textarea_default_helper.as_deref(),
            inline_snippet_names: &component.inline_snippet_names,
            svelte_component_props: true,
            lazy_prop_markers: &lazy_prop_markers,
            element_spread_markers: &spread_metadata.element_markers,
            scoped_class: scope
                .as_ref()
                .map(|(marker, hash)| (marker.as_str(), hash.as_str())),
        },
    )?;
    let (source, spread_import) =
        crate::svelte_spreads::finish(&compiled.source, &spread_metadata, target, &source_globals)?;
    compiled.source = source;
    if let Some(specifier) = spread_import {
        if !compiled.imports.contains(&specifier) {
            compiled.imports.push(specifier);
        }
    }
    if !component.component_bindings.is_empty() {
        compiled.source = crate::svelte_component_values::finish(
            &compiled.source,
            &component.component_bindings,
        )?;
    }
    if let Some(completion) = store_completion.filter(|_| !store_finished_in_props) {
        compiled.source = completion.wrap(&compiled.source, &name)?;
    }
    if matches!(target, MoonshineModuleTarget::Dom) {
        compiled.source = crate::svelte_scheduling::wrap(&compiled.source, &name)?;
        let scheduler_import = "@tschk/moonshine-svelte/pre".to_owned();
        if !compiled.imports.contains(&scheduler_import) {
            compiled.imports.push(scheduler_import);
        }
    }
    let export_types = format!(
        "{{{}}}",
        instance_exports
            .iter()
            .map(crate::svelte_exports::InstanceExport::type_field)
            .collect::<Vec<_>>()
            .join(";")
    );
    let mount_import = match target {
        MoonshineModuleTarget::Dom => "@tschk/moonshine-svelte/mount",
        MoonshineModuleTarget::Ssr => "@tschk/moonshine-svelte/server-mount",
    };
    if pure_javascript {
        compiled.source = crate::svelte_javascript::finish(
            &compiled.source,
            path,
            &name,
            &public_props,
            &export_types,
            mount_import,
            fresh,
        )?;
        compiled.language = crate::moonshine_module::MoonshineModuleLanguage::JavaScript;
    } else {
        let (source, _) = crate::svelte_mount::finish(
            &compiled.source,
            &name,
            &public_props,
            &export_types,
            target,
            fresh,
            &source_globals,
        )?;
        compiled.source = crate::svelte_mount_types::transform(&source, path, fresh)?;
    }
    if !compiled.imports.iter().any(|import| import == mount_import) {
        compiled.imports.push(mount_import.to_owned());
    }
    if compiled
        .imports
        .iter()
        .any(|source| source == "svelte" || source.starts_with("svelte/"))
    {
        return Err(
            "unsupported Svelte runtime import; only supported named helpers are bridged".into(),
        );
    }
    compiled.styles = styles.flatten().into_iter().collect();
    Ok(compiled)
}

// Validate and lower authored expressions first; server keys never execute.
fn erase_server_key_expressions(source: &str, marker: &str) -> Result<String, String> {
    struct Keys<'s> {
        marker: &'s str,
        edits: Vec<oxc_span::Span>,
    }
    impl<'a> Visit<'a> for Keys<'_> {
        fn visit_jsx_element(&mut self, node: &JSXElement<'a>) {
            if matches!(&node.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == self.marker)
            {
                for attribute in &node.opening_element.attributes {
                    if let JSXAttributeItem::Attribute(attribute) = attribute {
                        if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "each")
                        {
                            if let Some(JSXAttributeValue::ExpressionContainer(value)) =
                                &attribute.value
                            {
                                self.edits.push(value.span);
                            }
                        }
                    }
                }
            }
            walk::walk_jsx_element(self, node);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "invalid generated Svelte key syntax: {:?}",
            parsed.errors
        ));
    }
    let mut keys = Keys {
        marker,
        edits: Vec::new(),
    };
    keys.visit_program(&parsed.program);
    keys.edits.sort_by_key(|span| span.start);
    let mut out = source.to_owned();
    for span in keys.edits.into_iter().rev() {
        out.replace_range(span.start as usize..span.end as usize, "{()=>[void 0]}");
    }
    Ok(out)
}

fn bridge_svelte_helpers(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("Svelte module parse failed: {:?}", parsed.errors));
    }
    let mut edits = vec![];
    for statement in &parsed.program.body {
        if let Statement::ImportDeclaration(import) = statement {
            if let Some(replacement) = rewrite_svelte_import(source, import, target)? {
                edits.push((import.span, replacement));
            }
        }
        if let Statement::ExportNamedDeclaration(export) = statement {
            if export.export_kind == ImportOrExportKind::Value
                && export.specifiers.iter().any(|specifier| {
                    specifier.export_kind == ImportOrExportKind::Value
                        && specifier.exported.name() == "default"
                })
            {
                return Err("Svelte module scripts cannot export default".into());
            }
            if let Some(replacement) = rewrite_svelte_export(source, export, target)? {
                edits.push((export.span, replacement));
            }
        }
        if matches!(statement, Statement::ExportDefaultDeclaration(_)) {
            // The generated component is the only default export.
            if let Statement::ExportDefaultDeclaration(export) = statement {
                if !matches!(&export.declaration, ExportDefaultDeclarationKind::FunctionDeclaration(function) if function.id.as_ref().is_some_and(|id| id.name.starts_with("__crepusSvelteComponent")))
                {
                    return Err("Svelte module scripts cannot export default".into());
                }
            }
        }
    }
    let mut result = source.to_owned();
    for (span, replacement) in edits.into_iter().rev() {
        result.replace_range(span.start as usize..span.end as usize, &replacement);
    }
    Ok(result)
}

fn svelte_helper_runtime(
    name: &str,
    target: MoonshineModuleTarget,
    type_only: bool,
) -> Result<&'static str, String> {
    match name {
        "Snippet" if type_only => Ok("@tschk/moonshine-svelte/svelte"),
        "tick" => match target {
            MoonshineModuleTarget::Dom => Ok("@tschk/moonshine-svelte/svelte"),
            MoonshineModuleTarget::Ssr => Ok("@tschk/moonshine-svelte/ssr"),
        },
        "mount" | "unmount" => match target {
            MoonshineModuleTarget::Dom => Ok("@tschk/moonshine-svelte/mount"),
            MoonshineModuleTarget::Ssr => Ok("@tschk/moonshine-svelte/server-mount"),
        },
        "onMount" | "onDestroy" | "getContext" | "setContext" | "hasContext" | "createContext"
        | "getAllContexts" => Ok("@tschk/moonshine-svelte/svelte"),
        "untrack" => match target {
            MoonshineModuleTarget::Dom => Ok("@tschk/moonshine-dom/reactive"),
            MoonshineModuleTarget::Ssr => Ok("@tschk/moonshine-dom/ssr"),
        },
        _ => Err(format!("native Svelte helper {name} is unsupported")),
    }
}

/// Rewrite the admitted Svelte named imports for every reachable native module.
pub(crate) fn rewrite_svelte_import(
    source: &str,
    import: &ImportDeclaration<'_>,
    target: MoonshineModuleTarget,
) -> Result<Option<String>, String> {
    if matches!(
        import.source.value.as_str(),
        "svelte/action" | "svelte/attachments"
    ) {
        let valid = import.specifiers.as_ref().is_some_and(|specifiers| {
            !specifiers.is_empty()
                && specifiers.iter().all(|specifier| {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                        return false;
                    };
                    (import.import_kind == ImportOrExportKind::Type
                        || specifier.import_kind == ImportOrExportKind::Type)
                        && match import.source.value.as_str() {
                            "svelte/action" => matches!(
                                specifier.imported.name().as_str(),
                                "Action" | "ActionReturn"
                            ),
                            _ => specifier.imported.name() == "Attachment",
                        }
                })
        });
        if !valid || import.with_clause.is_some() || import.phase.is_some() {
            return Err("unsupported Svelte directive import; only named Action/ActionReturn/Attachment type imports are supported".into());
        }
        let mut replacement =
            source[import.span.start as usize..import.span.end as usize].to_owned();
        replacement.replace_range(
            (import.source.span.start - import.span.start) as usize
                ..(import.source.span.end - import.span.start) as usize,
            "\"@tschk/moonshine-svelte/directives\"",
        );
        return Ok(Some(replacement));
    }
    let store_import = import.source.value == "svelte/store";
    if import.source.value.starts_with("svelte/") && !store_import {
        return Err("native Svelte subpath imports are unsupported".into());
    }
    if import.source.value != "svelte" && !store_import {
        return Ok(None);
    }
    let mut names = std::collections::BTreeMap::<&str, Vec<String>>::new();
    for specifier in import.specifiers.iter().flatten() {
        let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
            return Err("native Svelte imports require supported named helpers".into());
        };
        let imported = specifier.imported.name();
        let type_only = import.import_kind == ImportOrExportKind::Type
            || specifier.import_kind == ImportOrExportKind::Type;
        let runtime = if store_import {
            svelte_store_runtime(imported.as_str(), type_only)?
        } else {
            svelte_helper_runtime(imported.as_str(), target, type_only)?
        };
        names
            .entry(runtime)
            .or_default()
            .push(source[specifier.span.start as usize..specifier.span.end as usize].to_owned());
    }
    if names.is_empty() || import.with_clause.is_some() || import.phase.is_some() {
        return Err("native Svelte imports require plain supported named helpers".into());
    }
    let prefix = if import.import_kind == ImportOrExportKind::Type {
        "import type"
    } else {
        "import"
    };
    let replacement = names
        .into_iter()
        .map(|(runtime, names)| format!("{prefix} {{ {} }} from {runtime:?};", names.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(replacement))
}

fn svelte_store_runtime(imported: &str, type_only: bool) -> Result<&'static str, String> {
    let supported = matches!(
        imported,
        "writable" | "readable" | "readonly" | "derived" | "get"
    ) || (type_only
        && matches!(
            imported,
            "Subscriber"
                | "Invalidator"
                | "Unsubscriber"
                | "Updater"
                | "StartStopNotifier"
                | "Readable"
                | "Writable"
        ));
    if !supported {
        return Err(format!(
            "native Svelte store helper {imported} is unsupported"
        ));
    }
    Ok("@tschk/moonshine-svelte/store")
}

/// Rewrite admitted named helper reexports while retaining their source forms.
pub(crate) fn rewrite_svelte_export(
    source: &str,
    export: &ExportNamedDeclaration<'_>,
    target: MoonshineModuleTarget,
) -> Result<Option<String>, String> {
    let Some(module) = &export.source else {
        return Ok(None);
    };
    let store_export = module.value == "svelte/store";
    if module.value.starts_with("svelte/") && !store_export {
        return Err("native Svelte subpath reexports are unsupported".into());
    }
    if module.value != "svelte" && !store_export {
        return Ok(None);
    }
    if export.specifiers.is_empty() || export.with_clause.is_some() {
        return Err("native Svelte reexports require plain supported named helpers".into());
    }
    let mut names = std::collections::BTreeMap::<&str, Vec<String>>::new();
    for specifier in &export.specifiers {
        let imported = specifier.local.name();
        let type_only = export.export_kind == ImportOrExportKind::Type
            || specifier.export_kind == ImportOrExportKind::Type;
        let runtime = if store_export {
            svelte_store_runtime(imported.as_str(), type_only)?
        } else {
            svelte_helper_runtime(imported.as_str(), target, type_only)?
        };
        names
            .entry(runtime)
            .or_default()
            .push(source[specifier.span.start as usize..specifier.span.end as usize].to_owned());
    }
    let prefix = if export.export_kind == ImportOrExportKind::Type {
        "export type"
    } else {
        "export"
    };
    Ok(Some(
        names
            .into_iter()
            .map(|(runtime, names)| {
                format!("{prefix} {{ {} }} from {runtime:?};", names.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

/// Rewrite module source literals, leaving equal ordinary strings untouched.
pub fn rewrite_moonshine_imports(
    source: &str,
    path: &Path,
    replacements: &HashMap<String, String>,
) -> Result<String, String> {
    struct Rewrite<'s> {
        replacements: &'s HashMap<String, String>,
        edits: Vec<(oxc_span::Span, String)>,
    }
    impl Rewrite<'_> {
        fn literal(&mut self, source: &StringLiteral<'_>) {
            if let Some(replacement) = self.replacements.get(source.value.as_str()) {
                self.edits.push((source.span, format!("{replacement:?}")));
            }
        }
    }
    impl<'a> Visit<'a> for Rewrite<'_> {
        fn visit_import_declaration(&mut self, node: &ImportDeclaration<'a>) {
            self.literal(&node.source);
            walk::walk_import_declaration(self, node);
        }
        fn visit_export_named_declaration(&mut self, node: &ExportNamedDeclaration<'a>) {
            if let Some(source) = &node.source {
                self.literal(source);
            }
            walk::walk_export_named_declaration(self, node);
        }
        fn visit_export_all_declaration(&mut self, node: &ExportAllDeclaration<'a>) {
            self.literal(&node.source);
            walk::walk_export_all_declaration(self, node);
        }
        fn visit_import_expression(&mut self, node: &ImportExpression<'a>) {
            if let Expression::StringLiteral(source) = &node.source {
                self.literal(source);
            }
            walk::walk_import_expression(self, node);
        }
        fn visit_ts_import_type(&mut self, node: &TSImportType<'a>) {
            self.literal(&node.source);
            walk::walk_ts_import_type(self, node);
        }
    }
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).map_err(|error| error.to_string())?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "generated module parse failed: {:?}",
            parsed.errors
        ));
    }
    let mut rewrite = Rewrite {
        replacements,
        edits: vec![],
    };
    rewrite.visit_program(&parsed.program);
    rewrite.edits.sort_by_key(|(span, _)| span.start);
    let mut result = source.to_owned();
    for (span, replacement) in rewrite.edits.into_iter().rev() {
        result.replace_range(span.start as usize..span.end as usize, &replacement);
    }
    Ok(result)
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    use crate::compile_moonshine_module;
    #[test]
    fn mount_helpers_bridge_aliases_and_mixed_reexports_by_target() {
        for (target, runtime) in [
            (MoonshineModuleTarget::Dom, "@tschk/moonshine-svelte/mount"),
            (
                MoonshineModuleTarget::Ssr,
                "@tschk/moonshine-svelte/server-mount",
            ),
        ] {
            for source in [
                "import {mount as start,unmount as stop,onMount as ready} from 'svelte';",
                "export {mount as start,unmount as stop,onMount as ready} from 'svelte';",
            ] {
                let output = bridge_svelte_helpers(source, target).unwrap();
                assert!(output.contains(runtime), "{output}");
                assert!(
                    output.contains("mount as start, unmount as stop"),
                    "{output}"
                );
                assert!(output.contains("onMount as ready"), "{output}");
                assert!(
                    output.contains("@tschk/moonshine-svelte/svelte"),
                    "{output}"
                );
            }
            for name in ["hydrate", "ComponentView", "ComponentFactory"] {
                assert!(bridge_svelte_helpers(
                    &format!("import {{{name}}} from 'svelte';"),
                    target
                )
                .is_err());
            }
        }
    }
    #[test]
    fn snippet_types_bridge_only_effective_type_imports_and_reexports() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "import type {Snippet as Row} from 'svelte';",
                "import {type Snippet as Row} from 'svelte';",
                "export type {Snippet as Row} from 'svelte';",
                "export {type Snippet as Row} from 'svelte';",
                "import {type Snippet as Row,onMount as mount,untrack as read} from 'svelte';",
                "export {type Snippet as Row,onDestroy as cleanup,untrack as read} from 'svelte';",
            ] {
                let out = bridge_svelte_helpers(source, target).unwrap();
                assert!(out.contains("Snippet as Row"), "{out}");
                assert!(out.contains("@tschk/moonshine-svelte/svelte"), "{out}");
                if source.contains("untrack") {
                    let runtime = if matches!(target, MoonshineModuleTarget::Dom) {
                        "@tschk/moonshine-dom/reactive"
                    } else {
                        "@tschk/moonshine-dom/ssr"
                    };
                    assert!(out.contains(runtime), "{out}");
                    assert!(out.contains("untrack as read"));
                }
                let allocator = Allocator::default();
                assert!(
                    Parser::new(&allocator, &out, SourceType::ts())
                        .parse()
                        .errors
                        .is_empty(),
                    "{out}"
                );
            }
            for source in [
                "import {Snippet} from 'svelte';",
                "export {Snippet as Row} from 'svelte';",
                "import type {Unknown} from 'svelte';",
                "export type {Unknown} from 'svelte';",
                "import type {Snippet} from 'svelte/internal';",
                "export type {Snippet} from 'svelte/internal';",
                "import type Snippet from 'svelte';",
                "import type * as Svelte from 'svelte';",
            ] {
                assert!(
                    bridge_svelte_helpers(source, target).is_err(),
                    "accepted {source}"
                );
            }
        }
    }

    #[test]
    fn context_helpers_and_aliases_bridge_for_both_targets() {
        let source = "import {getContext as read,setContext as provide,hasContext as has,createContext as pair,onDestroy as cleanup,untrack as detached} from 'svelte';";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = bridge_svelte_helpers(source, target).unwrap();
            assert!(out.contains("getContext as read"));
            assert!(out.contains("setContext as provide"));
            assert!(out.contains("hasContext as has"));
            assert!(out.contains("createContext as pair"));
            assert!(out.contains("onDestroy as cleanup"));
            assert!(out.contains("@tschk/moonshine-svelte/svelte"));
            let reactive = match target {
                MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/reactive",
                MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
            };
            assert!(out.contains(&format!(
                "import {{ untrack as detached }} from {reactive:?};"
            )));
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        }
    }
    #[test]
    fn declaration_type_imports_preserve_type_only_declarations_when_split() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = r"import type {onMount as Mount,untrack as Track,getContext as Context} from 'svelte';";
            let out = bridge_svelte_helpers(source, target).unwrap();
            assert_eq!(out.matches("import type {").count(), 2);
            assert!(out.contains("onMount as Mount, getContext as Context"));
            assert!(out.contains("import type { untrack as Track }"));
            assert!(!out.contains("import { type"));
            let allocator = Allocator::default();
            assert!(Parser::new(&allocator, &out, SourceType::tsx())
                .parse()
                .errors
                .is_empty());
        }
    }
    #[test]
    fn mixed_specifier_types_and_escaped_aliases_survive_runtime_splitting() {
        let source = r"import {type onMount as Mount,getContext as \u0067et,type untrack as Track,onDestroy as cleanup} from 'svelte';";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = bridge_svelte_helpers(source, target).unwrap();
            assert!(out
                .contains(r"type onMount as Mount, getContext as \u0067et, onDestroy as cleanup"));
            assert!(out.contains("import { type untrack as Track }"));
            assert!(!out.contains("import type"));
            let allocator = Allocator::default();
            assert!(Parser::new(&allocator, &out, SourceType::tsx())
                .parse()
                .errors
                .is_empty());
        }
    }
    #[test]
    fn ordinary_typescript_modules_share_the_bridge_and_keep_source_metadata() {
        let source = "import {createContext as pair,type onMount as Mount,untrack as detached} from 'svelte';export const context=pair<string>();export const identity=<T>(value:T)=>value;";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_module(source, Path::new("context.ts"), target).unwrap();
            assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
            assert!(compiled.source.contains("type onMount as Mount"));
            assert!(compiled.source.contains("identity = <T>"));
            assert_eq!(compiled.imports, vec!["svelte"]);
            let native = match target {
                MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/reactive",
                MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
            };
            assert!(compiled.source.contains(native));
        }
        let untouched = "import {createContext} from './context';";
        assert_eq!(
            bridge_svelte_helpers(untouched, MoonshineModuleTarget::Dom).unwrap(),
            untouched
        );
    }
    #[test]
    fn ordinary_modules_reject_svelte_runtime_escape_routes() {
        for source in [
            "import 'svelte';",
            "import runtime from 'svelte';",
            "import * as runtime from 'svelte';",
            "import {getContext} from 'svelte/internal';",
            "export * from 'svelte';",
            "export const load=()=>import('svelte');",
            "export type Context=typeof import('svelte').getContext;",
            "export type Context=typeof import('svelte/internal').getContext;",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(
                    compile_moonshine_module(source, Path::new("context.ts"), target).is_err(),
                    "{source}"
                );
            }
        }
    }
    #[test]
    fn named_helper_reexports_preserve_aliases_types_and_target_splits() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = r"export {getContext as \u0072ead,createContext,type onMount as Mount,untrack as detached} from 'svelte';";
            let compiled =
                compile_moonshine_module(source, Path::new("barrel.ts"), target).unwrap();
            assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
            assert!(compiled.source.contains("type onMount as Mount"));
            assert_eq!(compiled.imports, vec!["svelte"]);
            let bridged = bridge_svelte_helpers(source, target).unwrap();
            assert!(bridged.contains(r"getContext as \u0072ead"));
            let native = match target {
                MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/reactive",
                MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
            };
            assert!(compiled.source.contains(native));
            let types = bridge_svelte_helpers(
                "export type {getContext as Context,untrack as Track} from 'svelte';",
                target,
            )
            .unwrap();
            assert_eq!(types.matches("export type {").count(), 2);
            assert!(!types.contains("export { type"));
            let allocator = Allocator::default();
            assert!(Parser::new(&allocator, &types, SourceType::tsx())
                .parse()
                .errors
                .is_empty());
        }
    }
    #[test]
    fn tick_import_alias_and_reexport_use_the_existing_native_helper() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let runtime = match target {
                MoonshineModuleTarget::Dom => "@tschk/moonshine-svelte/svelte",
                MoonshineModuleTarget::Ssr => "@tschk/moonshine-svelte/ssr",
            };
            for source in [
                "import {tick} from 'svelte';export const wait=()=>tick();",
                "import {tick as settle} from 'svelte';export const wait=()=>settle();",
                "export {tick,tick as settle} from 'svelte';",
            ] {
                let compiled =
                    compile_moonshine_module(source, Path::new("tick.ts"), target).unwrap();
                assert!(compiled.source.contains(runtime));
                assert!(!compiled.source.contains("from 'svelte'"));
            }
            let compiled = compile_moonshine_svelte(
                "<script>import {tick as settle} from 'svelte';const wait=()=>settle();</script><button onclick={wait}>wait</button>",
                Path::new("Tick.svelte"),target,
            ).unwrap();
            assert!(compiled.source.contains(runtime));
        }
    }
    #[test]
    fn ordinary_default_helper_reexports_do_not_change_the_sfc_default_contract() {
        let source = "export {getContext as default} from 'svelte';";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_module(source, Path::new("barrel.ts"), target).unwrap();
            assert!(compiled.has_default_export);
            assert!(bridge_svelte_helpers(source, target).is_err());
            assert!(bridge_svelte_helpers("export {local as default};", target).is_err());
            assert!(compile_moonshine_module(
                "export {flushSync} from 'svelte';",
                Path::new("barrel.ts"),
                target
            )
            .is_err());
            assert!(compile_moonshine_module(
                "export {getContext} from 'svelte/internal';",
                Path::new("barrel.ts"),
                target
            )
            .is_err());
            assert!(compile_moonshine_module(
                "export * as context from 'svelte';",
                Path::new("barrel.ts"),
                target
            )
            .is_err());
        }
    }
    #[test]
    fn context_bridge_does_not_admit_other_runtime_forms() {
        for source in [
            "import {flushSync} from 'svelte';",
            "import helpers from 'svelte';",
            "import * as helpers from 'svelte';",
            "import {getContext} from 'svelte/internal';",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(bridge_svelte_helpers(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn scoped_css_tracks_dynamic_element_ownership() {
        let source = r#"<script>let tag='div';</script><svelte:element this={tag} class="inner"/><style>.inner{color:red}</style>"#;
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let module =
                compile_moonshine_svelte(source, Path::new("dynamic.svelte"), target).unwrap();
            assert!(!module.styles.is_empty());
            assert!(module.source.contains("crepus-v1-"), "{}", module.source);
            assert!(module
                .styles
                .iter()
                .any(|css| css.contains(".inner.crepus-v1-")));
        }
    }
}

#[cfg(test)]
mod store_import_tests {
    use super::*;
    fn rewrite(source: &str, target: MoonshineModuleTarget) -> Result<Option<String>, String> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty());
        let Statement::ImportDeclaration(import) = &parsed.program.body[0] else {
            panic!("expected import")
        };
        rewrite_svelte_import(source, import, target)
    }
    #[test]
    fn native_store_named_aliases_and_type_imports_keep_their_contract() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = rewrite(
                "import {writable as make, readonly as expose, type Readable as Source} from 'svelte/store';",
                target,
            )
            .unwrap()
            .unwrap();
            assert!(output.contains("@tschk/moonshine-svelte/store"));
            assert!(output.contains("writable as make"));
            assert!(output.contains("readonly as expose"));
            assert!(output.contains("type Readable as Source"));
            let output = rewrite("import type {Writable} from 'svelte/store';", target)
                .unwrap()
                .unwrap();
            assert!(output.starts_with("import type"));
        }
    }
    #[test]
    fn store_reexports_preserve_aliases_and_effective_types() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "export { writable as make, readonly as expose, type Readable as Source } from 'svelte/store';",
                "export type { Writable as Sink } from 'svelte/store';",
            ] {
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
                assert!(parsed.errors.is_empty());
                let Statement::ExportNamedDeclaration(export) = &parsed.program.body[0] else {
                    panic!("expected export")
                };
                let output = rewrite_svelte_export(source, export, target).unwrap().unwrap();
                assert_eq!(output, source.replace("'svelte/store'", "\"@tschk/moonshine-svelte/store\""));
            }
            for source in [
                "export {fromStore} from 'svelte/store';",
                "export type {Unknown} from 'svelte/store';",
            ] {
                assert!(bridge_svelte_helpers(source, target).is_err());
            }
        }
    }
    #[test]
    fn unavailable_store_exports_and_non_named_imports_fail_explicitly() {
        for source in [
            "import {fromStore} from 'svelte/store';",
            "import * as stores from 'svelte/store';",
            "import store from 'svelte/store';",
            "import type {Unknown} from 'svelte/store';",
        ] {
            assert!(
                rewrite(source, MoonshineModuleTarget::Dom).is_err(),
                "{source}"
            );
        }
    }
}
