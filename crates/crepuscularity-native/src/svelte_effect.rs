//! Lower effect registrations before the shared rune reference transformation.
use crate::{svelte_script::has_runtime_binding, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::{collections::HashSet, path::Path};

#[cfg(test)]
pub(crate) fn transform_effects(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    transform_effect_context(
        source,
        path,
        target,
        false,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn transform_effects_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    transform_effect_context(source, path, target, false, policy)
}

pub(crate) fn transform_companion_effects(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    transform_effect_context(
        source,
        path,
        target,
        true,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

fn transform_effect_context(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    companion: bool,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = if companion {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("js"))
        {
            SourceType::mjs()
        } else {
            SourceType::ts()
        }
    } else {
        SourceType::tsx()
    };
    let mut parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("{}: invalid Svelte effect syntax", path.display()));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!("{}: invalid Svelte effect scope", path.display()));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let occupied = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.as_str()),
            AstKind::IdentifierReference(id) => Some(id.name.as_str()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut alias = "__crepusSvelteEffect".to_owned();
    while source.contains(&alias) || occupied.iter().any(|id| id.starts_with(&alias)) {
        alias.push('_');
    }
    let mut tracking_alias = "__crepusSvelteTracking".to_owned();
    while source.contains(&tracking_alias)
        || occupied.iter().any(|id| id.starts_with(&tracking_alias))
    {
        tracking_alias.push('_');
    }
    let mut root_alias = "__crepusSvelteEffectRoot".to_owned();
    while source.contains(&root_alias) || occupied.iter().any(|id| id.starts_with(&root_alias)) {
        root_alias.push('_');
    }
    let mut pre_alias = "__crepusSveltePreEffect".to_owned();
    while source.contains(&pre_alias) || occupied.iter().any(|id| id.starts_with(&pre_alias)) {
        pre_alias.push('_');
    }
    let mut pre_used = false;
    let mut root_used = false;
    let mut effect_used = false;
    let mut tracking_used = false;
    let mut edits = vec![];
    let mut admitted = HashSet::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let root_member = match call.callee.get_inner_expression() {
            Expression::StaticMemberExpression(member) => Some(member.as_ref()),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => Some(member.as_ref()),
                _ => None,
            },
            _ => None,
        };
        if let Some(member) = root_member.filter(|member| member.property.name == "root") {
            if let Expression::Identifier(callee) = member.object.get_inner_expression() {
                if callee.name == "$effect" && !has_runtime_binding(&semantic, callee) {
                    if companion {
                        return Err(format!(
                            "{}: companion modules support only ordinary $effect(callback) registrations",
                            path.display()
                        ));
                    }
                    if call.arguments.len() != 1
                        || matches!(call.arguments.first(), Some(Argument::SpreadElement(_)))
                    {
                        return Err(format!(
                            "{}: $effect.root requires exactly one non-spread callback argument",
                            path.display()
                        ));
                    }
                    admitted.insert(callee.span.start);
                    match target {
                        MoonshineModuleTarget::Dom => {
                            root_used = true;
                            let start = call.callee.span().start;
                            let end = call.arguments[0].span().start;
                            let comments = parsed
                                .program
                                .comments
                                .iter()
                                .filter(|comment| {
                                    comment.span.start >= start && comment.span.end <= end
                                })
                                .map(|comment| {
                                    format!(
                                        "{}\n",
                                        &source[comment.span.start as usize
                                            ..comment.span.end as usize]
                                    )
                                })
                                .collect::<String>();
                            edits.push((
                                start as usize,
                                end as usize,
                                format!("{root_alias}({comments}"),
                            ));
                        }
                        MoonshineModuleTarget::Ssr => edits.push((
                            call.span.start as usize,
                            call.span.end as usize,
                            "(()=>{})".into(),
                        )),
                    }
                    continue;
                }
            }
        }
        let (callee, tracking, pre, member_optional) = match call.callee.get_inner_expression() {
            Expression::Identifier(callee) => (callee, false, false, false),
            Expression::StaticMemberExpression(member)
                if member.property.name == "tracking" || member.property.name == "pre" =>
            {
                let Expression::Identifier(callee) = member.object.get_inner_expression() else {
                    continue;
                };
                (
                    callee,
                    member.property.name == "tracking",
                    member.property.name == "pre",
                    member.optional,
                )
            }
            _ => continue,
        };
        if callee.name != "$effect" || has_runtime_binding(&semantic, callee) {
            continue;
        }
        if companion && (tracking || pre) {
            return Err(format!(
                "{}: companion modules support only ordinary $effect(callback) registrations",
                path.display()
            ));
        }
        let callee_span = call.callee.span();
        let replace_callee = |alias: &str| {
            let comments = parsed
                .program
                .comments
                .iter()
                .filter(|comment| {
                    comment.span.start >= callee_span.start && comment.span.end <= callee_span.end
                })
                .map(|comment| {
                    format!(
                        "{}\n",
                        &source[comment.span.start as usize..comment.span.end as usize]
                    )
                })
                .collect::<String>();
            if comments.is_empty() {
                alias.to_owned()
            } else {
                format!("({comments}{alias})")
            }
        };
        if tracking {
            if call.optional
                || member_optional
                || call.type_arguments.is_some()
                || !call.arguments.is_empty()
            {
                return Err(format!(
                    "{}: $effect.tracking() requires zero arguments without optional calls or generics",
                    path.display()
                ));
            }
            admitted.insert(callee.span.start);
            let (span, replacement) = match target {
                MoonshineModuleTarget::Dom => {
                    tracking_used = true;
                    (callee_span, replace_callee(&tracking_alias))
                }
                MoonshineModuleTarget::Ssr => (call.span, replace_callee("false")),
            };
            edits.push((span.start as usize, span.end as usize, replacement));
            continue;
        }
        let mut ancestors = nodes.ancestors(node.id());
        let statement = loop {
            match ancestors.next().map(|parent| parent.kind()) {
                Some(
                    AstKind::ParenthesizedExpression(_)
                    | AstKind::TSAsExpression(_)
                    | AstKind::TSSatisfiesExpression(_)
                    | AstKind::TSNonNullExpression(_)
                    | AstKind::TSTypeAssertion(_),
                ) => continue,
                Some(AstKind::ExpressionStatement(statement)) => break statement,
                _ => {
                    return Err(format!(
                        "{}: $effect must be a standalone expression statement",
                        path.display()
                    ));
                }
            }
        };
        // Oxc represents expression-bodied arrows using a synthetic expression statement.
        if nodes.ancestors(node.id()).any(|parent| {
            matches!(parent.kind(), AstKind::ArrowFunctionExpression(arrow)
                if arrow.get_expression().is_some_and(|expression| expression.span() == statement.expression.span()))
        }) {
            return Err(format!(
                "{}: $effect must be a standalone expression statement",
                path.display()
            ));
        }
        let nearest_function =
            nodes
                .ancestors(node.id())
                .find_map(|ancestor| match ancestor.kind() {
                    AstKind::Function(function) => Some(!function.r#async && !function.generator),
                    AstKind::ArrowFunctionExpression(function) => Some(!function.r#async),
                    _ => None,
                });
        if nearest_function.is_none() {
            return Err(format!(
                "{}: module initialization cannot register $effect; register during component initialization or an active effect",
                path.display()
            ));
        }
        if companion && nearest_function == Some(false) {
            return Err(format!(
                "{}: companion $effect registration requires a synchronous non-generator function",
                path.display()
            ));
        }
        if call.optional
            || member_optional
            || call.type_arguments.is_some()
            || call.arguments.len() != 1
            || matches!(call.arguments.first(), Some(Argument::SpreadElement(_)))
        {
            return Err(format!(
                "{}: $effect requires exactly one callback without optional calls, generics, or spread arguments",
                path.display()
            ));
        }
        admitted.insert(callee.span.start);
        let (span, replacement) = match target {
            MoonshineModuleTarget::Dom => {
                if pre {
                    pre_used = true;
                    (callee_span, replace_callee(&pre_alias))
                } else {
                    effect_used = true;
                    (callee_span, replace_callee(&alias))
                }
            }
            // Server builds must not evaluate callback factories or captures.
            MoonshineModuleTarget::Ssr => (statement.span, ";".into()),
        };
        edits.push((span.start as usize, span.end as usize, replacement));
    }
    for node in nodes.iter() {
        if let AstKind::IdentifierReference(id) = node.kind() {
            let reference = semantic
                .scoping()
                .get_reference(id.reference_id.get().unwrap());
            if reference.is_type() || reference.flags().is_value_as_type() {
                continue;
            }
            if id.name == "$effect"
                && !has_runtime_binding(&semantic, id)
                && !admitted.contains(&id.span.start)
            {
                let forms = if companion {
                    "companion modules support only ordinary $effect(callback) registrations"
                } else {
                    "unsupported $effect variant or reference; supported forms are $effect(callback), $effect.pre(callback), $effect.tracking(), and $effect.root(callback)"
                };
                return Err(format!("{}: {forms}", path.display()));
            }
        }
    }
    if edits.is_empty() {
        return Ok(source.to_owned());
    }
    edits.sort_by_key(|(start, end, _)| (*start, std::cmp::Reverse(*end)));
    // An outer SSR registration already removes any nested registrations.
    let mut disjoint = vec![];
    let mut previous_end = 0;
    for edit in edits {
        if edit.0 >= previous_end {
            previous_end = edit.1;
            disjoint.push(edit);
        }
    }
    let mut output = source.to_owned();
    for (start, end, replacement) in disjoint.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    let mut imports = Vec::new();
    if root_used {
        imports.push(format!("createRuneRoot as {root_alias}"));
    }
    if effect_used {
        imports.push(format!("createRuneEffect as {alias}"));
    }
    if tracking_used {
        imports.push(format!("runeTracking as {tracking_alias}"));
    }
    if !imports.is_empty() {
        output = format!(
            "import {{ {} }} from \"@tschk/moonshine-svelte/runes\";\n{output}",
            imports.join(", ")
        );
    }
    if pre_used {
        output = format!(
            "import {{createRunePreEffect as {pre_alias}}} from \"@tschk/moonshine-svelte/pre\";\n{output}"
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effect_helpers_reserve_decoded_identifiers() {
        let source = r"export default function Component(){const \u005f_crepusSvelteEffect=()=>{};const \u005f_crepusSveltePreEffect=()=>{};const \u005f_crepusSvelteEffectRoot=()=>{};const \u005f_crepusSvelteTracking=()=>false;$effect(()=>{});$effect.pre(()=>{});const stop=$effect.root(()=>{});const active=$effect.tracking();return <p/>}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        for alias in [
            "__crepusSvelteEffect_",
            "__crepusSveltePreEffect_",
            "__crepusSvelteEffectRoot_",
            "__crepusSvelteTracking_",
        ] {
            assert!(output.contains(&format!("as {alias}")), "{output}");
        }
    }
    #[test]
    fn expression_body_effects_are_not_standalone_statements() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "const make=()=> $effect(()=>{});",
                "const make=()=> ($effect.pre(()=>{}));",
            ] {
                assert!(transform_effects(source, Path::new("Effect.svelte"), target).is_err());
            }
            assert!(transform_effects(
                "const make=()=> invoke(()=>{$effect(()=>{});});",
                Path::new("Effect.svelte"),
                target
            )
            .is_ok());
        }
    }

    fn lower(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        transform_effects(source, Path::new("Counter.svelte"), target).map(|output| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
            output
        })
    }
    #[test]
    fn registrations_in_component_helpers_and_nested_effects_are_rewritten() {
        let source = "export default function Component(){function start(){$effect(()=>count)};start();$effect(()=>{$effect(()=>nested);return()=>cleanup()})}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(output.matches("__crepusSvelteEffect(").count(), 3);
        assert!(!output.contains("$effect("));
        assert!(output.contains("createRuneEffect as __crepusSvelteEffect"));
    }
    #[test]
    fn server_erases_entire_registrations_without_running_factories() {
        let source = "export default function Component(){ $effect(makeCallback());$effect(()=>{$effect(nested());});function helper(){$effect(makeNested())} return <p/>; }";
        let output = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!output.contains("makeCallback"));
        assert!(!output.contains("makeNested"));
        assert!(!output.contains("nested()"));
        assert!(!output.contains("moonshine-svelte/runes"));
        assert!(output.contains("return <p/>"));
    }
    #[test]
    fn parenthesized_and_type_wrapped_standalone_effects_are_supported() {
        let source =
            "export default function C(){(($effect(cb)));($effect(factory()) as void);return <p/>}";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(dom.matches("__crepusSvelteEffect(").count(), 2);
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!ssr.contains("factory()"));
        assert!(!ssr.contains("cb"));
        assert!(ssr.contains("return <p/>"));
    }
    #[test]
    fn ordinary_shadowed_calls_and_helper_names_remain_intact() {
        let source = "export const __crepusSvelteEffect=1;function helper($effect){$effect(cb)};export default function Component(){$effect(cb)}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("helper($effect){$effect(cb)}"));
        assert!(output.contains("__crepusSvelteEffect_(cb)"));
        assert_eq!(
            lower(
                "function helper($effect){$effect(cb)}",
                MoonshineModuleTarget::Ssr
            )
            .unwrap(),
            "function helper($effect){$effect(cb)}"
        );
    }
    #[test]
    fn tracking_queries_support_module_and_nested_expression_contexts() {
        let source = "export const active=$effect.tracking();export default function C(){const read=()=> $effect.tracking();return <p>{$effect.tracking()?'yes':'no'}</p>}";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(dom.matches("__crepusSvelteTracking()").count(), 3);
        assert!(dom.contains("runeTracking as __crepusSvelteTracking"));
        assert!(!dom.contains("createRuneEffect"));
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(ssr.contains("active=false"));
        assert!(ssr.contains("()=> false"));
        assert!(ssr.contains("{false?'yes':'no'}"));
        assert!(!ssr.contains("import"));
    }
    #[test]
    fn tracking_shadowing_hygiene_and_erased_nested_registrations() {
        let source = "const __crepusSvelteTracking=1;function local($effect){return $effect.tracking()}export default function C(){$effect(()=>console.log($effect.tracking()));return $effect.tracking()}";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("return $effect.tracking()"));
        assert!(dom.contains("runeTracking as __crepusSvelteTracking_"));
        assert!(dom.contains("createRuneEffect as __crepusSvelteEffect"));
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!ssr.contains("console.log"));
        assert!(ssr.contains("function C(){;return false}"));
        assert!(ssr.contains("return $effect.tracking()"));
    }
    #[test]
    fn roots_are_expressions_and_normalize_optional_generic_and_wrapped_callees() {
        for call in [
            "$effect.root(factory())",
            "$effect?.root(factory())",
            "$effect.root?.(factory())",
            "$effect?.root?.<void>(factory())",
            "($effect.root as any)(factory())",
            "(($effect.root))(factory())",
            "($effect).root(factory())",
            "($effect.root!)(factory())",
            "($effect?.root)(factory())",
        ] {
            let source =
                format!("export const stop={call};export function start(){{return {call}}}");
            let dom = lower(&source, MoonshineModuleTarget::Dom).unwrap();
            assert_eq!(
                dom.matches("__crepusSvelteEffectRoot(factory())").count(),
                2,
                "{call}: {dom}"
            );
            assert!(dom.contains("createRuneRoot as __crepusSvelteEffectRoot"));
            assert!(!dom.contains("createRuneEffect"));
            let ssr = lower(&source, MoonshineModuleTarget::Ssr).unwrap();
            assert!(!ssr.contains("factory()"));
            assert!(!ssr.contains("import"));
            assert_eq!(ssr.matches("(()=>{})").count(), 2);
        }
    }
    #[test]
    fn root_server_erasure_coalesces_nested_effects_and_preserves_immediate_use() {
        let source = "const stop=$effect.root(makeFactory($effect.root(()=>{$effect(()=>work());return()=>cleanup()})));function C(){$effect(()=>{$effect.root(factory());});return $effect.root(factory())()}";
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!ssr.contains("factory"));
        assert!(!ssr.contains("Factory"));
        assert!(!ssr.contains("cleanup"));
        assert!(!ssr.contains("work"));
        assert!(ssr.contains("return (()=>{})()"));
        let dom=lower("const __crepusSvelteEffectRoot=0;const stop=$effect.root(/* keep */()=>{$effect(()=>work())});function local($effect){return $effect.root(cb)}",MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("__crepusSvelteEffectRoot_(/* keep */"));
        assert!(dom.contains("return $effect.root(cb)"));
        assert!(dom.contains("createRuneEffect as __crepusSvelteEffect"));
    }
    #[test]
    fn invalid_root_arity_spreads_computed_and_references_fail() {
        for source in [
            "$effect.root()",
            "$effect.root(a,b)",
            "$effect.root(...callbacks)",
            "$effect['root'](callback)",
            "const root=$effect.root",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn invalid_tracking_queries_are_rejected() {
        for source in [
            "$effect.tracking(1)",
            "$effect.tracking(...args)",
            "$effect?.tracking()",
            "$effect.tracking?.()",
            "$effect.tracking<boolean>()",
            "const f=$effect.tracking",
            "$effect['tracking']()",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn invalid_placements_and_unimplemented_variants_fail_for_both_hosts() {
        for source in [
            "$effect(cb);",
            "function C(){const x=$effect(cb)}",
            "function C(){$effect()}",
            "function C(){$effect(a,b)}",
            "function C(){$effect(...callbacks)}",
            "function C(){$effect?.(cb)}",
            "function C(){$effect.pre.call(null,cb)}",
            "function C(){const f=$effect}",
            "function C(){$effect(()=>{$effect.pending()})}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn pre_registrations_use_dedicated_helper_and_server_erases_factories() {
        let source = "export default function Component(){($effect.pre(factory()));$effect(()=>{});return <p/>}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("createRunePreEffect as __crepusSveltePreEffect"));
        assert!(output.contains("from \"@tschk/moonshine-svelte/pre\""));
        assert!(output.contains("(__crepusSveltePreEffect(factory()))"));
        assert!(output.contains("createRuneEffect as"));
        let server = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!server.contains("factory()"));
        assert!(!server.contains("$effect"));
        assert!(!server.contains("moonshine-svelte/pre"));
    }
    #[test]
    fn pre_scope_arity_and_variant_validation_match_effect_registrations() {
        for source in [
            "$effect.pre(()=>{})",
            "function C(){const value=$effect.pre(()=>{})}",
            "function C(){$effect.pre()}",
            "function C(){$effect.pre(a,b)}",
            "function C(){$effect.pre(...args)}",
            "function C(){$effect.pre?.(()=>{})}",
            "function C(){$effect?.pre(()=>{})}",
            "function C(){$effect.pre<number>(()=>{})}",
            "function C(){const value=$effect.pre}",
            "function C(){$effect['pre'](()=>{})}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn pre_shadowing_hygiene_and_nested_server_erasure() {
        let source = "const __crepusSveltePreEffect=0;function ordinary($effect){$effect.pre(factory())}export default function C(){$effect.pre(()=>{$effect.root(()=>{$effect.pre(()=>{})});$effect.tracking()})}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("createRunePreEffect as __crepusSveltePreEffect_"));
        assert!(output.contains("function ordinary($effect){$effect.pre(factory())}"));
        let server = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(server.contains("function ordinary($effect){$effect.pre(factory())}"));
        assert!(server.contains("export default function C(){;}"));
    }
    #[test]
    fn transparent_effect_pre_and_tracking_callees_lower_for_both_targets() {
        let source = "export default function C(){($effect)(()=>{});($effect.pre)(()=>{});($effect.pre as any)(()=>{});($effect.pre!)(()=>{});(($effect as any).pre)(()=>{});($effect).pre(()=>{});($effect.pre satisfies Function)(()=>{});const tracked=($effect.tracking as any)();}";
        let output = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("__crepusSvelteEffect(()=>{})"));
        assert_eq!(output.matches("__crepusSveltePreEffect(()=>{})").count(), 6);
        assert!(output.contains("const tracked=__crepusSvelteTracking()"));
        assert!(!output.contains("$effect"));
        let server = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!server.contains("$effect"));
        assert!(server.contains("const tracked=false"));
    }
    #[test]
    fn callee_comments_survive_replacement_with_safe_line_breaks() {
        let output=lower("const __crepusSveltePreEffect=0;function C(){($effect /* keep */ . /* phase */ pre)(()=>{});($effect // line\n .pre as any)(()=>{});const tracked=($effect /* query */ .tracking)();}",MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("/* keep */\n/* phase */\n__crepusSveltePreEffect_)("));
        assert!(output.contains("// line\n__crepusSveltePreEffect_)("));
        assert!(output.contains("/* query */\n__crepusSvelteTracking)()"));
        assert!(!output.contains("$effect"));
    }
    #[test]
    fn wrapped_callee_shadowing_and_existing_invalid_variants_remain_exact() {
        let source = "function ordinary($effect){($effect.pre as any)(factory());($effect)(factory());($effect.tracking)()}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert_eq!(lower(source, target).unwrap(), source);
        }
        for source in [
            "function C(){($effect.pre)?.(()=>{})}",
            "function C(){($effect.pre)<number>(()=>{})}",
            "function C(){($effect.pre)(...args)}",
            "($effect.pre)(()=>{})",
            "function C(){const bad=($effect.pre)(()=>{})}",
            "function C(){($effect.tracking)(1)}",
            "function C(){($effect.tracking)?.()}",
            "function C(){($effect.pre['call'])(null,()=>{})}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn grouped_callee_comments_do_not_trigger_return_or_throw_asi() {
        let source = "function returns(){return ($effect /* block */ .tracking)()}function throws(){throw ($effect // line\n .tracking)()}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = lower(source, target).unwrap();
            assert!(output.contains("return (/* block */\n"), "{output}");
            assert!(output.contains("throw (// line\n"), "{output}");
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{output}");
            let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
            let returns = semantic
                .nodes()
                .iter()
                .filter_map(|node| match node.kind() {
                    AstKind::ReturnStatement(statement) => Some(statement),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 1);
            assert!(returns[0].argument.is_some(), "{output}");
            if matches!(target, MoonshineModuleTarget::Ssr) {
                assert!(output.contains("/* block */\nfalse)"));
                assert!(output.contains("// line\nfalse)"));
            }
        }
    }
}

#[cfg(test)]
mod companion_tests {
    use super::*;

    fn lower(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        transform_companion_effects(source, Path::new("factory.svelte.ts"), target).map(|output| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::ts()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
            output
        })
    }

    #[test]
    fn synchronous_factories_generic_arrows_and_nested_registrations_lower() {
        let source = "export function make(){$effect(factory());$effect(()=>{$effect(nested());});function start(){$effect(other())}}export const generic=<T>(value:T)=>{$effect(()=>value);return value};";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("createRuneEffect as __crepusSvelteEffect"));
        assert_eq!(dom.matches("__crepusSvelteEffect(").count(), 5);
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        for expression in [
            "factory()",
            "nested()",
            "other()",
            "$effect(",
            "createRuneEffect",
        ] {
            assert!(!ssr.contains(expression), "{ssr}");
        }
        assert!(ssr.contains("return value"));
    }

    #[test]
    fn ambient_signatures_wrapped_callees_and_comments_do_not_hide_effects() {
        let source = "declare const $effect:(cb:()=>void)=>void;export function make(){($effect as (cb:()=>void)=>void)(factory());($effect!)(other());($effect /* comment */)(()=>1);}";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(dom.matches("__crepusSvelteEffect").count(), 4);
        assert!(dom.contains("/* comment */"));
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!ssr.contains("factory()"));
        assert!(!ssr.contains("other()"));
        assert!(!ssr.contains("createRuneEffect"));
        assert!(ssr.contains("declare const $effect"));
    }

    #[test]
    fn runtime_functions_overloads_imports_and_type_only_references_remain_ordinary() {
        for source in [
            "function $effect(cb:()=>void):void;function $effect(cb:any){cb()}export function make(){$effect(factory())}",
            "import{$effect}from'./helpers';export const make=()=>{$effect(factory())};",
            "export function make($effect:(cb:()=>void)=>void){$effect(factory())}",
            "export type Effect=typeof $effect;",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert_eq!(lower(source, target).unwrap(), source);
            }
        }
    }

    #[test]
    fn unsupported_variants_and_registration_positions_remain_hard_errors() {
        for source in [
            "$effect(()=>{});export const value=1;",
            "export async function make(){$effect(()=>{})}",
            "export function* make(){$effect(()=>{})}",
            "export const make=async()=>{$effect(()=>{})};",
            "export const make=()=> $effect(()=>{});",
            "export function make(){const result=$effect(()=>{})}",
            "export function make(){$effect.pre(()=>{})}",
            "export function make(){$effect.tracking()}",
            "export function make(){$effect.root(()=>{})}",
            "export function make(){$effect.unknown(()=>{})}",
            "export function make(){$effect?.(()=>{})}",
            "export function make(){$effect<void>(()=>{})}",
            "export function make(){$effect(...callbacks)}",
            "export function make(){const effect=$effect}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = lower(source, target).unwrap_err();
                assert!(error.contains("factory.svelte.ts"), "{source}: {error}");
            }
        }
    }

    #[test]
    fn nearest_function_controls_registration_and_component_variants_stay_supported() {
        let source = "export async function outer(){function sync(){$effect(()=>1)}}";
        assert!(lower(source, MoonshineModuleTarget::Dom)
            .unwrap()
            .contains("__crepusSvelteEffect("));
        let component = "declare const $effect:{(cb:()=>void):void;pre(cb:()=>void):void;tracking():boolean;root(cb:()=>void):()=>void};export default function C(){$effect(factory());$effect.pre(preFactory());const tracked=$effect.tracking();const stop=$effect.root(rootFactory());return <p>{tracked}</p>}";
        let dom = transform_effects(component, Path::new("C.svelte"), MoonshineModuleTarget::Dom)
            .unwrap();
        for helper in [
            "createRuneEffect",
            "createRunePreEffect",
            "runeTracking",
            "createRuneRoot",
        ] {
            assert!(dom.contains(helper), "{dom}");
        }
        let ssr = transform_effects(component, Path::new("C.svelte"), MoonshineModuleTarget::Ssr)
            .unwrap();
        for factory in ["factory()", "preFactory()", "rootFactory()"] {
            assert!(!ssr.contains(factory), "{ssr}");
        }
        assert!(!ssr.contains("moonshine-svelte/runes"));
    }
}
