//! Type-only closure lifting for declaration-emittable component return metadata.
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_parser::Parser;
use oxc_semantic::{SemanticBuilder, SymbolId};
use oxc_span::{SourceType, Span};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

// shortcut: generic captures and dependent local types remain lexical; thread their type parameters only when supported.
/// Lift closed interfaces and non-generic interfaces capturing direct typeof values.
/// Unsupported local dependencies stay lexical rather than widening public types.
pub(crate) fn transform(
    source: &str,
    path: &Path,
    mut fresh: impl FnMut(&str) -> String,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let fail = || format!("{}: invalid Svelte mount type scope", path.display());
    if !parsed.errors.is_empty() {
        return Err(fail());
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail());
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let scopes = semantic.scoping();
    let mut groups: HashMap<SymbolId, Vec<(Span, Span)>> = HashMap::new();
    let mut used = HashSet::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                used.insert(id.name.to_string());
            }
            AstKind::IdentifierReference(id) => {
                used.insert(id.name.to_string());
            }
            _ => {}
        }
        let AstKind::TSInterfaceDeclaration(interface) = node.kind() else {
            continue;
        };
        let Some(symbol) = interface.id.symbol_id.get() else {
            return Err(fail());
        };
        if scopes.symbol_scope_id(symbol) == scopes.root_scope_id() {
            continue;
        }
        // Namespace members have a separate public name/merging contract.
        let mut function = false;
        for ancestor in nodes.ancestor_kinds(node.id()) {
            match ancestor {
                AstKind::TSModuleDeclaration(_) => break,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                    function = true;
                    break;
                }
                _ => {}
            }
        }
        if function {
            groups
                .entry(symbol)
                .or_default()
                .push((interface.span, interface.id.span));
        }
    }
    if groups.is_empty() {
        return Ok(source.to_owned());
    }
    let mut unsafe_groups = HashSet::new();
    let mut dependencies: HashMap<SymbolId, HashSet<SymbolId>> = HashMap::new();
    for (&symbol, declarations) in &groups {
        // A merged value/class/namespace cannot be renamed or moved as an interface.
        if scopes
            .symbol_declarations(symbol)
            .any(|id| !matches!(nodes.kind(id), AstKind::TSInterfaceDeclaration(_)))
        {
            unsafe_groups.insert(symbol);
        }
        for node in nodes.iter() {
            let AstKind::IdentifierReference(id) = node.kind() else {
                continue;
            };
            if !declarations
                .iter()
                .any(|(span, _)| contains(*span, id.span))
            {
                continue;
            }
            let reference = scopes.get_reference(id.reference_id.get().ok_or_else(fail)?);
            let Some(binding) = reference.symbol_id() else {
                // Oxc leaves the function's implicit arguments binding unresolved.
                if id.name == "arguments" {
                    unsafe_groups.insert(symbol);
                }
                continue;
            };
            if scopes.symbol_scope_id(binding) == scopes.root_scope_id()
                && scopes.symbol_flags(binding).is_variable()
                && nodes
                    .ancestor_kinds(node.id())
                    .any(|kind| matches!(kind, AstKind::TSTypeQuery(_)))
            {
                unsafe_groups.insert(symbol);
            }
            if groups.contains_key(&binding) {
                dependencies.entry(symbol).or_default().insert(binding);
            } else if scopes.symbol_scope_id(binding) != scopes.root_scope_id()
                && !declarations
                    .iter()
                    .any(|(span, _)| contains(*span, scopes.symbol_span(binding)))
            {
                unsafe_groups.insert(symbol);
            }
        }
    }
    loop {
        let rejected: Vec<_> = dependencies
            .iter()
            .filter_map(|(&symbol, deps)| {
                (!unsafe_groups.contains(&symbol)
                    && deps.iter().any(|dep| unsafe_groups.contains(dep)))
                .then_some(symbol)
            })
            .collect();
        if rejected.is_empty() {
            break;
        }
        unsafe_groups.extend(rejected);
    }
    let mut ordered: Vec<_> = groups
        .iter()
        .filter(|(symbol, _)| !unsafe_groups.contains(symbol))
        .collect();
    ordered.sort_by_key(|(_, declarations)| declarations.iter().map(|(span, _)| span.start).min());
    let mut names = HashMap::new();
    for (&symbol, _) in &ordered {
        let name = allocate("__crepusSvelteInterface", source, &mut used, &mut fresh);
        names.insert(symbol, name);
    }
    // Captured groups retain a lexical alias, so dependent interfaces must stay
    // lexical too. Only self-recursion and already-closed interface dependencies
    // can be threaded without capturing another lexical alias.
    let mut captured = Vec::new();
    let mut candidates: Vec<_> = groups
        .iter()
        .filter(|(symbol, _)| unsafe_groups.contains(symbol))
        .collect();
    candidates
        .sort_by_key(|(_, declarations)| declarations.iter().map(|(span, _)| span.start).min());
    for (&symbol, declarations) in candidates {
        if scopes.symbol_declarations(symbol).any(|id| !matches!(nodes.kind(id), AstKind::TSInterfaceDeclaration(interface) if interface.type_parameters.is_none())) {
            continue;
        }
        let mut queries = Vec::new();
        let mut supported = true;
        for node in nodes.iter() {
            let AstKind::IdentifierReference(id) = node.kind() else {
                continue;
            };
            if !declarations
                .iter()
                .any(|(span, _)| contains(*span, id.span))
            {
                continue;
            }
            let reference = scopes.get_reference(id.reference_id.get().ok_or_else(fail)?);
            let Some(binding) = reference.symbol_id() else {
                if id.name == "arguments" {
                    supported = false;
                    break;
                }
                continue;
            };
            let query = nodes.ancestor_kinds(node.id()).find_map(|kind| match kind {
                AstKind::TSTypeQuery(query) => Some(query),
                _ => None,
            });
            if binding == symbol
                || (names.contains_key(&binding) && query.is_none())
                || (scopes.symbol_scope_id(binding) == scopes.root_scope_id() && query.is_none())
                || declarations
                    .iter()
                    .any(|(span, _)| contains(*span, scopes.symbol_span(binding)))
            {
                continue;
            }
            let flags = scopes.symbol_flags(binding);
            if !(flags.is_variable() || flags.is_function())
                || flags.is_class()
                || flags.is_type_alias()
                || flags.is_interface()
                || flags.is_enum()
            {
                supported = false;
                break;
            }
            // Signature scopes intentionally discard outer control-flow narrowing.
            if nodes
                .ancestor_kinds(node.id())
                .take_while(|kind| !matches!(kind, AstKind::TSInterfaceDeclaration(_)))
                .any(|kind| {
                    matches!(
                        kind,
                        AstKind::TSMethodSignature(_)
                            | AstKind::TSCallSignatureDeclaration(_)
                            | AstKind::TSConstructSignatureDeclaration(_)
                            | AstKind::TSIndexSignature(_)
                            | AstKind::TSFunctionType(_)
                            | AstKind::TSConstructorType(_)
                    )
                })
            {
                supported = false;
                break;
            }
            let Some(query) = query.filter(|query| {
                query.type_arguments.is_none()
                    && declarations
                        .iter()
                        .any(|(span, _)| contains(*span, query.span))
            }) else {
                supported = false;
                break;
            };
            queries.push(query.span);
        }
        if !supported || queries.is_empty() {
            continue;
        }
        queries.sort_by_key(|span| span.start);
        queries.dedup();
        let lifted_name = allocate("__crepusSvelteInterface", source, &mut used, &mut fresh);
        let queries: Vec<_> = queries
            .into_iter()
            .map(|span| {
                let alias = allocate("__crepusSvelteCapture", source, &mut used, &mut fresh);
                let parameter =
                    allocate("__crepusSvelteCaptureType", source, &mut used, &mut fresh);
                (span, alias, parameter)
            })
            .collect();
        captured.push((symbol, declarations, lifted_name, queries));
    }
    let mut renames = Vec::new();
    for (&symbol, name) in &names {
        for (_, id) in &groups[&symbol] {
            renames.push((*id, name.clone()));
        }
    }
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = scopes.get_reference(id.reference_id.get().ok_or_else(fail)?);
        if let Some(name) = reference.symbol_id().and_then(|symbol| names.get(&symbol)) {
            renames.push((id.span, name.clone()));
        }
    }
    renames.sort_by_key(|(span, _)| span.start);
    let mut declarations: Vec<_> = ordered
        .iter()
        .flat_map(|(_, items)| items.iter().map(|(span, _)| *span))
        .collect();
    declarations.sort_by_key(|span| span.start);
    let mut lifted = String::new();
    for span in &declarations {
        let mut text = source[span.start as usize..span.end as usize].to_owned();
        for (edit, name) in renames
            .iter()
            .rev()
            .filter(|(edit, _)| contains(*span, *edit))
        {
            text.replace_range(
                (edit.start - span.start) as usize..(edit.end - span.start) as usize,
                name,
            );
        }
        lifted.push('\n');
        lifted.push_str(&text);
    }
    let mut edits: Vec<_> = renames
        .iter()
        .filter(|(span, _)| {
            !declarations.iter().any(|outer| contains(*outer, *span))
                && !captured.iter().any(|(_, declarations, _, _)| {
                    declarations
                        .iter()
                        .any(|(outer, _)| contains(*outer, *span))
                })
        })
        .cloned()
        .collect();
    edits.extend(declarations.into_iter().map(|span| (span, String::new())));
    for (symbol, declarations, name, queries) in captured {
        let parameters = queries
            .iter()
            .map(|(_, _, parameter)| parameter.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let arguments = queries
            .iter()
            .map(|(_, alias, _)| alias.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let mut ordered = declarations.clone();
        ordered.sort_by_key(|(span, _)| span.start);
        for (index, (span, id)) in ordered.iter().enumerate() {
            let mut local = String::new();
            for (query, alias, _) in queries
                .iter()
                .filter(|(query, _, _)| contains(*span, *query))
            {
                local.push_str(&format!(
                    "type {alias} = {};\n",
                    &source[query.start as usize..query.end as usize]
                ));
            }
            if index == 0 {
                local.push_str(&format!(
                    "type {} = {name}<{arguments}>;",
                    scopes.symbol_name(symbol)
                ));
            }
            edits.push((*span, local));
            // Closed dependency renames are folded into this copied declaration.
            // Keeping them as outer edits would overlap the lexical alias replacement.
            let mut changes: Vec<_> = renames
                .iter()
                .filter(|(edit, _)| {
                    contains(*span, *edit)
                        && !queries.iter().any(|(query, _, _)| contains(*query, *edit))
                })
                .cloned()
                .collect();
            changes.push((*id, format!("{name}<{parameters}>")));
            for (query, _, parameter) in queries
                .iter()
                .filter(|(query, _, _)| contains(*span, *query))
            {
                changes.push((*query, parameter.clone()));
            }
            for node in nodes.iter() {
                let AstKind::IdentifierReference(reference) = node.kind() else {
                    continue;
                };
                if !contains(*span, reference.span) {
                    continue;
                }
                if scopes
                    .get_reference(reference.reference_id.get().ok_or_else(fail)?)
                    .symbol_id()
                    == Some(symbol)
                {
                    changes.push((reference.span, format!("{name}<{parameters}>")));
                }
            }
            changes.sort_by_key(|(span, _)| span.start);
            let mut text = source[span.start as usize..span.end as usize].to_owned();
            for (edit, replacement) in changes.into_iter().rev() {
                text.replace_range(
                    (edit.start - span.start) as usize..(edit.end - span.start) as usize,
                    &replacement,
                );
            }
            lifted.push('\n');
            lifted.push_str(&text);
        }
    }
    edits.sort_by_key(|(span, _)| span.start);
    let mut output = source.to_owned();
    for (span, text) in edits.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, &text);
    }
    // Appending avoids disturbing hashbangs, directives, imports, and ESM hoisting.
    output.push_str(&lifted);
    Ok(output)
}

fn allocate(
    base: &str,
    source: &str,
    used: &mut HashSet<String>,
    fresh: &mut impl FnMut(&str) -> String,
) -> String {
    let mut name = fresh(base);
    // Independently reserve decoded source names and every generated family.
    while used.contains(&name) || source.contains(&name) {
        name.push('_');
    }
    used.insert(name.clone());
    name
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(source: &str) -> String {
        transform(source, Path::new("Types.ts"), |_| "Lifted".into()).unwrap()
    }
    #[test]
    fn recursive_generic_and_merged_interfaces_move_together() {
        let out=lower("function C(){interface Tree<T>{value:T;children:Tree<T>[]};interface Tree<T>{extra?:T};type Props=Tree<string>;return null as unknown as Props}");
        assert!(out.contains("type Props=Lifted<string>"));
        assert_eq!(out.matches("interface Lifted<T>").count(), 2);
        assert!(out.contains("children:Lifted<T>[]"));
        assert!(out.find("interface Lifted").unwrap() > out.find("return").unwrap());
    }
    #[test]
    fn mutual_recursion_and_shadowed_symbols_remain_distinct() {
        let out=lower("interface Node{module:true}function A(){interface Node{next:Other};interface Other{next:Node};return null as unknown as Node}function B(){interface Node{other:true};return null as unknown as Node}");
        assert!(out.starts_with("interface Node{module:true}"));
        assert!(out.contains("interface Lifted{next:Lifted_}"));
        assert!(out.contains("interface Lifted_{next:Lifted}"));
        assert!(out.contains("interface Lifted__{other:true}"));
    }
    #[test]
    fn captured_local_types_values_and_classes_stay_lexical() {
        for source in [
            "function C(){type Local=string;interface P{value:Local};return null as unknown as P}",
            "function C(){class Local{};interface P{value:Local};return null as unknown as P}",
            "function C(){class P{};interface P{value:string};return null as unknown as P}",
        ] {
            assert_eq!(lower(source), source);
        }
    }
    #[test]
    fn decoded_names_and_interface_type_parameters_are_reserved() {
        let out = lower(
            r"function C(){const \u004cifted=1;interface P<Lifted_>{value:Lifted_};return null as unknown as P<string>}",
        );
        assert!(out.contains("interface Lifted__<Lifted_>"));
    }
    fn capture(source: &str) -> String {
        transform(source, Path::new("Capture.ts"), str::to_owned).unwrap()
    }
    #[test]
    fn captures_direct_values_properties_and_self_recursion_without_runtime_reads() {
        let source = "function C(){const model={kind:'local' as const};interface P{kind:typeof model.kind;children:P[]};return null as unknown as P}";
        let out = capture(source);
        assert!(out.contains("type __crepusSvelteCapture = typeof model.kind;"));
        assert!(out.contains("type P = __crepusSvelteInterface<__crepusSvelteCapture>;"));
        assert!(out.contains("interface __crepusSvelteInterface<__crepusSvelteCaptureType>{kind:__crepusSvelteCaptureType;children:__crepusSvelteInterface<__crepusSvelteCaptureType>[]}"));
        assert_eq!(out.matches("typeof model.kind").count(), 1);
    }
    #[test]
    fn merged_queries_keep_their_original_flow_locations() {
        let out=capture("function C(){let value:string|number='first';interface P{first:typeof value};value=2;interface P{second:typeof value};return null as unknown as P}");
        let first = out
            .find("type __crepusSvelteCapture = typeof value;")
            .unwrap();
        let mutation = out.find("value=2").unwrap();
        let second = out
            .find("type __crepusSvelteCapture_ = typeof value;")
            .unwrap();
        assert!(first < mutation && mutation < second);
        assert_eq!(out.matches("type P =").count(), 1);
        assert_eq!(out.matches("interface __crepusSvelteInterface<").count(), 2);
    }
    #[test]
    fn unsupported_capture_graphs_remain_lexical() {
        for source in [
            "function C(){const x='x';interface P<T>{value:typeof x;extra:T};return null as unknown as P<string>}",
            "function C(){const x='x';interface A{value:typeof x};interface B{next:A};return null as unknown as B}",
            "function C(){type Local=string;const x=1;interface P{value:typeof x;other:Local};return null as unknown as P}",
            "function C(){class Local{};interface P{value:typeof Local};return null as unknown as P}",
        ] {
            let out=capture(source);
            // A captured A can lift independently, but B must retain its lexical A.
            if source.contains("interface B") {assert!(out.contains("interface B{next:A}"));assert!(!out.contains("interface __crepusSvelteInterface_"));}
            else {assert_eq!(out,source);}
        }
    }

    #[test]
    fn implicit_arguments_stays_in_its_function_scope() {
        for source in [
            "function C(){interface P{args:typeof arguments};return null as unknown as P}",
            "function C(){const x=1;interface P{args:typeof arguments;value:typeof x};return null as unknown as P}",
        ] {
            assert_eq!(capture(source), source);
        }
    }

    #[test]
    fn signature_queries_keep_their_own_flow_context() {
        for member in [
            "read():typeof value",
            "():typeof value",
            "new():typeof value",
            "read:()=>typeof value",
            "read:new()=>typeof value",
            "[key:string]:typeof value",
        ] {
            let source = format!("function C(){{let value:string|number='first';interface P{{{member}}};return null as unknown as P}}");
            assert_eq!(capture(&source), source);
        }
    }

    #[test]
    fn module_variable_queries_keep_the_function_flow_context() {
        let out = capture("let value:string|number='first';function C(){interface P{value:typeof value};return null as unknown as P}");
        assert!(out.contains("function C(){type __crepusSvelteCapture = typeof value;"));
        assert!(out.contains("interface __crepusSvelteInterface<__crepusSvelteCaptureType>{value:__crepusSvelteCaptureType}"));
    }
    #[test]
    fn captured_interfaces_can_extend_and_reference_closed_generic_interfaces() {
        let source="function C(){interface Base<T>{base:T};const label='local' as const;interface Props extends Base<string>{label:typeof label;member:Base<number>;children:Props[]};return null as unknown as Props}";
        let out = capture(source);
        assert!(out.contains("type Props = __crepusSvelteInterface_<__crepusSvelteCapture>;"));
        assert!(out.contains("interface __crepusSvelteInterface<"));
        assert!(out.contains("interface __crepusSvelteInterface_<__crepusSvelteCaptureType> extends __crepusSvelteInterface<string>"));
        assert!(out.contains("member:__crepusSvelteInterface<number>"));
        assert!(out.contains("children:__crepusSvelteInterface_<__crepusSvelteCaptureType>[]"));
        let allocator = Allocator::default();
        assert!(Parser::new(&allocator, &out, SourceType::tsx())
            .parse()
            .errors
            .is_empty());
    }
    #[test]
    fn dependent_capture_aliases_are_not_mistaken_for_closed_interfaces() {
        let source="function C(){const x='x';interface Base{value:typeof x};interface Props extends Base{other:typeof x};return null as unknown as Props}";
        let out = capture(source);
        assert!(out.contains("interface Props extends Base{other:typeof x}"));
        assert!(out.contains("type Base = __crepusSvelteInterface<"));
        assert!(!out.contains("type Props ="));
    }
}
