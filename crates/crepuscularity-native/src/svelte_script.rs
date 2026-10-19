//! Scope-aware lowering of Svelte instance and companion factory runes.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_transformer::{TransformOptions, Transformer};
use std::{collections::HashSet, path::Path};

pub(crate) fn is_rune(name: &str) -> bool {
    matches!(
        name,
        "$state" | "$derived" | "$props" | "$bindable" | "$effect" | "$inspect" | "$host"
    )
}

/// Ambient rune declarations are erased and do not create runtime shadowing.
pub(crate) fn has_runtime_binding(
    semantic: &oxc_semantic::Semantic<'_>,
    id: &IdentifierReference<'_>,
) -> bool {
    let Some(symbol) = semantic
        .scoping()
        .get_reference(id.reference_id.get().unwrap())
        .symbol_id()
    else {
        return false;
    };
    if !is_rune(id.name.as_str()) {
        return true;
    }
    let runtime = |flags: oxc_syntax::symbol::SymbolFlags| {
        (flags.is_value() || flags.is_import()) && !flags.is_ambient() && !flags.is_type_import()
    };
    let declarations = semantic.scoping().symbol_redeclarations(symbol);
    if declarations.is_empty() {
        runtime(semantic.scoping().symbol_flags(symbol))
    } else {
        declarations
            .iter()
            .any(|declaration| runtime(declaration.flags))
    }
}

#[derive(Clone, Copy)]
enum RuneContext<'a> {
    Instance(&'a str, &'a str),
    ComponentModule(&'a str, &'a str),
    Companion,
}
impl<'a> RuneContext<'a> {
    fn instance(self) -> Option<(&'a str, &'a str)> {
        match self {
            Self::Instance(name, props) | Self::ComponentModule(name, props) => Some((name, props)),
            Self::Companion => None,
        }
    }
}
#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    component_name: &str,
    props_name: &str,
) -> Result<String, String> {
    transform_context(
        source,
        path,
        target,
        RuneContext::Instance(component_name, props_name),
        crate::svelte_javascript::IrPolicy::Typed,
        &Default::default(),
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    component_name: &str,
    props_name: &str,
    module: bool,
    (policy, globals): (
        crate::svelte_javascript::IrPolicy,
        &crate::source_globals::SourceGlobals,
    ),
) -> Result<String, String> {
    let context = if module {
        RuneContext::ComponentModule(component_name, props_name)
    } else {
        RuneContext::Instance(component_name, props_name)
    };
    transform_context(source, path, target, context, policy, globals)
}

pub(crate) fn transform_companion(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ts"))
    {
        crate::svelte_class::validate_language(
            source,
            path,
            &[oxc_span::Span::new(0, source.len() as u32)],
            SourceType::ts(),
        )?;
    }
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("js"))
    {
        // Validate once at the external companion entry; recursive lowering consumes trusted IR.
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        if !parsed.errors.is_empty() {
            return Err(format!(
                "{}: invalid authored companion JavaScript",
                path.display()
            ));
        }
        let checked = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !checked.errors.is_empty() {
            return Err(format!(
                "{}: invalid authored companion JavaScript scope",
                path.display()
            ));
        }
    }
    let globals = crate::svelte_module::companion_source_globals(source);
    transform_context(
        source,
        path,
        target,
        RuneContext::Companion,
        crate::svelte_javascript::IrPolicy::Typed,
        &globals,
    )
}

fn transform_context(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    context: RuneContext<'_>,
    policy: crate::svelte_javascript::IrPolicy,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    let instance = context.instance();
    let allocator = Allocator::default();
    let source_type = if instance.is_some() {
        SourceType::tsx()
    } else {
        SourceType::ts()
    };
    let mut parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte instance script: {:?}",
            path.display(),
            parsed.errors
        ));
    }
    let javascript_companion = matches!(context, RuneContext::Companion)
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("js"));
    // Original JS was parsed and scope-checked at the external companion entry.
    // Generated helpers need TS grammar, but public class duplicates retain JS semantics.
    if javascript_companion {
        parsed.program.source_type = SourceType::mjs();
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte instance scope: {:?}",
            path.display(),
            built.errors
        ));
    }
    let semantic = built.semantic;
    if let Some(plan) = crate::svelte_class::plan(source, path, &semantic)? {
        // The class pass mutates AST nodes; rebuild semantic identities before
        // ordinary rune lowering rather than reusing the original node table.
        drop(semantic);
        parsed.program.source_type = source_type;
        let lowered = crate::svelte_class::lower(
            source,
            path,
            target,
            &allocator,
            &mut parsed.program,
            plan,
        )?;
        return transform_context(&lowered, path, target, context, policy, globals);
    }
    let nodes = semantic.nodes();
    let mut exported = HashSet::new();
    if matches!(
        context,
        RuneContext::ComponentModule(..) | RuneContext::Companion
    ) {
        for statement in &parsed.program.body {
            if let Statement::ExportDefaultDeclaration(export) = statement {
                if let Some(Expression::Identifier(id)) = export
                    .declaration
                    .as_expression()
                    .map(Expression::get_inner_expression)
                {
                    let reference = semantic.scoping().get_reference(
                        id.reference_id
                            .get()
                            .ok_or("unresolved default export reference")?,
                    );
                    if let Some(symbol) = reference.symbol_id() {
                        exported.insert(symbol);
                    }
                }
                continue;
            }
            let Statement::ExportNamedDeclaration(export) = statement else {
                continue;
            };
            if export.source.is_some() || export.export_kind == ImportOrExportKind::Type {
                continue;
            }
            if let Some(Declaration::VariableDeclaration(declaration)) = &export.declaration {
                for declaration in &declaration.declarations {
                    for id in declaration.id.get_binding_identifiers() {
                        exported.insert(id.symbol_id.get().ok_or("unresolved module export")?);
                    }
                }
            }
            for specifier in &export.specifiers {
                if specifier.export_kind == ImportOrExportKind::Type {
                    continue;
                }
                if let ModuleExportName::IdentifierReference(id) = &specifier.local {
                    let reference = semantic
                        .scoping()
                        .get_reference(id.reference_id.get().ok_or("unresolved export reference")?);
                    if let Some(symbol) = reference.symbol_id() {
                        exported.insert(symbol);
                    }
                }
            }
        }
    }
    let mut names = HashSet::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                names.insert(id.name.as_str());
            }
            AstKind::IdentifierReference(id) => {
                names.insert(id.name.as_str());
            }
            _ => {}
        }
    }
    let unique = |base: &str| {
        let mut name = base.to_owned();
        while names.contains(name.as_str()) || source.contains(&name) {
            name.push('_');
        }
        name
    };
    // Expand patterns before scalar cell lowering so defaults/references receive the
    // same semantic-symbol rewrite as authored scalar declarations (no overlapping edits).
    let mut pattern_edits = Vec::new();
    let mut reserved: HashSet<String> = names.iter().map(|name| (*name).to_owned()).collect();
    let mut fresh = |base: &str| {
        let mut name = base.to_owned();
        while reserved.contains(&name) || source.contains(&name) {
            name.push('_');
        }
        reserved.insert(name.clone());
        name
    };
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(decl) = node.kind() else {
            continue;
        };
        if matches!(decl.id, BindingPattern::BindingIdentifier(_)) {
            continue;
        }
        let Some(Expression::CallExpression(call)) = &decl.init else {
            continue;
        };
        let callee = match &call.callee {
            Expression::Identifier(id) if id.name == "$derived" => id,
            Expression::StaticMemberExpression(member) if member.property.name == "by" => {
                let Expression::Identifier(id) = &member.object else {
                    continue;
                };
                if id.name != "$derived" {
                    continue;
                }
                id
            }
            _ => continue,
        };
        if has_runtime_binding(&semantic, callee) {
            continue;
        }
        let fail = |message: &str| format!("{}: {message}", path.display());
        let shared = fresh("__crepusDerivedPattern");
        let annotation = decl
            .type_annotation
            .as_ref()
            .map(|annotation| &source[annotation.span.start as usize..annotation.span.end as usize])
            .unwrap_or("");
        let initializer = &source[call.span.start as usize..call.span.end as usize];
        let mut declarations = vec![format!("{shared}{annotation}={initializer}")];
        let mut defaults = Vec::new();
        let project = |binding: &BindingPattern<'_>,
                       read: String,
                       temporary: String|
         -> Result<(String, Option<oxc_span::Span>), String> {
            let (id, default) = match binding {
                BindingPattern::BindingIdentifier(id) => (id.as_ref(), None),
                BindingPattern::AssignmentPattern(assignment) => {
                    let BindingPattern::BindingIdentifier(id) = &assignment.left else {
                        return Err(fail("nested $derived patterns are unsupported"));
                    };
                    (id.as_ref(), Some(assignment.right.span()))
                }
                _ => return Err(fail("nested $derived patterns are unsupported")),
            };
            let init = if let Some(span) = default {
                let expression = &source[span.start as usize..span.end as usize];
                format!(
                    "$derived.by(()=>{{const {temporary}={read};return {temporary}===void 0?({expression}):{temporary}}})"
                )
            } else {
                format!("$derived({read})")
            };
            Ok((format!("{}={init}", id.name), default))
        };
        match &decl.id {
            BindingPattern::ObjectPattern(pattern) => {
                if pattern.rest.is_some() {
                    return Err(fail("rest $derived patterns are unsupported"));
                }
                for property in &pattern.properties {
                    if property.computed {
                        return Err(fail("computed $derived keys are unsupported"));
                    }
                    let key = match &property.key {
                        PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                        PropertyKey::StringLiteral(string) => string.value.as_str(),
                        _ => return Err(fail("unsupported $derived property key")),
                    };
                    let quoted = serde_json::to_string(key).map_err(|error| {
                        fail(&format!("cannot encode $derived property key: {error}"))
                    })?;
                    let (projection, default) = project(
                        &property.value,
                        format!("{shared}[{quoted}]"),
                        fresh("__crepusDerivedDefault"),
                    )?;
                    declarations.push(projection);
                    defaults.extend(default);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                if pattern.rest.is_some() {
                    return Err(fail("rest $derived patterns are unsupported"));
                }
                // The helper uses platform intrinsics rather than authored Array/Symbol bindings.
                if nodes.iter().any(|node| matches!(node.kind(), AstKind::BindingIdentifier(id) if id.name == "globalThis")) && !globals.is_active() {
                    return Err(fail("array $derived patterns with a shadowed globalThis are unsupported"));
                }
                let materialized = fresh("__crepusDerivedArray");
                let gather = fresh("__crepusDerivedGather");
                let input = fresh("__crepusDerivedInput");
                let slots: Vec<_> = pattern
                    .elements
                    .iter()
                    .enumerate()
                    .map(|(index, _)| fresh(&format!("__crepusDerivedItem{index}")))
                    .collect();
                let helper = crate::svelte_pattern::fixed_gather(&gather, &input, &slots, globals);
                // One gather memo for all projections, including elided positions.
                declarations.push(format!(
                    "{materialized}=$derived.by(()=>{{{helper}return {gather}({shared})}})"
                ));
                for (index, binding) in pattern.elements.iter().enumerate() {
                    if let Some(binding) = binding {
                        let (projection, default) = project(
                            binding,
                            format!("{materialized}[{index}]"),
                            fresh("__crepusDerivedDefault"),
                        )?;
                        declarations.push(projection);
                        defaults.extend(default);
                    }
                }
            }
            _ => return Err(fail("unsupported $derived binding pattern")),
        }
        // Keep comments removed with the pattern, but don't duplicate comments already
        // present in the initializer, type annotation or copied default expressions.
        let comments = parsed
            .program
            .comments
            .iter()
            .filter(|comment| {
                let span = comment.span;
                span.start >= decl.span.start
                    && span.end <= decl.span.end
                    && !(span.start >= call.span.start && span.end <= call.span.end)
                    && !decl.type_annotation.as_ref().is_some_and(|annotation| {
                        span.start >= annotation.span.start && span.end <= annotation.span.end
                    })
                    && !defaults
                        .iter()
                        .any(|default| span.start >= default.start && span.end <= default.end)
            })
            .map(|comment| {
                format!(
                    "{}\n",
                    &source[comment.span.start as usize..comment.span.end as usize]
                )
            })
            .collect::<String>();
        pattern_edits.push((
            decl.span.start as usize,
            decl.span.end as usize,
            // The authored keyword may touch the opening pattern (`let{...}`).
            format!(" {comments}{}", declarations.join(",")),
        ));
    }
    if !pattern_edits.is_empty() {
        pattern_edits.sort_by_key(|edit| edit.0);
        if pattern_edits.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(format!(
                "{}: nested destructured rune declarations are unsupported",
                path.display()
            ));
        }
        let mut expanded = source.to_owned();
        for (start, end, replacement) in pattern_edits.into_iter().rev() {
            expanded.replace_range(start..end, &replacement);
        }
        return transform_context(&expanded, path, target, context, policy, globals);
    }
    let cell = unique("__crepusSvelteCell");
    let raw_cell = unique("__crepusSvelteRawCell");
    let derived_helper = unique("__crepusSvelteDerived");
    let snapshot = unique("__crepusSvelteSnapshot");
    let unbound = unique("__crepusSvelteUnbound");
    let mut unbound_used = false;
    let mut edits = Vec::new();
    let mut bindings = Vec::new();
    let mut admitted = HashSet::new();
    let mut state_used = false;
    let mut raw_used = false;
    let mut derived_used = false;
    let mut snapshot_used = false;
    let fail = |message: &str| format!("{}: {message}", path.display());
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(decl) = node.kind() else {
            continue;
        };
        let Some(Expression::CallExpression(call)) = &decl.init else {
            continue;
        };
        let (callee, by, raw, member_optional, callee_span) = match &call.callee {
            Expression::Identifier(callee) => (callee, false, false, false, callee.span),
            Expression::StaticMemberExpression(member) => {
                let Expression::Identifier(callee) = &member.object else {
                    continue;
                };
                let by = callee.name == "$derived" && member.property.name == "by";
                let raw = callee.name == "$state" && member.property.name == "raw";
                if !by && !raw {
                    continue;
                }
                (callee, by, raw, member.optional, member.span)
            }
            _ => continue,
        };
        if !callee.name.starts_with('$') || has_runtime_binding(&semantic, callee) {
            continue;
        }
        if !matches!(callee.name.as_str(), "$state" | "$derived" | "$props") {
            continue;
        }
        let ancestors: Vec<_> = nodes.ancestors(node.id()).map(|n| n.kind()).collect();
        let component_owned = instance.is_some_and(|(component_name, _)| ancestors.iter().any(|kind| matches!(kind, AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==component_name))));
        if let Some((component_name, _)) =
            instance.filter(|_| component_owned || matches!(context, RuneContext::Instance(..)))
        {
            let function = ancestors.iter().find_map(|kind| match kind {
                AstKind::Function(f) => Some(*f),
                _ => None,
            });
            if !function.is_some_and(|f| f.id.as_ref().is_some_and(|id| id.name == component_name))
                || ancestors.iter().any(|kind| {
                    matches!(
                        kind,
                        AstKind::ArrowFunctionExpression(_) | AstKind::BlockStatement(_)
                    )
                })
            {
                return Err(fail(
                    "runes must be declared directly in the component instance",
                ));
            }
        } else {
            if callee.name == "$props" {
                return Err(fail(
                    "$props is unsupported outside component initialization",
                ));
            }
        }
        let BindingPattern::BindingIdentifier(id) = &decl.id else {
            return Err(fail("destructured rune declarations are not supported yet"));
        };
        let symbol = id.symbol_id.get().ok_or("unresolved rune binding")?;
        let stable_export = !component_owned && exported.contains(&symbol);
        if stable_export {
            if callee.name == "$derived" {
                return Err(fail(
                    "cannot export a derived binding; export a function that reads it",
                ));
            }
            if nodes.iter().any(|node| match node.kind() {
                AstKind::IdentifierReference(id) => {
                    id.reference_id.get().is_some_and(|reference| {
                        let reference = semantic.scoping().get_reference(reference);
                        reference.symbol_id() == Some(symbol) && reference.is_write()
                    })
                }
                _ => false,
            }) {
                return Err(fail(
                    "cannot export reassigned state; export a function that reads it",
                ));
            }
        }
        if call.optional
            || member_optional
            || (callee.name != "$state" && call.type_arguments.is_some())
        {
            return Err(fail("optional or generic rune calls are unsupported"));
        }
        if callee.name == "$props" {
            if !call.arguments.is_empty() {
                return Err(fail("$props takes no arguments"));
            }
            edits.push((
                call.span.start as usize,
                call.span.end as usize,
                instance.unwrap().1.to_owned(),
            ));
        } else {
            let derived = callee.name == "$derived";
            if call.arguments.len() > 1 || (derived && call.arguments.is_empty()) {
                return Err(fail(
                    "$state requires zero or one arguments; $derived requires one",
                ));
            }
            if let Some(types) = &call.type_arguments {
                if types.params.len() != 1 {
                    return Err(fail("$state accepts one type argument"));
                }
                if call.arguments.is_empty() {
                    let span = types.params[0].span();
                    let ty = &source[span.start as usize..span.end as usize];
                    edits.push((
                        types.span.start as usize,
                        types.span.end as usize,
                        format!("<({ty}) | undefined>"),
                    ));
                }
            }
            let arg = call.arguments.first();
            if matches!(arg, Some(Argument::SpreadElement(_))) {
                return Err(fail("spread rune arguments are unsupported"));
            }
            if by
                && !matches!(
                    arg,
                    Some(
                        Argument::ArrowFunctionExpression(_)
                            | Argument::FunctionExpression(_)
                            | Argument::Identifier(_)
                    )
                )
            {
                return Err(fail("$derived.by requires a function"));
            }
            edits.push((
                callee_span.start as usize,
                callee_span.end as usize,
                if by {
                    derived_helper.clone()
                } else if derived {
                    format!("{derived_helper}(() => ")
                } else if raw && matches!(target, MoonshineModuleTarget::Dom) {
                    raw_cell.clone()
                } else {
                    cell.clone()
                },
            ));
            if derived {
                if !by {
                    edits.push((
                        arg.unwrap().span().end as usize,
                        arg.unwrap().span().end as usize,
                        if component_owned && matches!(target, MoonshineModuleTarget::Ssr) {
                            "), true".into()
                        } else {
                            ")".into()
                        },
                    ));
                } else if component_owned && matches!(target, MoonshineModuleTarget::Ssr) {
                    let end = arg.unwrap().span().end as usize;
                    edits.push((end, end, ", true".into()));
                }
                if by
                    && (matches!(arg, Some(Argument::ArrowFunctionExpression(function)) if function.r#async)
                        || matches!(arg, Some(Argument::FunctionExpression(function)) if function.r#async || function.generator))
                {
                    return Err(fail(
                        "async/generator $derived.by callbacks are unsupported",
                    ));
                }
                derived_used = true;
            } else {
                if raw && matches!(target, MoonshineModuleTarget::Dom) {
                    raw_used = true;
                } else {
                    state_used = true;
                }
                if call.arguments.is_empty() {
                    edits.push((
                        (call.span.end - 1) as usize,
                        (call.span.end - 1) as usize,
                        "void 0".into(),
                    ));
                }
            }
            if stable_export {
                edits.push((
                    call.span.end as usize,
                    call.span.end as usize,
                    ".value".into(),
                ));
            }
            if let Some(annotation) = decl.type_annotation.as_ref().filter(|_| !stable_export) {
                let span = annotation.type_annotation.span();
                let ty = &source[span.start as usize..span.end as usize];
                edits.push((
                    annotation.span.start as usize,
                    annotation.span.end as usize,
                    format!(": {{value: {ty}}}"),
                ));
            }
            if !stable_export {
                bindings.push((symbol, decl.kind == VariableDeclarationKind::Const));
            }
        }
        admitted.insert(callee.span.start);
    }
    if let Some((component_name, _)) = instance {
        for node in nodes.iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression()
            else {
                continue;
            };
            let Expression::Identifier(callee) = member.object.get_inner_expression() else {
                continue;
            };
            if callee.name != "$state"
                || member.property.name != "snapshot"
                || has_runtime_binding(&semantic, callee)
            {
                continue;
            }
            if !matches!(context,RuneContext::ComponentModule(..)) && !nodes.ancestors(node.id()).any(|n|matches!(n.kind(),AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==component_name))) {return Err(fail("$state.snapshot must be used within a component instance"));}
            if call.optional
                || member.optional
                || call.type_arguments.is_some()
                || call.arguments.len() != 1
                || matches!(call.arguments.first(), Some(Argument::SpreadElement(_)))
            {
                return Err(fail("$state.snapshot requires one non-spread argument"));
            }
            // Pinned Svelte SSR keeps direct snapshot initializers as raw values.
            let raw_initializer = matches!(target, MoonshineModuleTarget::Ssr)
            && nodes.ancestors(node.id()).any(|ancestor| {
                matches!(ancestor.kind(), AstKind::VariableDeclarator(decl) if decl.init.as_ref().is_some_and(|init| init.get_inner_expression().span() == call.span))
            });
            let comments = |start: u32, end: u32| {
                parsed
                    .program
                    .comments
                    .iter()
                    .filter(|comment| comment.span.start >= start && comment.span.end <= end)
                    .map(|comment| {
                        format!(
                            "{}\n",
                            &source[comment.span.start as usize..comment.span.end as usize]
                        )
                    })
                    .collect::<String>()
            };
            if raw_initializer {
                // Keep nested argument edits intact while removing call syntax and trailing commas.
                let argument = call.arguments[0].span();
                edits.push((
                    call.span.start as usize,
                    argument.start as usize,
                    format!("({}", comments(call.span.start, argument.start)),
                ));
                edits.push((
                    argument.end as usize,
                    call.span.end as usize,
                    format!("{})", comments(argument.end, call.span.end)),
                ));
            } else {
                let span = call.callee.span();
                let comments = comments(span.start, span.end);
                let replacement = if comments.is_empty() {
                    snapshot.clone()
                } else {
                    format!("({comments}{snapshot})")
                };
                edits.push((span.start as usize, span.end as usize, replacement));
            }
            admitted.insert(callee.span.start);
            snapshot_used |= !raw_initializer;
        }
    }
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().unwrap());
        let cell_binding = bindings
            .iter()
            .find(|(symbol, _)| Some(*symbol) == reference.symbol_id());
        let type_reference = reference.is_type() || reference.flags().is_value_as_type();
        if type_reference
            && cell_binding.is_none()
            && (instance.is_none() || reference.symbol_id().is_some())
        {
            continue;
        }
        if !has_runtime_binding(&semantic, id)
            && id.name.starts_with('$')
            && (instance.is_some() || is_rune(id.name.as_str()))
            && !admitted.contains(&id.span.start)
        {
            if instance.is_none() {
                return Err(fail(&format!(
                    "unsupported or misplaced Svelte rune {}",
                    id.name
                )));
            }
            return Err(fail("unsupported or misplaced Svelte rune"));
        }
        let Some((_, constant)) = cell_binding else {
            continue;
        };
        if type_reference {
            return Err(fail(
                "rune bindings in TypeScript type queries are not supported yet",
            ));
        }
        if *constant && reference.is_write() {
            return Err(fail("assigning to a const rune binding is invalid"));
        }
        let mut reference_span = id.span;
        let mut direct_call = false;
        for parent in nodes.ancestors(node.id()) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => {
                    reference_span = parent.kind().span();
                    continue;
                }
                AstKind::CallExpression(call) => direct_call = call.callee.span() == reference_span,
                AstKind::TaggedTemplateExpression(tag) => {
                    direct_call = tag.tag.span() == reference_span
                }
                _ => {}
            }
            break;
        }
        if direct_call {
            let mut wrapped =
                source[reference_span.start as usize..reference_span.end as usize].to_owned();
            wrapped.replace_range(
                (id.span.start - reference_span.start) as usize
                    ..(id.span.end - reference_span.start) as usize,
                &format!("{}.value", id.name),
            );
            edits.push((
                reference_span.start as usize,
                reference_span.end as usize,
                format!("{unbound}({wrapped})"),
            ));
            unbound_used = true;
            continue;
        }
        let replacement = format!("{}.value", id.name);
        let replacement = match nodes.parent_kind(node.id()) {
            AstKind::ObjectProperty(p) if p.shorthand => {
                crate::svelte_expression::shorthand_property(id.name.as_str(), &replacement)
            }
            AstKind::AssignmentTargetPropertyIdentifier(_) => format!("{}: {replacement}", id.name),
            _ => replacement,
        };
        edits.push((id.span.start as usize, id.span.end as usize, replacement));
    }
    edits.sort_by_key(|e| (e.0, e.1));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    let mut imports = Vec::new();
    if matches!(target, MoonshineModuleTarget::Dom) {
        if state_used {
            imports.push(format!("createState as {cell}"));
        }
        if derived_used {
            imports.push(format!("createDerived as {derived_helper}"));
        }
        if raw_used {
            imports.push(format!("createRawState as {raw_cell}"));
        }
    } else {
        let mut server_imports = Vec::new();
        if state_used {
            server_imports.push(format!("createServerState as {cell}"));
        }
        if derived_used {
            server_imports.push(format!("createServerDerived as {derived_helper}"));
        }
        if !server_imports.is_empty() {
            out = format!(
                "import {{ {} }} from \"@tschk/moonshine-svelte/ssr\";\n{out}",
                server_imports.join(", ")
            );
        }
    }
    if snapshot_used {
        imports.push(format!("stateSnapshot as {snapshot}"));
    }
    if !imports.is_empty() {
        out = format!(
            "import {{ {} }} from \"@tschk/moonshine-svelte/runes\";\n{out}",
            imports.join(", ")
        );
    }
    if matches!(context, RuneContext::Companion)
        && globals.is_active()
        && out.contains(&globals.realm)
    {
        out = format!("{}{out}", globals.imports);
    }
    if unbound_used {
        out.push_str(&format!(
            "\nfunction {unbound}<T>(value:T):T {{ return value; }}\n"
        ));
    }
    if instance.is_none()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("js"))
    {
        // Only generated companion helpers contain TypeScript in a JS module.
        // The caller validates the original module with its authored source type.
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::ts()).parse();
        if !parsed.errors.is_empty() {
            return Err(fail("invalid generated companion module"));
        }
        let mut program = parsed.program;
        program.source_type = SourceType::mjs();
        let built = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&program);
        if !built.errors.is_empty() {
            return Err(fail("invalid generated companion scope"));
        }
        let scoping = built.semantic.into_scoping();
        // Re-enable only the existing TS erasure pass after JS-compatible scope checks.
        program.source_type = SourceType::ts();
        let transformed = Transformer::new(&allocator, path, &TransformOptions::default())
            .build_with_scoping(scoping, &mut program);
        if !transformed.errors.is_empty() {
            return Err(fail("companion TypeScript helper erasure failed"));
        }
        out = Codegen::new().build(&program).code;
        let parsed = Parser::new(&allocator, &out, SourceType::mjs()).parse();
        if !parsed.errors.is_empty() {
            return Err(fail("invalid erased companion JavaScript"));
        }
        let checked = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !checked.errors.is_empty() {
            return Err(fail("invalid erased companion JavaScript scope"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(script: &str) -> Result<String, String> {
        transform(&format!("export default function Component(props){{ {script}; return <button>{{count}}</button> }}"),Path::new("counter.svelte"),MoonshineModuleTarget::Dom,"Component","props").map(|out| {
            let allocator=Allocator::default();
            let parsed=Parser::new(&allocator,&out,SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(),"generated TSX errors: {:?}\n{out}",parsed.errors);
            out
        })
    }
    #[test]
    fn instantiated_rune_callbacks_are_unbound_without_unbinding_methods() {
        let source="export default function Component(props){let fn=$state.raw(function<T>(this:void){return this===undefined});let object=$state.raw({method<T>(){return this}});const call=(fn<number>)();const optional=(fn<string>)?.();const tag=(fn<boolean>)`text`;const method=(object.method<number>)();return <p>{call}</p>}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = transform(
                source,
                Path::new("generic.svelte"),
                target,
                "Component",
                "props",
            )
            .unwrap();
            assert!(
                out.contains("__crepusSvelteUnbound((fn.value<number>))()"),
                "{out}"
            );
            assert!(
                out.contains("__crepusSvelteUnbound((fn.value<string>))?.()"),
                "{out}"
            );
            assert!(
                out.contains("__crepusSvelteUnbound((fn.value<boolean>))`text`"),
                "{out}"
            );
            assert!(out.contains("(object.value.method<number>)()"), "{out}");
            assert!(!out.contains("__crepusSvelteUnbound(object.value"), "{out}");
            let allocator = Allocator::default();
            assert!(
                Parser::new(&allocator, &out, SourceType::tsx())
                    .parse()
                    .errors
                    .is_empty(),
                "{out}"
            );
        }
    }
    #[test]
    fn compact_derived_patterns_keep_keyword_token_boundaries() {
        let out = lower("let count=$state(1);let{value:renamed}=$derived({value:count});let[first]=$derived([count]);return [renamed,first]").unwrap();
        assert!(out.contains("let __crepusDerivedPattern"), "{out}");
        assert!(!out.contains("let__crepus"), "{out}");
    }

    #[test]
    fn derived_object_and_array_patterns_reuse_scalar_scope_lowering() {
        let script = "let count=$state(1);let {value:renamed,missing=7}=$derived({value:count,missing:void 0});let [first,second]=$derived([count,renamed]);renamed=99;first=88;function shadow(renamed){return renamed};const obj={renamed};return [renamed,missing,first,second]";
        let out = lower(script).unwrap();
        assert!(
            out.contains("__crepusDerivedPattern.value[\"value\"]"),
            "{out}"
        );
        assert!(out.contains("===void 0?(7)"), "{out}");
        assert!(out.contains("renamed.value=99"), "{out}");
        assert!(out.contains("first.value=88"), "{out}");
        assert!(
            out.contains("function shadow(renamed){return renamed}"),
            "{out}"
        );
        assert!(out.contains("renamed: renamed.value"), "{out}");
        assert!(out.contains("globalThis.Array.isArray"), "{out}");
        assert!(out.contains("globalThis.Symbol.iterator in"), "{out}");
        assert!(out.contains("globalThis.Array.from"), "{out}");
        assert!(out.contains("T extends readonly unknown[] ? T"), "{out}");
        assert!(!out.contains("Iterable<any>"), "{out}");
    }

    #[test]
    fn derived_pattern_defaults_remain_lazy_and_elisions_are_materialized() {
        let out = lower("let count=$state(1);let {value=make(count),other=unused()}=$derived(read());let [,second=make(count)]=$derived(iterable());return value+second").unwrap();
        assert!(
            out.contains("? (make(count.value))") || out.contains("?(make(count.value))"),
            "{out}"
        );
        assert!(out.contains("?(unused())"), "{out}");
        // Both iterator positions receive temporaries, even though the first is elided.
        assert!(
            out.contains("const [__crepusDerivedItem0,__crepusDerivedItem1]"),
            "{out}"
        );
        assert!(
            out.contains("[__crepusDerivedItem0,__crepusDerivedItem1] as unknown"),
            "{out}"
        );
        assert!(out.contains("readonly unknown[]"), "{out}");
    }

    #[test]
    fn derived_patterns_preserve_annotations_comments_and_generated_name_collisions() {
        let out = lower("const __crepusDerivedPattern=0;const __crepusDerivedItem0=0;let { /* keep field */ value:renamed }: {value:number}=$derived({value:1});let [item]:readonly [number]=$derived([1]);return renamed+item").unwrap();
        assert!(out.contains("/* keep field */"), "{out}");
        assert!(
            out.contains("__crepusDerivedPattern_: {value: {value:number}}")
                || out.contains("__crepusDerivedPattern_:{value: {value:number}}"),
            "{out}"
        );
        assert!(out.contains("__crepusDerivedItem0_"), "{out}");
        assert!(out.contains("readonly [number]"), "{out}");
    }

    #[test]
    fn derived_patterns_keep_server_instance_cache_and_companion_mode_capture() {
        let source = "export default function Component(props){let {value}=$derived(props.object);let [item]=$derived.by(()=>props.iterable);return [value,item]}";
        let out = transform(
            source,
            Path::new("pattern.svelte"),
            MoonshineModuleTarget::Ssr,
            "Component",
            "props",
        )
        .unwrap();
        assert!(out.contains("createServerDerived as"), "{out}");
        assert!(out.contains("), true)"), "{out}");
        let companion=transform_companion("export function factory(input){let {value}=$derived(input);let [item]=$derived.by(()=>input);return {get value(){return value},get item(){return item}}}",Path::new("pattern.svelte.js"),MoonshineModuleTarget::Ssr).unwrap();
        assert!(companion.contains("createServerDerived as"), "{companion}");
        assert!(!companion.contains(", true)"), "{companion}");
        assert!(!companion.contains("<T>"), "{companion}");
        assert!(!companion.contains(" as Iterable"), "{companion}");
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &companion, SourceType::mjs())
                .parse()
                .errors
                .is_empty(),
            "{companion}"
        );
    }

    #[test]
    fn unsupported_derived_patterns_fail_closed_and_const_projections_stay_const() {
        for source in [
            "let {nested:{value}}=$derived(read())",
            "let [[value]]=$derived(read())",
            "let {value,...rest}=$derived(read())",
            "let [value,...rest]=$derived(read())",
            "let {[key]:value}=$derived(read())",
            "let {value}=$state({value:1})",
            "const {value}=$derived({value:1});value=2",
            "function nested(){let {value}=$derived(read())}",
            "const globalThis={};let [value]=$derived(read())",
        ] {
            assert!(lower(source).is_err(), "accepted {source}");
        }
        let out = lower(
            "function $derived(value){return value};const {value}=$derived({value:1});return value",
        )
        .unwrap();
        assert!(!out.contains("createDerived as"), "{out}");
    }

    #[test]
    fn no_argument_state_initializers_ignore_lexically_shadowed_undefined() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for rune in ["$state", "$state.raw"] {
                let body = format!(
                    "const undefined=7;let value={rune}();let typed={rune}<number>();return [value,typed]"
                );
                let source = format!("export default function Component(props){{{body}}}");
                let out = transform(
                    &source,
                    Path::new("fixture.svelte"),
                    target,
                    "Component",
                    "props",
                )
                .unwrap();
                assert!(out.contains("(void 0)"), "{out}");
                assert!(out.contains("<(number) | undefined>(void 0)"), "{out}");
                assert!(!out.contains("(undefined)"), "{out}");
                assert!(out.contains("const undefined=7"));
                let allocator = Allocator::default();
                assert!(
                    Parser::new(&allocator, &out, SourceType::tsx())
                        .parse()
                        .errors
                        .is_empty(),
                    "{out}"
                );
                let companion = transform_companion(
                    &format!("export function factory(){{{body}}}"),
                    Path::new("factory.svelte.ts"),
                    target,
                )
                .unwrap();
                assert!(
                    companion.contains("<(number) | undefined>(void 0)"),
                    "{companion}"
                );
                assert!(!companion.contains("(undefined)"), "{companion}");
                let allocator = Allocator::default();
                assert!(
                    Parser::new(&allocator, &companion, SourceType::ts())
                        .parse()
                        .errors
                        .is_empty(),
                    "{companion}"
                );
            }
        }
    }

    #[test]
    fn normal_derived_trailing_commas_comments_and_parentheses_remain_valid() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for argument in [
                "state,",
                "(state),",
                "state /* tail */,",
                "(state) // tail\n,",
            ] {
                let body =
                    format!("let state=$state(1);let value=$derived({argument});return value");
                let source = format!("export default function Component(props){{{body}}}");
                let out = transform(
                    &source,
                    Path::new("fixture.svelte"),
                    target,
                    "Component",
                    "props",
                )
                .unwrap();
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
                assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
                assert_eq!(
                    out.contains("), true"),
                    matches!(target, MoonshineModuleTarget::Ssr)
                );
                let companion = transform_companion(
                    &format!("export function factory(){{{body}}}"),
                    Path::new("factory.svelte.ts"),
                    target,
                )
                .unwrap();
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, &companion, SourceType::ts()).parse();
                assert!(parsed.errors.is_empty(), "{:?}\n{companion}", parsed.errors);
                assert!(!companion.contains(", true"));
            }
        }
    }

    #[test]
    fn server_helpers_are_imported_hygienically_with_instance_cache_policy() {
        let source = "const __crepusSvelteCell=0;const __crepusSvelteDerived=0;export default function Component(props){let state=$state.raw<number>(1);let value=$derived(state+1);let by=$derived.by(()=>state, /* trailing */);return [value,by]}";
        let out = transform(
            source,
            Path::new("server.svelte"),
            MoonshineModuleTarget::Ssr,
            "Component",
            "props",
        )
        .unwrap();
        assert!(out.contains("createServerState as __crepusSvelteCell_"));
        assert!(out.contains("createServerDerived as __crepusSvelteDerived_"));
        assert!(out.contains("__crepusSvelteDerived_(() => (state.value+1), true)"));
        assert!(out.contains("__crepusSvelteDerived_(()=>state.value, true, /* trailing */)"));
        assert!(!out.contains("function __crepusSvelteDerived_"));
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
    }

    #[test]
    fn companion_server_helpers_keep_runtime_selected_cache_policy() {
        let out = transform_companion("export function factory(){let state=$state(1);let value=$derived(state+1);let by=$derived.by(()=>state,);return [value,by]}", Path::new("factory.svelte.ts"), MoonshineModuleTarget::Ssr).unwrap();
        assert!(out.contains("createServerState as __crepusSvelteCell"));
        assert!(out.contains("createServerDerived as __crepusSvelteDerived"));
        assert!(out.contains("__crepusSvelteDerived(() => (state.value+1))"));
        assert!(out.contains("__crepusSvelteDerived(()=>state.value,)"));
        assert!(!out.contains(", true"));
        assert!(!out.contains("isSSRRendering"));
    }

    #[test]
    fn server_state_is_raw_and_derived_is_a_pure_successful_cache() {
        let source = "export default function Component(props){let count=$state(props.raw);let d=$derived(count.value);let by=$derived.by(()=>count.value);d=null;return <p>{d}</p>}";
        let out = transform(
            source,
            Path::new("server.svelte"),
            MoonshineModuleTarget::Ssr,
            "Component",
            "props",
        )
        .unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        assert!(out.contains("createServerState as __crepusSvelteCell"));
        assert!(out.contains("createServerDerived as __crepusSvelteDerived"));
        assert!(out.contains("from \"@tschk/moonshine-svelte/ssr\""));
        assert!(!out.contains("function __crepusSvelteCell"));
        assert!(!out.contains("function __crepusSvelteDerived"));
        assert!(out.contains("d.value=null"));
        assert!(!out.contains("getOwner"));
        let snapshot=transform("export default function Component(props){let state=$state({});return $state.snapshot(state)}",Path::new("server.svelte"),MoonshineModuleTarget::Ssr,"Component","props").unwrap();
        assert!(snapshot.contains("stateSnapshot as __crepusSvelteSnapshot"));
        assert!(!snapshot.contains("createState as"));
        assert!(!snapshot.contains("createDerived as"));
    }
    #[test]
    fn scalar_updates_and_derived_are_scope_aware() {
        let out=lower("let count=$state(0);let doubled=$derived(count*2);const obj={count};function nested(count){return count}; const old=count++;count+=2;({count}={count:3})").unwrap();
        assert!(out.contains("count.value++"));
        assert!(out.contains("count.value+=2"));
        assert!(out.contains("({count: count.value}={count:3})"));
        assert!(out.contains("{count: count.value}"));
        assert!(out.contains("function nested(count){return count}"));
        assert!(out.contains("(() => (count.value*2))"));
    }
    #[test]
    fn derived_by_keeps_callbacks_and_resolves_their_lexical_references() {
        let out = lower("let count=$state(1);let total=$derived.by(()=>{let count=3;return count});let doubled=$derived.by(function(){return count*2});let sum=$derived.by(read);function read(){return count+1}").unwrap();
        assert!(out.contains("__crepusSvelteDerived(()=>{let count=3;return count})"));
        assert!(out.contains("__crepusSvelteDerived(function(){return count.value*2})"));
        assert!(out.contains("__crepusSvelteDerived(read)"));
        for script in [
            "let count=0;let d=$derived.by(1)",
            "let count=0;let d=$derived.by(async()=>count)",
            "let count=0;let d=$derived.by(function*(){yield count})",
            "let count=0;let d=$derived.by(()=>count,()=>count)",
        ] {
            assert!(lower(script).is_err(), "{script}");
        }
    }
    #[test]
    fn derived_initializes_lazily_under_the_instance_owner() {
        let out = lower("let doubled=$derived(count*2);let count=$state(0)").unwrap();
        assert!(out.contains("let doubled=__crepusSvelteDerived(() => (count.value*2))"));
        assert!(out.contains("createDerived as __crepusSvelteDerived"));
        assert!(out.contains("@tschk/moonshine-svelte/runes"));
        assert!(!out.contains("function __crepusSvelteDerived"));
    }
    #[test]
    fn mutable_derived_values_use_cells_for_all_assignment_forms() {
        let out =
            lower("let count=$state(0);let d=$derived(count);d++;d+=2;const obj={d};({d}={d:3})")
                .unwrap();
        assert!(out.contains("d.value++"));
        assert!(out.contains("d.value+=2"));
        assert!(out.contains("{d: d.value}"));
        assert!(out.contains("({d: d.value}={d:3})"));
        assert!(lower("let count=$state(0);const d=$derived(count);d++").is_err());
    }
    #[test]
    fn rejects_unsafe_or_misplaced_runes() {
        for script in [
            "let count=$state.eager(1)",
            "let count=0;$effect(()=>count++)",
            "let count=0;function child(){let x=$state(1)}",
            "let {count}=$props()",
            "const count=$state(0);count++",
            "let count=0;console.log($store)",
            "let count=$state(0);type T=typeof count",
        ] {
            assert!(lower(script).is_err(), "{script}");
        }
    }
    #[test]
    fn shadowed_rune_is_normal_javascript() {
        let out = lower("function $state(x){return x};let count=$state({x:1})").unwrap();
        assert!(out.contains("let count=$state({x:1})"));
        assert!(!out.contains("import"));
    }
    #[test]
    fn deep_state_initializers_and_replacement_keep_native_javascript() {
        let out=lower("let count=$state({items:[{value:1}]});count.items[0].value++;count={items:[]};let fn=$state(function(){return this});fn();(fn)();(fn as Function)();let empty=$state();let typed=$state<{value:number}>({value:2});let optional=$state<number>()").unwrap();
        assert!(out.contains("createState as __crepusSvelteCell"));
        assert!(out.contains("count.value.items[0].value++"));
        assert!(out.contains("count.value={items:[]}"));
        assert!(out.contains("__crepusSvelteUnbound(fn.value)()"));
        assert!(out.contains("__crepusSvelteUnbound((fn.value))()"));
        assert!(out.contains("__crepusSvelteUnbound((fn.value as Function))()"));
        assert!(out.contains("empty=__crepusSvelteCell(void 0)"));
        assert!(out.contains("typed=__crepusSvelteCell<{value:number}>({value:2})"));
        assert!(out.contains("optional=__crepusSvelteCell<(number) | undefined>(void 0)"));
        assert!(!out.contains("function __crepusSvelteCell"));
        assert!(lower("const count=$state({value:1});count.value++").is_ok());
        assert!(lower("const count=$state({value:1});count={value:2}").is_err());
    }
    #[test]
    fn snapshots_rewrite_only_unbound_rune_callees() {
        let out=lower("let count=$state({value:1});const copy=$state.snapshot(count);const handler=()=> $state.snapshot({count});function inner($state){return $state.snapshot(count)}").unwrap();
        assert!(out.contains("stateSnapshot as __crepusSvelteSnapshot"));
        assert!(out.contains("copy=__crepusSvelteSnapshot(count.value)"));
        assert!(out.contains("__crepusSvelteSnapshot({count: count.value})"));
        assert!(out.contains("return $state.snapshot(count.value)"));
        for script in [
            "let count=$state.raw?.({})",
            "let count=$state.eager(1)",
            "let count=$state(1,2)",
            "let count=$state(...values)",
            "let count=0;$state.snapshot()",
            "let count=0;$state.snapshot(...values)",
            "let count=0;$state?.snapshot(count)",
        ] {
            assert!(lower(script).is_err(), "{script}");
        }
    }
    #[test]
    fn raw_state_preserves_identity_with_shallow_reactive_replacements() {
        let out=lower("let count=$state.raw({value:1});count.value++;count={value:2};let fn=$state.raw(()=>1);fn();let empty=$state.raw();let typed=$state.raw<number>()").unwrap();
        assert!(out.contains("count=__crepusSvelteRawCell({value:1})"));
        assert!(out.contains("count.value.value++"));
        assert!(out.contains("count.value={value:2}"));
        assert!(out.contains("createRawState as __crepusSvelteRawCell"));
        assert!(!out.contains("function __crepusSvelteRawCell"));
        assert!(!out.contains("createSignal"));
        assert!(!out.contains("equals:"));
        assert!(out.contains("empty=__crepusSvelteRawCell(void 0)"));
        assert!(out.contains("typed=__crepusSvelteRawCell<(number) | undefined>(void 0)"));
        assert!(!out.contains("createState as"));
        let server=transform("export default function Component(props){let count=$state.raw(props.raw);return count}",Path::new("server.svelte"),MoonshineModuleTarget::Ssr,"Component","props").unwrap();
        assert!(server.contains("createServerState as __crepusSvelteCell"));
        assert!(!server.contains("@tschk/moonshine-svelte/runes"));
        let shadowed =
            lower("function inner($state){let x=$state.raw({});return x};let count=0").unwrap();
        assert!(shadowed.contains("$state.raw({})"));
        for source in [
            "let count=$state.raw(1,2)",
            "let count=$state?.raw(1)",
            "let count=$state.raw(...values)",
            "let {count}=$state.raw({})",
            "let count=0;function child(){let x=$state.raw(0)}",
        ] {
            assert!(lower(source).is_err(), "{source}");
        }
    }

    #[test]
    fn server_snapshot_initializers_preserve_identity_but_other_contexts_clone() {
        let source = "export default function Component(props){const direct=$state.snapshot(props.input);const parens=($state.snapshot(make()));return [direct,parens]}";
        let server = transform(
            source,
            Path::new("snapshot.svelte"),
            MoonshineModuleTarget::Ssr,
            "Component",
            "props",
        )
        .unwrap();
        assert!(server.contains("const direct=(props.input)"));
        assert!(server.contains("const parens=((make()))"));
        assert!(!server.contains("import"));
        let client = transform(
            source,
            Path::new("snapshot.svelte"),
            MoonshineModuleTarget::Dom,
            "Component",
            "props",
        )
        .unwrap();
        assert!(client.contains("stateSnapshot as __crepusSvelteSnapshot"));
        for expression in [
            "return $state.snapshot(props.input)",
            "const value={copy:$state.snapshot(props.input)};return value",
            "let value;value=$state.snapshot(props.input);return value",
            "return take($state.snapshot(props.input))",
            "return [$state.snapshot(props.input)]",
            "const value=$state.snapshot(props.input).n;return value",
        ] {
            let source = format!("export default function Component(props){{{expression}}}");
            let server = transform(
                &source,
                Path::new("snapshot.svelte"),
                MoonshineModuleTarget::Ssr,
                "Component",
                "props",
            )
            .unwrap();
            assert!(
                server.contains("stateSnapshot as __crepusSvelteSnapshot"),
                "{server}"
            );
            assert!(
                server.contains("__crepusSvelteSnapshot(props.input)"),
                "{server}"
            );
        }
    }
    #[test]
    fn props_alias_and_annotations() {
        let out = lower("let p=$props();let count:number=$state(1);let d:number=$derived(count*2)")
            .unwrap();
        assert!(out.contains("let p=props"));
        assert!(out.contains("count: {value: number}"));
        assert!(out.contains("d: {value: number}"));
    }
    #[test]
    fn transparent_snapshot_callees_and_raw_server_trailing_commas() {
        for expression in [
            "$state.snapshot(props.input,)",
            "($state.snapshot)(props.input,)",
            "($state.snapshot as any)(props.input,)",
            "($state.snapshot!)(props.input,)",
            "($state).snapshot(props.input,)",
            "($state as any).snapshot(props.input,)",
            "(($state.snapshot as any)(props.input,) as any)",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let source = format!(
                    "export default function Component(props){{const copy={expression};return copy}}"
                );
                let output = transform(
                    &source,
                    Path::new("snapshot.svelte"),
                    target,
                    "Component",
                    "props",
                )
                .unwrap();
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
                assert!(
                    parsed.errors.is_empty(),
                    "{expression}: {:?}\n{output}",
                    parsed.errors
                );
                if matches!(target, MoonshineModuleTarget::Ssr) {
                    assert!(!output.contains("stateSnapshot as"), "{output}");
                    assert!(output.contains("(props.input)"), "{output}");
                } else {
                    assert!(
                        output.contains("__crepusSvelteSnapshot(props.input,)"),
                        "{output}"
                    );
                }
            }
        }
    }
    #[test]
    fn raw_snapshot_boundaries_preserve_argument_and_nested_reference_edits() {
        let source = "export default function Component(props){let state=$state({count:1});const copy=($state /* head */ .snapshot as any)(/* before */{state,nested:$state.snapshot(state)} /* after */ // tail\n,);return copy}";
        let output = transform(
            source,
            Path::new("snapshot.svelte"),
            MoonshineModuleTarget::Ssr,
            "Component",
            "props",
        )
        .unwrap();
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
        assert!(
            output.contains("state: state.value,nested:__crepusSvelteSnapshot(state.value)"),
            "{output}"
        );
        for comment in ["/* head */", "/* before */", "/* after */", "// tail"] {
            assert!(output.contains(comment), "{output}");
        }
        assert!(output.contains("// tail\n)"), "{output}");
    }
    #[test]
    fn snapshot_comments_remain_grouped_for_return_and_throw() {
        let source = "const __crepusSvelteSnapshot=0;export default function Component(props){function read(){return ($state /* keep */ .snapshot as any)(props.input)}function fail(){throw ($state // line\n .snapshot!)(props.input)}return read()}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = transform(
                source,
                Path::new("snapshot.svelte"),
                target,
                "Component",
                "props",
            )
            .unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{output}");
            assert!(
                output.contains("return (/* keep */\n__crepusSvelteSnapshot_)(props.input)"),
                "{output}"
            );
            assert!(
                output.contains("throw (// line\n__crepusSvelteSnapshot_)(props.input)"),
                "{output}"
            );
        }
    }
    #[test]
    fn wrapped_snapshot_shadowing_and_guard_policy_are_preserved() {
        let output =
            lower("function nested($state){return ($state.snapshot as any)(value)}").unwrap();
        assert!(output.contains("return ($state.snapshot as any)(value)"));
        for script in [
            "($state.snapshot)()",
            "($state.snapshot)(a,b)",
            "($state.snapshot)(...args)",
            "($state.snapshot)?.(value)",
            "($state?.snapshot)(value)",
            "($state.snapshot)<number>(value)",
        ] {
            assert!(lower(script).is_err(), "{script}");
        }
    }
}

#[cfg(test)]
mod companion_tests {
    use super::*;

    fn lower(source: &str, target: MoonshineModuleTarget, javascript: bool) -> String {
        let path = Path::new(if javascript {
            "factory.svelte.js"
        } else {
            "factory.svelte.ts"
        });
        let output = transform_companion(source, path, target).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            &output,
            if javascript {
                SourceType::mjs()
            } else {
                SourceType::ts()
            },
        )
        .parse();
        assert!(parsed.errors.is_empty(), "{output}\n{:?}", parsed.errors);
        output
    }

    #[test]
    fn direct_js_companion_entry_rejects_invalid_authored_source_before_ir() {
        for source in [
            "export const value:number=1",
            "interface Model{value:number};export const value=1",
            "export class Model{#x;#x}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = transform_companion(source, Path::new("direct.svelte.js"), target)
                    .expect_err("raw JS must be validated even without the module/effect caller");
                assert!(
                    error.contains("invalid authored companion JavaScript"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn factories_arrows_blocks_and_closures_share_cell_rewriting() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = lower(
                "export function factory(){let count=$state(0);{let raw=$state.raw({n:1});raw.n++;}let double=$derived(count*2);let by=$derived.by(()=>double);return {get count(){return count},inc:()=>count++,read:()=>({count,by})}}export const arrow=()=>{let x=$state(1);return()=>x};",
                target,
                false,
            );
            assert!(output.contains("count.value++"), "{output}");
            assert!(output.contains("count: count.value"), "{output}");
            assert!(output.contains("by.value"), "{output}");
            assert!(output.contains("raw.value.n++"), "{output}");
            assert!(output.contains("return()=>x.value"), "{output}");
        }
    }

    #[test]
    fn shadowed_runes_unknown_globals_and_type_references_are_preserved() {
        let source = "type State=typeof $state;export function factory($state:any){return $state(1)}export function other(){let count=$state(0);return [$foo,count]}";
        let output = lower(source, MoonshineModuleTarget::Dom, false);
        assert!(output.contains("return $state(1)"));
        assert!(output.contains("typeof $state"));
        assert!(output.contains("[$foo,count.value]"));
    }

    #[test]
    fn companion_placements_share_module_export_law() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "let n=$state(0);let d=$derived(n*2);export function read(){return d}export function set(v:number){n=v}",
                "export const model=$state({n:0});export function inc(){model.n++}",
                "export let count=$state(0)",
                "declare const $state:any;export let count=$state(0)",
                "let n=$state(0);export default n",
                "let n=$state(0);export default {get value(){return n}};export function inc(){n++}",
                "let n=$state(0);export default ()=>n;export function inc(){n++}",
                "{let count=$derived(1)}",
                "export async function f(){let count=$state(0);await Promise.resolve();return()=>count}",
                "export async function f(){await Promise.resolve();let count=$state(0);return()=>count}",
                "export function* f(){yield;let count=$state(0);return()=>count}",
                "export async function* f(){let count=$state(0);yield;await Promise.resolve();return()=>count}",
                "const f=async()=>{let count=$state(0)}",
            ] {
                let output = transform_companion(source, Path::new("factory.svelte.ts"), target)
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
                if source.contains("$state(") {
                    let helper = if matches!(target, MoonshineModuleTarget::Dom) {
                        "createState as"
                    } else {
                        "createServerState as"
                    };
                    assert!(output.contains(helper), "{output}");
                }
                if source.contains("$derived(") {
                    let helper = if matches!(target, MoonshineModuleTarget::Dom) {
                        "createDerived as"
                    } else {
                        "createServerDerived as"
                    };
                    assert!(output.contains(helper), "{output}");
                }
                assert!(!output.contains("$state("), "{output}");
                assert!(!output.contains("$derived("), "{output}");
            }
            for source in [
                "export let n=$state(0);export function inc(){n++}",
                "let n=$state(0);export{n};export function inc(){n++}",
                "let n=$state(0);export{n as default};export function inc(){n++}",
                "let n=$state(0);export default n;export function inc(){n++}",
                "let n=$state(0);export default (n);export function inc(){n++}",
                "export const n=$derived(1)",
                "let n=$derived(1);export default n",
            ] {
                let error = transform_companion(source, Path::new("factory.svelte.ts"), target)
                    .expect_err("invalid exported rune must reject");
                let diagnostic = if source.contains("$derived(") {
                    "cannot export a derived binding"
                } else {
                    "cannot export reassigned state"
                };
                assert!(error.contains(diagnostic), "{source}: {error}");
            }
        }
    }

    #[test]
    fn misplaced_and_unsupported_companion_runes_fail_on_both_hosts() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "function f(){let count=$state(0);type Count=typeof count;return count}",
                "function f(){let {count}=$state({count:0})}",
                "function f(){let props=$props()}",
                "function f(){let x=$bindable()}",
                "function f(){$effect(()=>{})}",
                "function f(){return $effect.tracking()}",
                "function f(){return $state.snapshot({})}",
                "function f(){let x=$state(...values)}",
                "export class Model{constructor(){this.value=$state(0)}}",
                "export class Model{static #value=$state(0)}",
                "export class Model{static value=$derived(1)}",
                "export class Model{static #value=$derived(1)}",
                "const model={value:$state(0)}",
                "export const model=$state.raw(...values)",
            ] {
                assert!(
                    transform_companion(source, Path::new("factory.svelte.ts"), target).is_err(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn public_companion_class_fields_are_lowered_on_both_hosts() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "export class Model{value=$state(0);raw=$state.raw();derived=$derived(this.value*2)}",
                "export class Model{#value=$state(0);#raw=$state.raw();#derived=$derived(this.#value*2);#by=$derived.by(()=>this.#value+1);read(){return this.#derived+this.#by}}",
            ] {
                let output = transform_companion(source, Path::new("model.svelte.ts"), target)
                    .expect("public/private rune class fields");
                assert!(!output.contains("$state"), "{output}");
                assert!(!output.contains("$derived"), "{output}");
            }
        }
    }

    #[test]
    fn component_type_only_rune_names_remain_legal() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = "export default function Component(props){type $state={value:number};let item:$state={value:1};let count=$state(1);return <p>{item.value}/{count}</p>}";
            let output = transform(
                source,
                Path::new("typed.svelte"),
                target,
                "Component",
                "props",
            )
            .unwrap();
            assert!(output.contains("item:$state={value:1}"), "{output}");
            assert!(output.contains("count.value"), "{output}");
            assert!(!output.contains("count=$state(1)"), "{output}");
        }
    }

    #[test]
    fn ambient_rune_types_do_not_shadow_runtime_lowering() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = "declare const $state:<T>(value:T)=>T;declare function $derived<T>(read:T):T;export function factory(){let value=$state(1);let twice=$derived(value*2);return {get value(){return value},get twice(){return twice}}}";
            let output = lower(source, target, false);
            assert!(!output.contains("value=$state(1)"), "{output}");
            assert!(!output.contains("twice=$derived("), "{output}");
            assert!(output.contains("value.value"), "{output}");
            assert!(transform_companion(
                "declare const $effect:any;export function factory(){$effect(()=>{})}",
                Path::new("factory.svelte.ts"),
                target
            )
            .is_err());
            let shadow = lower(
                "function $state(value:number):number;function $state(value:number){return value}export const value=$state(1);",
                target,
                false,
            );
            assert!(shadow.contains("value=$state(1)"), "{shadow}");
            let merged = lower(
                "declare var $state:(value:number)=>number;var $state=(value:number)=>value;export const value=$state(1);",
                target,
                false,
            );
            assert!(merged.contains("value=$state(1)"), "{merged}");
        }
    }

    #[test]
    fn generic_typescript_arrow_factories_remain_typescript() {
        let output = lower(
            "export const identity=<T>(value:T)=>value;export const factory=<T>(initial:T)=>{let value=$state(initial);return()=>value};",
            MoonshineModuleTarget::Dom,
            false,
        );
        assert!(output.contains("identity=<T>(value:T)=>value"), "{output}");
        assert!(output.contains("return()=>value.value"), "{output}");
    }

    #[test]
    fn javascript_companions_erase_generated_helper_types() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = lower(
                "export function factory(){let fn=$state(()=>1);let n=$derived(1);return ()=>fn()+n}",
                target,
                true,
            );
            assert!(!output.contains("<T>"), "{output}");
            assert!(!output.contains(":T"), "{output}");
            assert!(output.contains("__crepusSvelteUnbound"), "{output}");
        }
    }

    fn assert_platform_realm(output: &str, alias: &str) {
        assert!(
            output.contains(&format!("sourceRealm as {alias}")),
            "{output}"
        );
        assert_eq!(
            output.matches("@tschk/moonshine-dom/platform").count(),
            1,
            "{output}"
        );
        assert_eq!(output.matches(&format!("{alias}()")).count(), 3, "{output}");
        assert!(!output.contains("globalThis.Array"), "{output}");
        assert!(!output.contains("globalThis.Symbol"), "{output}");
    }

    #[test]
    fn global_this_binders_lower_array_patterns_through_platform_realm() {
        for source in [
            "const globalThis={marker:1};export function factory(input){let [first]=$derived(input);return ()=>[first,globalThis.marker,globalThis[input]]}",
            "export function factory(globalThis){let [first]=$derived(globalThis.read());return ()=>first}",
            "export function outer(){return function inner(globalThis){let [first]=$derived(globalThis);return ()=>first}}",
            "export function factory(input){try{input()}catch(\\u0067lobalThis){let [first]=$derived(input);return ()=>first}}",
            "import {read as globalThis} from \"./read\";export function factory(){let [first]=$derived(globalThis());return ()=>first}",
        ] {
            for javascript in [false, true] {
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let output = lower(source, target, javascript);
                    assert_platform_realm(&output, "__crepusSourceRealm0");
                    for authored in ["globalThis.marker", "globalThis[input]"] {
                        if source.contains(authored) {
                            assert!(output.contains(authored), "{output}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn global_this_alias_avoids_plain_and_escaped_authored_names() {
        let source = "const __crepusSourceRealm0=1;const \\u005f_crepusSourceRealm1=2;const globalThis={};export function factory(input){let [first]=$derived(input);return ()=>[first,__crepusSourceRealm0,__crepusSourceRealm1,globalThis]}";
        for javascript in [false, true] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = lower(source, target, javascript);
                assert_platform_realm(&output, "__crepusSourceRealm2");
                assert!(output.contains("const __crepusSourceRealm0"), "{output}");
            }
        }
    }

    #[test]
    fn companions_without_array_patterns_or_binders_keep_default_realm_output() {
        for source in [
            "export function factory(input){let [first]=$derived(input);return ()=>[first,globalThis.marker]}",
            "const globalThis={};export function factory(){let value=$state(1);return ()=>[value,globalThis]}",
        ] {
            for name in ["factory.svelte.ts", "factory.svelte.js"] {
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let path = Path::new(name);
                    let default = transform_context(
                        source,
                        path,
                        target,
                        RuneContext::Companion,
                        crate::svelte_javascript::IrPolicy::Typed,
                        &Default::default(),
                    )
                    .unwrap();
                    assert_eq!(transform_companion(source, path, target).unwrap(), default);
                    assert!(!default.contains("sourceRealm"), "{default}");
                }
            }
        }
    }

    #[test]
    fn unsupported_array_patterns_stay_rejected_with_global_this_binders() {
        for source in [
            "const globalThis={};export function factory(input){let [[value]]=$derived(input);return ()=>value}",
            "const globalThis={};export function factory(input){let [value,...rest]=$derived(input);return ()=>value}",
            "const globalThis={};export function factory(input){let {nested:[value]}=$derived(input);return ()=>value}",
            "const globalThis={};export function factory(input){let {value,...rest}=$derived(input);return ()=>value}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(
                    transform_companion(source, Path::new("factory.svelte.ts"), target).is_err(),
                    "{source}"
                );
            }
        }
    }
}
