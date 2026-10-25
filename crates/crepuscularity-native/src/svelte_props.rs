//! Target-specific destructured Svelte props: live DOM bindings and SSR snapshots.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

type Edit = (usize, usize, String);
fn render(source: &str, span: Span, edits: &[Edit]) -> String {
    let mut text = source[span.start as usize..span.end as usize].to_owned();
    for (start, end, value) in edits
        .iter()
        .rev()
        .filter(|(a, b, _)| *a >= span.start as usize && *b <= span.end as usize)
    {
        text.replace_range(
            start - span.start as usize..end - span.start as usize,
            value,
        );
    }
    text
}

#[cfg(test)]
pub(crate) fn transform_props(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    component_name: &str,
    props_name: &str,
    exported_props: &[&str],
    server_before_commit: Option<&str>,
) -> Result<(String, bool, String), String> {
    transform_props_with_policy(
        source,
        path,
        target,
        (component_name, props_name),
        exported_props,
        server_before_commit,
        (
            crate::svelte_javascript::IrPolicy::Typed,
            &Default::default(),
        ),
    )
}

pub(crate) fn transform_props_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    (component_name, props_name): (&str, &str),
    exported_props: &[&str],
    server_before_commit: Option<&str>,
    (policy, globals): (
        crate::svelte_javascript::IrPolicy,
        &crate::source_globals::SourceGlobals,
    ),
) -> Result<(String, bool, String), String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let fail = |message: &str| format!("{}: {message}", path.display());
    if !parsed.errors.is_empty() {
        return Err(fail("invalid Svelte props syntax"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("invalid Svelte props scope"));
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
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) || occupied.iter().any(|id| id.starts_with(&name)) {
            name.push('_');
        }
        name
    };
    let helper = fresh("__crepusSvelteProp");
    let rest_helper = fresh("__crepusSvelteRest");
    let component_ref = fresh("__crepusPropsComponentRef");
    let untrack = fresh("__crepusPropUntrack");
    let memo = fresh("__crepusPropMemo");
    let get_owner = fresh("__crepusPropGetOwner");
    let run_with_owner = fresh("__crepusPropRunWithOwner");
    let unbound = fresh("__crepusPropUnbound");
    let bindable_helper = fresh("__crepusSvelteBindableProp");
    let create_prop = fresh("__crepusCreateBindableProp");
    let bindable_state = fresh("__crepusBindableState");
    let after_render = fresh("__crepusBindableAfterRender");
    let public_props = fresh("__crepusSveltePublicProps");
    let javascript = policy == crate::svelte_javascript::IrPolicy::JavaScript;
    let source_props = fresh("__crepusSvelteJSProps");
    let original_props_name = props_name;
    let props_name = if javascript {
        source_props.as_str()
    } else {
        props_name
    };
    let mut source_docs = String::new();
    let mut aggregate_annotation = false;
    let mut doc_edits = Vec::new();

    let mut public_type = "{}".to_owned();
    let mut admitted_bindable = HashSet::new();
    let mut unbound_used = false;
    let mut declarations = Vec::new();
    let mut aliases = Vec::new();
    let mut props_declarations = 0usize;
    let mut bindings = HashMap::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(decl) = node.kind() else {
            continue;
        };
        let call = match decl.init.as_ref() {
            Some(Expression::CallExpression(call)) => call.as_ref(),
            Some(Expression::ChainExpression(chain)) => match &chain.expression {
                ChainElement::CallExpression(call) => call.as_ref(),
                _ => continue,
            },
            _ => continue,
        };
        let Expression::Identifier(callee) = call.callee.get_inner_expression() else {
            continue;
        };
        if callee.name != "$props"
            || semantic
                .scoping()
                .get_reference(callee.reference_id.get().unwrap())
                .symbol_id()
                .is_some()
        {
            continue;
        }
        if !matches!(
            decl.id,
            BindingPattern::ObjectPattern(_) | BindingPattern::BindingIdentifier(_)
        ) {
            continue;
        }
        let ancestors: Vec<_> = nodes.ancestors(node.id()).map(|n| n.kind()).collect();
        if ancestors.iter().any(|kind| {
            matches!(
                kind,
                AstKind::ArrowFunctionExpression(_) | AstKind::BlockStatement(_)
            )
        }) || !ancestors
            .iter()
            .find_map(|kind| {
                if let AstKind::Function(f) = kind {
                    Some(*f)
                } else {
                    None
                }
            })
            .is_some_and(|f| f.id.as_ref().is_some_and(|id| id.name == component_name))
        {
            return Err(fail(
                "$props declarations must be directly in the component instance",
            ));
        }
        if call.optional || !call.arguments.is_empty() {
            return Err(fail("$props takes no arguments"));
        }
        props_declarations += 1;
        if props_declarations > 1 {
            return Err(fail("$props may only be declared once per component"));
        }
        if javascript {
            let declaration = ancestors
                .iter()
                .find_map(|kind| {
                    if let AstKind::VariableDeclaration(declaration) = kind {
                        Some(*declaration)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| fail("missing JavaScript props declaration"))?;
            for comment in &parsed.program.comments {
                if comment.is_jsdoc() && comment.attached_to == declaration.span.start {
                    let text = &source[comment.span.start as usize..comment.span.end as usize];
                    if text.contains("@satisfies") {
                        return Err(fail(
                            "unsupported JavaScript JSDoc satisfies on props; use an aggregate type annotation",
                        ));
                    }
                    let typed = text.match_indices("@type").any(|(index, _)| {
                        text[index + 5..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                    });
                    if typed && declaration.declarations.len() != 1 {
                        return Err(fail(
                            "unsupported JavaScript JSDoc props with multiple variable declarators",
                        ));
                    }
                    aggregate_annotation |= typed;
                    source_docs.push_str(text);
                    source_docs.push('\n');
                    doc_edits.push((
                        comment.span.start as usize,
                        comment.span.end as usize,
                        String::new(),
                    ));
                }
            }
        }
        if matches!(decl.id, BindingPattern::BindingIdentifier(_)) {
            if call.type_arguments.is_some() {
                return Err(fail("generic $props identifier aliases are unsupported"));
            }
            public_type = decl
                .type_annotation
                .as_ref()
                .map(|annotation| render(source, annotation.type_annotation.span(), &[]))
                .unwrap_or_else(|| {
                    if javascript {
                        format!("{}.Record<string,unknown>", globals.types)
                    } else {
                        format!("{}.Record<string,any>", globals.types)
                    }
                });
            aliases.push((
                call.span.start as usize,
                call.span.end as usize,
                format!("{rest_helper}({props_name},[])"),
            ));
            continue;
        }
        let BindingPattern::ObjectPattern(pattern) = &decl.id else {
            continue;
        };
        let ty = if let Some(args) = &call.type_arguments {
            if args.params.len() != 1 {
                return Err(fail("$props requires one props type"));
            }
            Some(args.params[0].span())
        } else {
            decl.type_annotation
                .as_ref()
                .map(|a| a.type_annotation.span())
        };
        let mut props = Vec::new();
        for property in &pattern.properties {
            if property.computed {
                return Err(fail("computed $props keys are unsupported"));
            }
            let key = match &property.key {
                PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                PropertyKey::StringLiteral(s) => s.value.to_string(),
                _ => return Err(fail("unsupported $props key")),
            };
            let (binding, mut default) = match &property.value {
                BindingPattern::BindingIdentifier(id) => (id.as_ref(), None),
                BindingPattern::AssignmentPattern(a) => {
                    let BindingPattern::BindingIdentifier(id) = &a.left else {
                        return Err(fail("nested $props patterns are unsupported"));
                    };
                    (id.as_ref(), Some(a.right.span()))
                }
                _ => return Err(fail("nested $props patterns are unsupported")),
            };
            let mut bindable = false;
            let mut bindable_type = None;
            if let BindingPattern::AssignmentPattern(assignment) = &property.value {
                if let Expression::CallExpression(call) = assignment.right.get_inner_expression() {
                    if let Expression::Identifier(callee) = &call.callee {
                        if callee.name == "$bindable"
                            && semantic
                                .scoping()
                                .get_reference(callee.reference_id.get().unwrap())
                                .symbol_id()
                                .is_none()
                        {
                            if call.optional
                                || call.arguments.len() > 1
                                || call
                                    .arguments
                                    .iter()
                                    .any(|argument| matches!(argument, Argument::SpreadElement(_)))
                            {
                                return Err(fail(
                                    "$bindable requires zero or one nonspread argument",
                                ));
                            }
                            if let Some(arguments) = &call.type_arguments {
                                if arguments.params.len() != 1 {
                                    return Err(fail("$bindable requires one type argument"));
                                }
                                bindable_type = Some(arguments.params[0].span());
                            }
                            bindable = true;
                            default = call.arguments.first().map(GetSpan::span);
                            admitted_bindable.insert(callee.span.start);
                        }
                    }
                }
            }
            bindings.insert(
                binding.symbol_id.get().unwrap(),
                decl.kind == VariableDeclarationKind::Const,
            );
            let writable = exported_props.contains(&binding.name.as_str())
                || semantic
                    .symbol_references(binding.symbol_id.get().unwrap())
                    .any(|reference| reference.is_write());
            props.push((
                key,
                binding.name.to_string(),
                default,
                writable,
                bindable,
                bindable_type,
            ));
        }
        let rest = if let Some(rest) = &pattern.rest {
            let BindingPattern::BindingIdentifier(id) = &rest.argument else {
                return Err(fail("nested $props rest patterns are unsupported"));
            };
            Some(id.name.to_string())
        } else {
            None
        };
        if props.is_empty() && rest.is_none() {
            return Err(fail("empty $props destructuring is unsupported"));
        }
        public_type = ty.map(|span| render(source, span, &[])).unwrap_or_else(|| {
            let mut fields = props
                .iter()
                .enumerate()
                .map(|(index, (key, _, _, _, _, bindable_type))| {
                    let optional = matches!(
                        pattern.properties[index].value,
                        BindingPattern::AssignmentPattern(_)
                    );
                    let value_type = if javascript {
                        Some(format!("typeof {}.value", props[index].1))
                    } else {
                        None
                    }
                    .or_else(|| bindable_type.map(|span| render(source, span, &[])))
                    .unwrap_or_else(|| "any".to_owned());
                    format!(
                        "{}{}:{value_type}",
                        serde_json::to_string(key).unwrap(),
                        if optional { "?" } else { "" }
                    )
                })
                .collect::<Vec<_>>();
            if rest.is_some() {
                fields.push(
                    if javascript {
                        "[key:string]:unknown"
                    } else {
                        "[key:string]:any"
                    }
                    .to_owned(),
                );
            }
            format!("{{{}}}", fields.join(";"))
        });
        declarations.push((decl.span, props, rest, ty));
    }
    for node in nodes.iter() {
        if let AstKind::IdentifierReference(id) = node.kind() {
            let reference = semantic
                .scoping()
                .get_reference(id.reference_id.get().unwrap());
            if id.name == "$bindable"
                && reference.symbol_id().is_none()
                && !admitted_bindable.contains(&id.span.start)
            {
                return Err(fail(
                    "$bindable is only supported as a direct $props default",
                ));
            }
        }
    }
    let body = parsed
        .program
        .body
        .iter()
        .find_map(|statement| {
            let Statement::ExportDefaultDeclaration(export) = statement else {
                return None;
            };
            let ExportDefaultDeclarationKind::FunctionDeclaration(function) = &export.declaration
            else {
                return None;
            };
            function
                .id
                .as_ref()
                .is_some_and(|id| id.name == component_name)
                .then_some(function.body.as_ref())
                .flatten()
        })
        .ok_or_else(|| fail("public props require the named component body"))?;
    if aggregate_annotation {
        public_type = format!("typeof {source_props}");
    }
    let aggregate = if javascript && props_declarations != 0 {
        format!("{source_docs}const {source_props}={original_props_name};\n")
    } else {
        String::new()
    };
    let contract = (
        body.span.start as usize + 1,
        body.span.start as usize + 1,
        format!("\n{aggregate}type {public_props}={public_type};\n"),
    );
    if declarations.is_empty() && aliases.is_empty() {
        let mut output = source.to_owned();
        output.insert_str(contract.0, &contract.2);
        return Ok((output, false, public_props));
    }
    let readonly_dom = |default: &Option<Span>, writable: &bool| {
        matches!(target, MoonshineModuleTarget::Dom) && !*writable && default.is_none()
    };
    let needs_prop_helper = declarations.iter().any(|(_, props, _, _)| {
        props.iter().any(|(_, _, default, writable, bindable, _)| {
            !bindable || readonly_dom(default, writable)
        })
    });
    let needs_bindable_helper = declarations.iter().any(|(_, props, _, _)| {
        props.iter().any(|(_, _, default, writable, bindable, _)| {
            *bindable && !readonly_dom(default, writable)
        })
    });
    let needs_bindable_state = matches!(target, MoonshineModuleTarget::Dom)
        && declarations.iter().any(|(_, props, _, _)| {
            props
                .iter()
                .any(|(_, _, default, _, bindable, _)| *bindable && default.is_some())
        });
    let bindable_names: Vec<_> = declarations
        .iter()
        .flat_map(|(_, props, _, _)| {
            props
                .iter()
                .filter(|(_, _, _, _, bindable, _)| *bindable)
                .map(|(_, name, _, _, _, _)| name)
        })
        .collect();
    let needs_rest_helper =
        !aliases.is_empty() || declarations.iter().any(|(_, _, rest, _)| rest.is_some());
    let mut edits = aliases;
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().unwrap());
        let Some(constant) = reference.symbol_id().and_then(|sym| bindings.get(&sym)) else {
            continue;
        };
        if reference.is_type() {
            return Err(fail("prop binding type queries are unsupported"));
        }
        if *constant && reference.is_write() {
            return Err(fail("assigning to a const prop binding is invalid"));
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
        let value = format!("{}.value", id.name);
        let value = match nodes.parent_kind(node.id()) {
            AstKind::ObjectProperty(p) if p.shorthand => {
                crate::svelte_expression::shorthand_property(id.name.as_str(), &value)
            }
            AstKind::AssignmentTargetPropertyIdentifier(_) => format!("{}: {value}", id.name),
            _ => value,
        };
        edits.push((id.span.start as usize, id.span.end as usize, value));
    }
    let mut finalizer_handled = false;
    if needs_bindable_helper && matches!(target, MoonshineModuleTarget::Ssr) {
        let commit = bindable_names
            .iter()
            .map(|name| format!("{name}.commit();"))
            .collect::<String>();
        for node in nodes.iter() {
            let AstKind::ReturnStatement(statement) = node.kind() else {
                continue;
            };
            let Some(argument) = &statement.argument else {
                continue;
            };
            if !matches!(nodes.parent_kind(node.id()), AstKind::FunctionBody(_)) {
                continue;
            }
            let owner = nodes.ancestors(node.id()).find(|parent| {
                matches!(
                    parent.kind(),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                )
            });
            if !owner.is_some_and(|parent|matches!(parent.kind(),AstKind::Function(function) if function.id.as_ref().is_some_and(|id|id.name==component_name))) {
                continue;
            }
            let span = argument.span();
            finalizer_handled |= server_before_commit.is_some();
            edits.push((
                span.start as usize,
                span.start as usize,
                format!("{after_render}("),
            ));
            edits.push((
                span.end as usize,
                span.end as usize,
                format!(",()=>{{{}{commit}}})", server_before_commit.unwrap_or("")),
            ));
        }
    }
    edits.sort_by_key(|e| (e.0, e.1));
    let mut replacements = Vec::new();
    for (span, props, rest, ty) in &declarations {
        let mut pieces = Vec::new();
        for (key, name, default, writable, bindable, bindable_type) in props {
            let has_default = default.is_some();
            let default = default
                .map(|span| render(source, span, &edits))
                .unwrap_or_else(|| "void 0".into());
            let generic = ty
                .map(|span| {
                    format!(
                        "<({})[{}]>",
                        render(source, span, &[]),
                        serde_json::to_string(key).unwrap()
                    )
                })
                .or_else(|| bindable_type.map(|span| format!("<{}>", render(source, span, &[]))))
                .unwrap_or_default();
            if *bindable
                && !(matches!(target, MoonshineModuleTarget::Dom) && !*writable && !has_default)
            {
                let annotation = bindable_type
                    .filter(|_| has_default)
                    .map(|span| format!(":{}", render(source, span, &[])))
                    .unwrap_or_default();
                let default = if matches!(target, MoonshineModuleTarget::Dom) && has_default {
                    format!("{bindable_state}({default}).value")
                } else {
                    default
                };
                pieces.push(format!("{name}={bindable_helper}{generic}()({props_name},{},(){annotation}=>({default}),{has_default})",serde_json::to_string(key).unwrap()));
                continue;
            }
            pieces.push(format!(
                "{name}={helper}{generic}()({props_name},{},()=>({default}),{writable},{has_default})",
                serde_json::to_string(key).unwrap()
            ));
        }
        if let Some(rest) = rest {
            let keys: Vec<_> = props.iter().map(|(key, _, _, _, _, _)| key).collect();
            pieces.push(format!(
                "{rest}={rest_helper}({props_name},{})",
                serde_json::to_string(&keys).unwrap()
            ));
        }
        replacements.push((
            span.start as usize,
            span.end as usize,
            format!(" {}", pieces.join(",")),
        ));
    }
    edits.retain(|(start, end, _)| {
        !declarations
            .iter()
            .any(|(span, _, _, _)| *start >= span.start as usize && *end <= span.end as usize)
    });
    edits.extend(replacements);
    edits.extend(doc_edits);
    edits.push(contract);
    edits.sort_by_key(|e| (e.0, e.1));
    let mut out = source.to_owned();
    for (start, end, value) in edits.into_iter().rev() {
        out.replace_range(start..end, &value);
    }
    if needs_bindable_state {
        out = format!(
            "import {{createState as {bindable_state}}} from \"@tschk/moonshine-svelte/runes\";\n{out}"
        );
    }
    // Non-undefined F members are already in T; keeping them can add a redundant never[] union.
    if needs_bindable_helper {
        let (runtime, spec) = match target {
            MoonshineModuleTarget::Dom => ("createProp", "@tschk/moonshine-svelte/runes"),
            MoonshineModuleTarget::Ssr => ("createServerProp", "@tschk/moonshine-svelte/ssr"),
        };
        out = format!("import {{{runtime} as {create_prop}}} from {spec:?};\n{out}");
        match target {
            MoonshineModuleTarget::Dom=>out.push_str(&format!(r#"
function {bindable_helper}<T=any>(){{return <F extends T|undefined,D extends boolean>(props:any,key:string,fallback:()=>F,hasDefault:D):{{value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T}}=>{{
 type Value=D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T;
 return {create_prop}<Value>(props,key,hasDefault?{{bindable:true,mode:"dom",fallback:fallback as ()=>Value}}:{{bindable:true,mode:"dom"}});
}};}}
"#)),
            MoonshineModuleTarget::Ssr=>{
                out=format!("import {{afterRender as {after_render}}} from \"@tschk/moonshine-svelte/ssr\";\n{out}");
                out.push_str(&format!(r#"
function {bindable_helper}<T=any>(){{return <F extends T|undefined,D extends boolean>(props:any,key:string,fallback:()=>F,hasDefault:D):{{value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T;commit:()=>void}}=>{{
 type Value=D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T;
 return {create_prop}<Value>(props,key,hasDefault?fallback as ()=>Value:void 0);
}};}}
"#));
            },
        }
    }
    if needs_prop_helper {
        if matches!(target, MoonshineModuleTarget::Dom) {
            let runtime = "@tschk/moonshine-dom/reactive";
            out = format!(
                "import {{untrack as {untrack},getOwner as {get_owner},runWithOwner as {run_with_owner}}} from {runtime:?};\n{out}"
            );
            out = format!(
                "import {{createDerived as {memo}}} from \"@tschk/moonshine-svelte/runes\";\n{out}"
            );
            out.push_str(&format!(r#"
function {helper}<T=any>() {{return <F extends T|undefined,D extends boolean>(props:any,key:PropertyKey,fallback:()=>F,writable:boolean,hasDefault:D):{{value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T}} => {{
 const initial=hasDefault||writable?props[key]:undefined;const owner={get_owner}();let fallbackMemo:{{value:F}}|undefined;
 const getFallback=()=>{{if(!fallbackMemo)fallbackMemo={run_with_owner}(owner,()=>{memo}(fallback));return fallbackMemo.value;}};
 if(hasDefault&&initial===undefined){untrack}(getFallback);
 if(!writable)return {{get value(){{const value=props[key];return value===undefined?getFallback():value;}}}};
 return {run_with_owner}(owner,()=>{memo}<D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T>(()=>{{const value=props[key];return value===undefined?getFallback():value;}}));
}};}}"#));
        } else {
            out.push_str(&format!(r#"
function {helper}<T=any>() {{return <F extends T|undefined,D extends boolean>(props:any,key:PropertyKey,fallback:()=>F,_writable:boolean,_hasDefault:D):{{value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T}} => {{
 const value=props[key];return {{value:value===undefined?fallback():value}};
}};}}"#));
        }
    }
    let realm = &globals.realm;
    if needs_rest_helper {
        if matches!(target, MoonshineModuleTarget::Dom) {
            out = format!(
                "import {{componentRef as {component_ref}}} from \"@tschk/moonshine-dom/component-instance\";\n{out}"
            );
            out.push_str(&format!(r#"
function {rest_helper}(props:any,excluded:string[]):any {{const skip=new {realm}.Set<PropertyKey>([...excluded,"$$slots","$$events","$$legacy",{component_ref}]);return new {realm}.Proxy({{}},{{get:(_,key)=>skip.has(key)?undefined:props[key],has:(_,key)=>!skip.has(key)&&key in props,ownKeys:()=>{realm}.Reflect.ownKeys(props).filter(key=>!skip.has(key)),getOwnPropertyDescriptor:(_,key)=>{{if(skip.has(key))return undefined;return key in props?{{enumerable:true,configurable:true,value:props[key]}}:undefined;}},set:()=>false,deleteProperty:()=>false,defineProperty:()=>false,setPrototypeOf:()=>false,preventExtensions:()=>false}});}}
"#));
        } else {
            out.push_str(&format!(r#"
function {rest_helper}(props:any,excluded:string[]):any {{
 const skip=new {realm}.Set<PropertyKey>([...excluded,"$$slots","$$events"]);const rest:any={{}};
 for(const key of {realm}.Reflect.ownKeys(props)){{
  if(skip.has(key)||!{realm}.Object.prototype.propertyIsEnumerable.call(props,key))continue;
  {realm}.Object.defineProperty(rest,key,{{value:props[key],writable:true,enumerable:true,configurable:true}});
 }}
 return rest;
}}
"#));
        }
    }
    if unbound_used {
        out.push_str(&format!(
            "\nfunction {unbound}<T>(value:T):T {{ return value; }}\n"
        ));
    }
    if javascript {
        let value = "D extends true?((K extends keyof P?P[K]:undefined) extends infer T?T extends undefined?never:T:never)|F:(K extends keyof P?P[K]:unknown)";
        for name in [&helper, &bindable_helper] {
            if !out.contains(&format!("function {name}<")) {
                continue;
            }
            let tail = if name == &bindable_helper && matches!(target, MoonshineModuleTarget::Ssr) {
                ";commit:()=>void"
            } else {
                ""
            };
            let parameters = if name == &bindable_helper {
                "hasDefault:D"
            } else {
                "writable:boolean,hasDefault:D"
            };
            let doc = format!(
                "/** @returns {{<P,K extends string|number|symbol,F,D extends boolean>(props:P,key:K,fallback:()=>F,{parameters})=>{{value:{value}{tail}}}}} */\n"
            );
            out = out.replace(
                &format!("function {name}<"),
                &format!("{doc}function {name}<"),
            );
        }
        if needs_rest_helper {
            let doc = "/** @template P @template {keyof P} K @param {P} props @param {K[]} excluded @returns {{[Q in keyof P as Q extends K?never:Q]:P[Q]}} */\n";
            out = out.replace(
                &format!("function {rest_helper}("),
                &format!("{doc}function {rest_helper}("),
            );
        }
    }
    Ok((out, finalizer_handled, public_props))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(script: &str) -> Result<String, String> {
        lower_target(script, MoonshineModuleTarget::Dom)
    }
    fn lower_target(script: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let source =
            format!("export default function Component(props){{{script};return <p>{{count}}</p>}}");
        let (out, _, _) = transform_props(
            &source,
            Path::new("test.svelte"),
            target,
            "Component",
            "props",
            &[],
            None,
        )?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        Ok(out)
    }
    #[test]
    fn instantiated_prop_callbacks_keep_types_and_unbound_receivers() {
        let script="let {callback,object}=$props();const a=(callback<number>)();const b=(callback<string>)?.();const c=(callback<boolean>)`text`;const d=(object.method<number>)()";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower_target(script, target).unwrap();
            assert!(
                out.contains("__crepusPropUnbound((callback.value<number>))()"),
                "{out}"
            );
            assert!(
                out.contains("__crepusPropUnbound((callback.value<string>))?.()"),
                "{out}"
            );
            assert!(
                out.contains("__crepusPropUnbound((callback.value<boolean>))`text`"),
                "{out}"
            );
            assert!(out.contains("(object.value.method<number>)()"), "{out}");
            assert!(!out.contains("__crepusPropUnbound(object.value"), "{out}");
        }
    }
    #[test]
    fn prop_helpers_reserve_decoded_identifiers() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = lower_target(
                r"const \u005f_crepusSvelteProp=7;let {count}:{count:number}=$props();",
                target,
            )
            .unwrap();
            assert!(output.contains("function __crepusSvelteProp_<"), "{output}");
        }
    }

    #[test]
    fn absent_prop_fallbacks_do_not_capture_shadowed_undefined() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower_target("const undefined=7;let {optional,required}:{optional?:string,required:string}=$props();required='next';const value=optional", target).unwrap();
            assert!(
                out.contains("\"optional\",()=>(void 0),false,false)"),
                "{out}"
            );
            assert!(
                out.contains("\"required\",()=>(void 0),true,false)"),
                "{out}"
            );
            assert!(!out.contains("()=>(undefined)"), "{out}");
            assert!(out.contains("const undefined=7"));
            assert!(out.contains("({optional?:string,required:string})[\"optional\"]"));
            assert!(out.contains("({optional?:string,required:string})[\"required\"]"));
        }
    }

    #[test]
    fn empty_array_defaults_retain_the_declared_mutable_array_type() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for script in [
                "type Extract=number;let {items=[]}:{items?:number[]}=$props();items.push(2)",
                "let {items=[]}:{items?:number[]}=$props();items=[3];items.push(2)",
                "let {items=$bindable([])}:{items?:number[]}=$props();items.push(2)",
            ] {
                let out = lower_target(script, target).unwrap();
                assert!(
                    out.contains(
                        "value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T"
                    ),
                    "{out}"
                );
                assert!(!out.contains("(T extends undefined?never:T)|F"), "{out}");
                assert!(out.contains("items.value.push(2)"), "{out}");
            }
        }
    }

    #[test]
    fn typed_fallbacks_are_contextualized_by_the_declared_prop_type() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for script in [
                "let {initial='closed'}:{initial?:'open'|'closed'}=$props()",
                "let {initial=$bindable('closed')}:{initial?:'open'|'closed'}=$props()",
            ] {
                let out = lower_target(script, target).unwrap();
                assert!(
                    out.contains("return <F extends T|undefined,D extends boolean>"),
                    "{out}"
                );
                assert!(!out.contains("return <const F"));
            }
        }
    }

    #[test]
    fn live_alias_defaults_rest_and_shadowing() {
        let out=lower("let {count,title:label=count,...rest}=$props();count++;const obj={count};function inner(count){return count}").unwrap();
        assert!(out.contains("count.value++"));
        assert!(out.contains("()=> (count.value)") || out.contains("()=>(count.value)"));
        assert!(out.contains("{count: count.value}"));
        assert!(out.contains("function inner(count){return count}"));
        assert!(out.contains("__crepusSvelteRest(props,[\"count\",\"title\"])"));
    }
    #[test]
    fn rest_descriptors_do_not_expose_parent_setters() {
        let out = lower("let {count,...rest}=$props();rest={replacement:true}").unwrap();
        assert!(out.contains(
            "return key in props?{enumerable:true,configurable:true,value:props[key]}:undefined"
        ));
        assert!(!out.contains("getOwnPropertyDescriptor(props,key)"));
        assert!(out.contains("rest={replacement:true}"));
    }
    #[test]
    fn component_reference_symbol_exclusion_is_hygienic_and_dom_only() {
        for script in ["let attrs=$props()", "let {count,...rest}=$props()"] {
            let source = format!("const __crepusPropsComponentRef=1;{script}");
            let out = lower(&source).unwrap();
            assert!(out.contains("componentRef as __crepusPropsComponentRef_"));
            assert!(out.contains("\"$$legacy\",__crepusPropsComponentRef_]"));
            assert!(out.contains("Reflect.ownKeys(props).filter(key=>!skip.has(key))"));
            assert!(!out.contains("typeof key"));
            let ssr = lower_target(&source, MoonshineModuleTarget::Ssr).unwrap();
            assert!(!ssr.contains("component-instance"));
            assert!(!ssr.contains("__crepusPropsComponentRef_]"));
        }
    }

    #[test]
    fn dom_rest_excludes_client_internals_through_every_read_surface() {
        let out = lower("let {count,...rest}=$props()").unwrap();
        assert!(
            out.contains("new globalThis.Set<PropertyKey>([...excluded,\"$$slots\",\"$$events\",\"$$legacy\",__crepusPropsComponentRef])")
        );
        for surface in [
            "get:(_,key)=>skip.has(key)?undefined:props[key]",
            "has:(_,key)=>!skip.has(key)&&key in props",
            "ownKeys:()=>globalThis.Reflect.ownKeys(props).filter(key=>!skip.has(key))",
            "getOwnPropertyDescriptor:(_,key)=>{if(skip.has(key))return undefined;",
        ] {
            assert!(
                out.contains(surface),
                "missing guarded DOM rest surface {surface}: {out}"
            );
        }
        let ssr = lower_target("let {count,...rest}=$props()", MoonshineModuleTarget::Ssr).unwrap();
        assert!(ssr.contains("[...excluded,\"$$slots\",\"$$events\"]"));
        assert!(!ssr.contains("$$legacy"));
    }
    #[test]
    fn defaults_initialize_in_order_and_remain_reactive() {
        let out = lower("let {count=1,title=count}=$props()").unwrap();
        assert!(out.contains("if(hasDefault&&initial===undefined)__crepusPropUntrack(getFallback)"));
        assert!(out.contains("__crepusPropRunWithOwner(owner,()=>__crepusPropMemo(fallback))"));
        assert!(out.contains("return __crepusPropRunWithOwner(owner,()=>__crepusPropMemo<D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T>(()=>{const value=props[key];return value===undefined?getFallback():value;}))"));
        assert!(!out.contains("createSignal"));
        assert!(!out.contains("createComputed"));
        assert!(out.contains("\"title\",()=>(count.value)"));
    }
    #[test]
    fn readonly_props_read_source_directly() {
        let out = lower("let {count,note}=$props();count++;console.log(note)").unwrap();
        assert!(out.contains("\"count\",()=>(void 0),true,false)"));
        assert!(out.contains("\"note\",()=>(void 0),false,false)"));
        assert!(out.contains("if(!writable)return {get value(){const value=props[key];return value===undefined?getFallback():value;}}"));
        assert!(!out.contains("const initial=props[key]"));
        assert!(out.contains("const initial=hasDefault||writable?props[key]:undefined"));
    }
    #[test]
    fn compact_destructuring_keeps_declaration_keyword_separate() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower_target("let{value}=$props();console.log(value)", target).unwrap();
            assert!(out.contains("let value="), "{out}");
            assert!(!out.contains("letvalue="));
        }
    }
    #[test]
    fn exported_props_accept_instance_overrides_without_authored_writes() {
        let (out, _, _) = transform_props(
            "export default function Component(props){const{label}=$props();return label}",
            Path::new("test.svelte"),
            MoonshineModuleTarget::Dom,
            "Component",
            "props",
            &["label"],
            None,
        )
        .unwrap();
        assert!(out.contains("\"label\",()=>(void 0),true,false)"));
    }
    #[test]
    fn typed_and_multiple_declarations() {
        let out = lower("let {count}=$props<{count:number}>(),other=count").unwrap();
        assert!(out.contains("<({count:number})[\"count\"]>"));
        assert!(out.contains("other=count.value"));
    }
    #[test]
    fn function_props_preserve_unbound_receivers_and_object_methods() {
        let out=lower("let {count,callback,obj,tag}=$props();callback();(callback)();(callback as Function)();callback?.();tag`text`;obj.method()").unwrap();
        for call in [
            "__crepusPropUnbound(callback.value)()",
            "__crepusPropUnbound((callback.value))()",
            "__crepusPropUnbound((callback.value as Function))()",
            "__crepusPropUnbound(callback.value)?.()",
            "__crepusPropUnbound(tag.value)`text`",
            "obj.value.method()",
        ] {
            assert!(out.contains(call), "missing {call}: {out}");
        }
    }
    #[test]
    fn defaults_do_not_widen_required_props() {
        let out = lower(
            "let {count,name,optional=1}: {count:number,name:string,optional?:number}=$props()",
        )
        .unwrap();
        assert!(out.contains("\"name\",()=>(void 0),false,false)"));
        assert!(out.contains("\"optional\",()=>(1),false,true)"));
        assert!(out.contains(
            "value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T"
        ));
    }
    #[test]
    fn ssr_bindings_are_local_snapshots_with_ordered_defaults_and_unbound_calls() {
        let out = lower_target("let {count=1,title=count,callback}: {count?:number,title?:number,callback:()=>void}=$props();count++;callback()", MoonshineModuleTarget::Ssr).unwrap();
        assert!(out
            .contains("const value=props[key];return {value:value===undefined?fallback():value}"));
        assert!(out.contains("count.value++"));
        assert!(out.contains("\"title\",()=>(count.value)"));
        assert!(out.contains("__crepusPropUnbound(callback.value)()"));
        assert!(out.contains(
            "value:D extends true?(T extends undefined?never:T)|(F extends undefined?F:never):T"
        ));
        for runtime in [
            "@tschk/moonshine-dom",
            "@tschk/moonshine-svelte",
            "createDerived",
            "__crepusPropGetOwner",
            "__crepusPropRunWithOwner",
            "__crepusPropUntrack",
        ] {
            assert!(
                !out.contains(runtime),
                "unexpected SSR runtime {runtime}: {out}"
            );
        }
        assert!(!out.contains("get value()"));
    }
    #[test]
    fn ssr_rest_copies_only_enumerable_own_keys_into_mutable_safe_data_properties() {
        let out = lower_target(
            "let {count,...rest}=$props();rest.label='local';delete rest.old",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(out.contains("globalThis.Reflect.ownKeys(props)"));
        assert!(out.contains("propertyIsEnumerable.call(props,key)"));
        assert!(out.contains("[...excluded,\"$$slots\",\"$$events\"]"));
        assert!(out.contains("globalThis.Object.defineProperty(rest,key,{value:props[key],writable:true,enumerable:true,configurable:true})"));
        assert!(!out.contains("globalThis.Proxy"));
        assert!(out.contains("rest.label='local';delete rest.old"));
        assert!(out.contains("__crepusSvelteRest(props,[\"count\"])"));
    }
    #[test]
    fn props_aliases_keep_types_references_shadowing_and_local_reassignment() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower_target("let attrs: {count:number}=$props();const current=attrs.count;function inner(attrs){return attrs.count}attrs={count:2}", target).unwrap();
            assert!(out.contains("attrs: {count:number}=__crepusSvelteRest(props,[])"));
            assert!(out.contains("const current=attrs.count"));
            assert!(out.contains("function inner(attrs){return attrs.count}"));
            assert!(out.contains("attrs={count:2}"));
            assert!(!out.contains("attrs.value"));
            assert!(!out.contains("function __crepusSvelteProp<"));
            assert_eq!(
                out.contains("@tschk/moonshine-dom/component-instance"),
                matches!(target, MoonshineModuleTarget::Dom)
            );
        }
    }
    #[test]
    fn alias_and_rest_only_use_target_specific_helper_without_named_runtime() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for script in ["let attrs=$props()", "let {...attrs}=$props()"] {
                let out = lower_target(script, target).unwrap();
                assert!(out.contains("function __crepusSvelteRest("));
                assert!(!out.contains("function __crepusSvelteProp<"));
                if matches!(target, MoonshineModuleTarget::Dom) {
                    assert!(out.contains("@tschk/moonshine-dom/component-instance"));
                    assert!(out.contains("globalThis.Proxy"));
                    assert!(out.contains("$$legacy"));
                } else {
                    assert!(!out.contains("@tschk/moonshine"));
                    assert!(!out.contains("globalThis.Proxy"));
                    assert!(out.contains("globalThis.Object.defineProperty"));
                    assert!(!out.contains("$$legacy"));
                }
            }
        }
    }
    #[test]
    fn aliases_validate_scope_call_shape_and_generic_limit_without_touching_shadowed_calls() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for script in [
                "let attrs=$props(1)",
                "let attrs=$props?.()",
                "let attrs=$props<{count:number}>()",
                "if(true){let attrs=$props()}",
                "function nested(){let attrs=$props()}",
            ] {
                assert!(lower_target(script, target).is_err(), "{script}");
            }
            let out =
                lower_target("const $props=()=>({count:1});let attrs=$props()", target).unwrap();
            assert!(!out.contains("__crepusSvelteRest"));
            assert!(out.contains("let attrs=$props()"));
        }
    }
    #[test]
    fn props_can_only_be_declared_once_across_aliases_and_patterns() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for script in [
                "let first=$props();let second=$props()",
                "let {count}=$props();let {title}=$props()",
                "let first=$props();let {count}=$props()",
                "let {count}=$props();let second=$props()",
                "let first=$props(),second=$props()",
                "let {count}=$props(),second=$props()",
            ] {
                let error = lower_target(script, target).unwrap_err();
                assert!(error.contains("test.svelte"), "{error}");
                assert!(
                    error.contains("$props may only be declared once per component"),
                    "{script}: {error}"
                );
            }
            let out = lower_target("let attrs=$props();function inner($props){let first=$props();let second=$props();return first}function local(){const $props=()=>1;return $props()+$props()}", target).unwrap();
            assert!(out.contains("attrs=__crepusSvelteRest(props,[])"));
            assert!(out.contains("let first=$props();let second=$props()"));
        }
    }
    #[test]
    fn invalid_patterns_reject() {
        for script in [
            "let {count:{x}}=$props()",
            "let {[key]:count}=$props()",
            "const {count}=$props();count++",
        ] {
            assert!(lower(script).is_err(), "{script}");
        }
    }
    #[test]
    fn bindable_defaults_use_native_prop_cells_and_distinguish_absent_fallback() {
        let out=lower("let {count=$bindable(),title:label=$bindable(undefined),other=1}=$props(); count=2; const pair={count,label}").unwrap();
        assert!(out.contains("createProp as __crepusCreateBindableProp"));
        assert!(
            out.contains("count=__crepusSvelteBindableProp()(props,\"count\",()=>(void 0),false)")
        );
        assert!(out
            .contains("label=__crepusSvelteBindableProp()(props,\"title\",()=>(__crepusBindableState(undefined).value),true)"));
        assert!(out.contains("count.value=2"));
        assert!(out.contains("{count: count.value,label: label.value}"));
        assert!(out.contains("hasDefault?{bindable:true,mode:\"dom\",fallback:fallback as ()=>Value}:{bindable:true,mode:\"dom\"}"));
        assert!(out.contains("other=__crepusSvelteProp"));
        assert!(!out.contains("$bindable"));
    }
    #[test]
    fn typed_renamed_bindables_keep_key_types_and_generic_fallback_checks() {
        let out=lower("let {title:label=$bindable<string>('fallback'),count=$bindable<number>()}: {title?:string,count?:number}=$props();label.toUpperCase()").unwrap();
        assert!(out.contains("label=__crepusSvelteBindableProp<({title?:string,count?:number})[\"title\"]>()(props,\"title\",():string=>(__crepusBindableState('fallback').value),true)"));
        assert!(out.contains("count=__crepusSvelteProp<({title?:string,count?:number})[\"count\"]>()(props,\"count\",()=>(void 0),false,false)"));
        assert!(out.contains("label.value.toUpperCase()"));
    }
    #[test]
    fn server_bindables_commit_after_root_serialization_only() {
        let out=lower_target("let {count=$bindable(3),title=$bindable()}=$props();function nested(){return count};const arrow=()=>{return count};count++;",MoonshineModuleTarget::Ssr).unwrap();
        assert!(out.contains("createServerProp as __crepusCreateBindableProp"));
        assert!(out.contains("afterRender as __crepusBindableAfterRender"));
        assert!(out.contains("function nested(){return count.value}"));
        assert!(out.contains("const arrow=()=>{return count.value}"));
        assert!(out.contains("return __crepusBindableAfterRender(<p>{count.value}</p>,()=>{count.commit();title.commit();})"));
        assert!(out.contains("count.value++"));
        assert!(!out.contains("createProp as"));
        assert!(!out.contains("moonshine-dom/reactive"));
    }
    #[test]
    fn bindable_admission_shadowing_const_rules_and_default_refs() {
        let out = lower("let {earlier=1,count=$bindable(earlier)}=$props()").unwrap();
        assert!(out.contains("\"count\",()=>(__crepusBindableState(earlier.value).value),true"));
        let out =
            lower("function $bindable(value){return value};let {count=$bindable(2)}=$props()")
                .unwrap();
        assert!(!out.contains("createProp as"));
        assert!(out.contains("$bindable(2)"));
        for source in [
            "let {count=$bindable(1,2)}=$props()",
            "let {count=$bindable(...args)}=$props()",
            "let {count=$bindable?.(1)}=$props()",
            "let {count=$bindable<number,string>(1)}=$props()",
            "let count=$bindable(1)",
            "let {count=1}=$props();const bad=$bindable",
            "const {count=$bindable(1)}=$props();count=2",
        ] {
            assert!(lower(source).is_err(), "{source}");
        }
    }
    #[test]
    fn readonly_bindable_without_fallback_uses_no_constructor_or_setup_read() {
        let out = lower("let {count=$bindable()}=$props()").unwrap();
        assert!(
            out.contains("count=__crepusSvelteProp()(props,\"count\",()=>(void 0),false,false)")
        );
        assert!(!out.contains("createProp as"));
        assert!(!out.contains("__crepusSvelteBindableProp"));
        assert!(out.contains("const initial=hasDefault||writable?props[key]:undefined"));
        assert!(out.contains("if(!writable)return {get value(){const value=props[key]"));
    }
    #[test]
    fn bindable_dom_defaults_are_deep_state_values_while_server_defaults_stay_raw() {
        let source = "let {count=$bindable({nested:{value:1}})}=$props();count.nested.value++";
        let out = lower(source).unwrap();
        assert!(out.contains("createState as __crepusBindableState"));
        assert!(out.contains("__crepusBindableState({nested:{value:1}}).value"));
        assert!(out.contains("count.value.nested.value++"));
        let server = lower_target(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(!server.contains("createState as"));
        assert!(server.contains("()=>("));
        assert!(server.contains("{nested:{value:1}}"));
    }
}
