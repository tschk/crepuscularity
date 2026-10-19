//! Component-owned Svelte auto-subscriptions and writable-store expressions.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::{collections::HashMap, path::Path};

pub(crate) struct ServerStoreCompletion {
    group: String,
    after_render: String,
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    component: &str,
    target: MoonshineModuleTarget,
) -> Result<(String, Option<ServerStoreCompletion>), String> {
    transform_with_policy(
        source,
        path,
        component,
        target,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    component: &str,
    target: MoonshineModuleTarget,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(String, Option<ServerStoreCompletion>), String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let fail = |message: &str| format!("{}: {message}", path.display());
    if !parsed.errors.is_empty() {
        return Err(fail("invalid Svelte store script"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("invalid Svelte store scope"));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let function = nodes
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Function(f) if f.id.as_ref().is_some_and(|id| id.name == component) => Some(f),
            _ => None,
        })
        .ok_or_else(|| fail("missing Svelte instance function"))?;
    let scope = function
        .scope_id
        .get()
        .expect("semantic builder assigns the component function scope");
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| fail("missing Svelte instance body"))?;
    let server = matches!(target, MoonshineModuleTarget::Ssr);
    let set_value = unique(source, "__crepusSetStoreValue");
    let mutate = unique(source, "__crepusMutateStoreValue");
    let update = unique(source, "__crepusUpdateStoreValue");
    let untrack = unique(source, "__crepusStoreUntrack");
    let mut uses_set = false;
    let mut uses_mutate = false;
    let mut uses_update = false;
    let mut uses_untrack = false;
    let mut stores = Vec::new();
    let mut edits = Vec::new();
    let mut endings = Vec::new();
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let Some(name) = id.name.strip_prefix('$') else {
            continue;
        };
        if name.is_empty()
            || matches!(
                name,
                "state" | "derived" | "props" | "bindable" | "effect" | "inspect" | "host"
            )
            || crate::svelte_script::has_runtime_binding(&semantic, id)
        {
            continue;
        }
        if !nodes
            .ancestors(node.id())
            .any(|n| matches!(n.kind(), AstKind::Function(f) if std::ptr::eq(f, function)))
        {
            return Err(fail(
                "store subscriptions must belong to the component instance",
            ));
        }
        let symbol = semantic
            .scoping()
            .find_binding(node.scope_id(), name.into())
            .ok_or_else(|| fail("auto-subscription requires a declared store"))?;
        if semantic.scoping().find_binding(scope, name.into()) != Some(symbol) {
            return Err(fail(
                "store subscriptions require a top-level component or imported store",
            ));
        }
        let reference = semantic.scoping().get_reference(
            id.reference_id
                .get()
                .expect("semantic builder assigns every identifier reference"),
        );
        if reference.is_type() || reference.flags().is_value_as_type() {
            return Err(fail("store subscription type queries are not supported"));
        }
        let index = stores
            .iter()
            .position(|(s, _)| *s == symbol)
            .unwrap_or_else(|| {
                stores.push((symbol, name.to_owned()));
                stores.len() - 1
            });
        let helper = unique(source, &format!("__crepusStore{index}"));
        let read = format!("{helper}.get({name})!");
        let mut replacement = format!("({read})");
        let mut handled_write = false;
        let mut direct_write = false;
        for parent in nodes.ancestors(node.id()) {
            match parent.kind() {
                AstKind::AssignmentExpression(a) if contains(a.left.span(), id.span) => {
                    let Some(root) = assignment_root(&a.left) else {
                        return Err(fail(
                            "destructuring $store assignment targets are not supported",
                        ));
                    };
                    if root != id.span {
                        continue;
                    }
                    handled_write = true;
                    direct_write = a.left.span() == id.span;
                    if direct_write {
                        uses_set = true;
                        let comments = parsed
                            .program
                            .comments
                            .iter()
                            .filter(|comment| {
                                a.span.start <= comment.span.start
                                    && comment.span.end <= a.right.span().start
                            })
                            .map(|comment| {
                                format!(
                                    "{}\n",
                                    &source[comment.span.start as usize..comment.span.end as usize]
                                )
                            })
                            .collect::<String>();
                        let operator = a.operator.as_str();
                        let prefix = if operator == "=" {
                            format!("{set_value}({name},{comments}")
                        } else {
                            let operator = operator
                                .strip_suffix('=')
                                .expect("compound assignment ends with equals");
                            format!("{set_value}({name},{comments}({read}) {operator} (")
                        };
                        edits.push((a.span.start as usize, a.right.span().start as usize, prefix));
                        endings.push((
                            a.span.end as usize,
                            a.span.start as usize,
                            if operator == "=" { ")" } else { "))" }.into(),
                        ));
                    } else {
                        uses_mutate = true;
                        if server {
                            edits.push((
                                a.span.start as usize,
                                a.span.start as usize,
                                format!("{mutate}({helper},{name},"),
                            ));
                            endings.push((a.span.end as usize, a.span.start as usize, ")".into()));
                        } else {
                            uses_untrack = true;
                            replacement = format!("{untrack}(()=>{read})");
                            edits.push((
                                a.span.start as usize,
                                a.span.start as usize,
                                format!("{mutate}({name},"),
                            ));
                            endings.push((
                                a.span.end as usize,
                                a.span.start as usize,
                                format!(",{untrack}(()=>{read}))"),
                            ));
                        }
                    }
                    break;
                }
                AstKind::UpdateExpression(a) if contains(a.argument.span(), id.span) => {
                    let Some(root) = simple_root(&a.argument) else {
                        return Err(fail("unsupported $store update target"));
                    };
                    if root != id.span {
                        continue;
                    }
                    handled_write = true;
                    direct_write = a.argument.span() == id.span;
                    if direct_write {
                        uses_update = true;
                        let delta = if a.operator.as_str() == "++" { 1 } else { -1 };
                        let comments = parsed
                            .program
                            .comments
                            .iter()
                            .filter(|comment| contains(a.span, comment.span))
                            .map(|comment| {
                                format!(
                                    "{}\n",
                                    &source[comment.span.start as usize..comment.span.end as usize]
                                )
                            })
                            .collect::<String>();
                        edits.push((
                            a.span.start as usize,
                            a.span.end as usize,
                            format!("{update}({name},{comments}{read},{delta},{})", a.prefix),
                        ));
                    } else if !server {
                        uses_mutate = true;
                        uses_untrack = true;
                        replacement = format!("{untrack}(()=>{read})");
                        edits.push((
                            a.span.start as usize,
                            a.span.start as usize,
                            format!("{mutate}({name},"),
                        ));
                        endings.push((
                            a.span.end as usize,
                            a.span.start as usize,
                            format!(",{untrack}(()=>{read}))"),
                        ));
                    }
                    break;
                }
                AstKind::ForOfStatement(a) if contains(a.left.span(), id.span) => {
                    return Err(fail(
                        "$store iteration assignment targets are not supported",
                    ));
                }
                AstKind::ForInStatement(a) if contains(a.left.span(), id.span) => {
                    return Err(fail(
                        "$store iteration assignment targets are not supported",
                    ));
                }
                AstKind::UnaryExpression(a)
                    if a.operator.as_str() == "delete" && contains(a.argument.span(), id.span) =>
                {
                    return Err(fail("deleting $store values is not supported"));
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                _ => {}
            }
        }
        if reference.is_write() && !handled_write {
            return Err(fail("unsupported $store assignment target"));
        }
        if !direct_write {
            if matches!(nodes.parent_kind(node.id()), AstKind::ObjectProperty(p) if p.shorthand) {
                replacement = format!("{}:{read}", id.name);
            }
            edits.push((id.span.start as usize, id.span.end as usize, replacement));
        }
    }
    if stores.is_empty() {
        return Ok((source.to_owned(), None));
    }
    let helpers: HashMap<_, _> = stores
        .iter()
        .enumerate()
        .map(|(i, (symbol, _))| (*symbol, unique(source, &format!("__crepusStore{i}"))))
        .collect();
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic.scoping().get_reference(
            id.reference_id
                .get()
                .expect("semantic builder assigns every identifier reference"),
        );
        let Some(helper) = reference.symbol_id().and_then(|s| helpers.get(&s)) else {
            continue;
        };
        if !reference.is_write() {
            continue;
        }
        let symbol = reference
            .symbol_id()
            .expect("selected store reference has a resolved symbol");
        let reactive = nodes.iter().any(|candidate| {
            let AstKind::VariableDeclarator(declaration) = candidate.kind() else { return false; };
            let BindingPattern::BindingIdentifier(binding) = &declaration.id else { return false; };
            if binding.symbol_id.get() != Some(symbol) { return false; }
            let Some(Expression::CallExpression(call)) = &declaration.init else { return false; };
            match &call.callee {
                Expression::Identifier(callee) => callee.name == "$state" && !crate::svelte_script::has_runtime_binding(&semantic, callee),
                Expression::StaticMemberExpression(member) => matches!(&member.object, Expression::Identifier(callee) if callee.name == "$state" && !crate::svelte_script::has_runtime_binding(&semantic, callee)) && member.property.name == "raw",
                _ => false,
            }
        });
        if !reactive {
            return Err(fail(
                "store-variable replacement currently requires a $state binding",
            ));
        }
        let parent = nodes.parent_kind(node.id());
        let AstKind::AssignmentExpression(a) = parent else {
            return Err(fail("only simple store-variable replacement is supported"));
        };
        if a.left.span() != id.span || a.operator.as_str() != "=" {
            return Err(fail("only simple store-variable replacement is supported"));
        }
        // Separate insertions preserve edits nested in the RHS and ensure assignment precedes release.
        edits.push((
            a.span.start as usize,
            a.span.start as usize,
            format!("{helper}.releaseIfChanged(("),
        ));
        endings.push((a.span.end as usize, a.span.start as usize, "))".into()));
    }
    // The uncalled closure infers the payload after rune lowering, without a type query on a cell.
    let factory = unique(source, "__crepusStoreFactory");
    let imported = unique(source, "__crepusCreateStoreReference");
    let group = unique(source, "__crepusServerStores");
    let group_factory = unique(source, "__crepusServerStoreFactory");
    let after_render = unique(source, "__crepusStoreAfterRender");
    let view = unique(source, "__crepusStoreView");
    let mut declarations = stores
        .iter()
        .enumerate()
        .map(|(i, (_, name))| {
            format!(
                "const {}={factory}(()=>{name}{});",
                unique(source, &format!("__crepusStore{i}")),
                if server {
                    format!(",{group}")
                } else {
                    String::new()
                }
            )
        })
        .collect::<String>();
    if server {
        declarations.insert_str(0, &format!("const {group}={group_factory}();"));
    }
    edits.push((
        body.span.start as usize + 1,
        body.span.start as usize + 1,
        declarations,
    ));
    let Some(Statement::ReturnStatement(ret)) = body.statements.last() else {
        return Err(fail("store setup requires a final component return"));
    };
    let argument = ret
        .argument
        .as_ref()
        .ok_or_else(|| fail("missing store component view"))?;
    edits.push((
        ret.span.start as usize,
        argument.span().start as usize,
        format!("const {view}="),
    ));
    let finish = if server {
        String::new()
    } else {
        stores
            .iter()
            .enumerate()
            .map(|(i, _)| format!("{}.finish();", unique(source, &format!("__crepusStore{i}"))))
            .collect::<String>()
    };
    edits.push((
        argument.span().end as usize,
        ret.span.end as usize,
        format!(";{finish}return {view};"),
    ));
    // Nested writes may end at the same byte. Close inner expressions first.
    endings.sort_by_key(|(end, start, _)| (*end, std::cmp::Reverse(*start)));
    for (end, _, text) in endings {
        if let Some((_, _, previous)) = edits
            .iter_mut()
            .find(|(start, finish, _)| *start == end && *finish == end)
        {
            previous.push_str(&text);
        } else {
            edits.push((end, end, text));
        }
    }
    edits.sort_by_key(|e| (e.0, e.1));
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    let mut imports = Vec::new();
    if uses_set {
        imports.push(format!("setStoreValue as {set_value}"));
    }
    if uses_mutate {
        imports.push(format!(
            "{} as {mutate}",
            if server {
                "mutateServerStore"
            } else {
                "mutateStoreValue"
            }
        ));
    }
    if uses_update {
        imports.push(format!("updateStoreValue as {update}"));
    }
    if !imports.is_empty() {
        output.insert_str(
            0,
            &format!(
                "import {{{}}} from '@tschk/moonshine-svelte/{}';\n",
                imports.join(","),
                if server { "ssr" } else { "store-binding" }
            ),
        );
    }
    if uses_untrack {
        output.insert_str(
            0,
            &format!("import {{untrack as {untrack}}} from '@tschk/moonshine-dom/reactive';\n"),
        );
    }
    let typed_source =
        "import('@tschk/moonshine-svelte/store-binding').StoreSource<T>|null|undefined";
    if server {
        Ok((
            format!(
                "import {{createServerStoreReferences as {group_factory},afterRender as {after_render}}} from '@tschk/moonshine-svelte/ssr';\nfunction {factory}<T>(_read:()=>{typed_source},group:import('@tschk/moonshine-svelte/ssr').ServerStoreReferences){{return group.create<T>();}}\n{output}"
            ),
            Some(ServerStoreCompletion {
                group,
                after_render,
            }),
        ))
    } else {
        Ok((
            format!(
                "import {{createStoreReference as {imported}}} from '@tschk/moonshine-svelte/store-binding';\nfunction {factory}<T>(_read:()=>{typed_source}){{return {imported}<T>();}}\n{output}"
            ),
            None,
        ))
    }
}
fn contains(outer: oxc_span::Span, inner: oxc_span::Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
fn expression_root(expression: &Expression<'_>) -> Option<oxc_span::Span> {
    match expression.get_inner_expression() {
        Expression::Identifier(id) => Some(id.span),
        Expression::StaticMemberExpression(member) => expression_root(&member.object),
        Expression::ComputedMemberExpression(member) => expression_root(&member.object),
        Expression::PrivateFieldExpression(member) => expression_root(&member.object),
        _ => None,
    }
}
fn simple_root(target: &SimpleAssignmentTarget<'_>) -> Option<oxc_span::Span> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => Some(id.span),
        SimpleAssignmentTarget::StaticMemberExpression(member) => expression_root(&member.object),
        SimpleAssignmentTarget::ComputedMemberExpression(member) => expression_root(&member.object),
        SimpleAssignmentTarget::PrivateFieldExpression(member) => expression_root(&member.object),
        _ => None,
    }
}
fn assignment_root(target: &AssignmentTarget<'_>) -> Option<oxc_span::Span> {
    target.as_simple_assignment_target().and_then(simple_root)
}
fn unique(source: &str, base: &str) -> String {
    let mut name = base.to_owned();
    while source.contains(&name) {
        name.push('_');
    }
    name
}

impl ServerStoreCompletion {
    pub(crate) fn statement(&self) -> String {
        format!("{}.finish();", self.group)
    }

    /// Finalize outside bindable commits and buffered settlement, after successful serialization.
    pub(crate) fn wrap(&self, source: &str, component: &str) -> Result<String, String> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid compiled Svelte store completion syntax".into());
        }
        let functions: Vec<_> = parsed
            .program
            .body
            .iter()
            .filter_map(|statement| {
                let Statement::ExportDefaultDeclaration(export) = statement else {
                    return None;
                };
                let ExportDefaultDeclarationKind::FunctionDeclaration(function) =
                    &export.declaration
                else {
                    return None;
                };
                function
                    .id
                    .as_ref()
                    .is_some_and(|id| id.name == component)
                    .then_some(function)
            })
            .collect();
        if functions.len() != 1 {
            return Err(
                "Svelte store completion requires one named default component function".into(),
            );
        }
        let body = functions[0]
            .body
            .as_ref()
            .ok_or("missing Svelte store completion body")?;
        let Some(Statement::ReturnStatement(statement)) = body.statements.last() else {
            return Err("Svelte store completion requires a final return".into());
        };
        let argument = statement
            .argument
            .as_ref()
            .ok_or("missing Svelte store completion value")?;
        let span = argument.span();
        let mut output = source.to_owned();
        output.insert_str(span.end as usize, &format!(",()=>{}.finish())", self.group));
        output.insert_str(span.start as usize, &format!("{}(", self.after_render));
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writable_operator_comments_survive_this_ast_pass() {
        let source = "import {current} from './store';function Component(){function write(){return $current /*store*/ += /*payload*/ 2;}return null;}";
        let (output, _) = transform(
            source,
            Path::new("Comment.svelte"),
            "Component",
            MoonshineModuleTarget::Dom,
        )
        .expect("valid writable component");
        assert!(output.contains("/*store*/"), "{output}");
        assert!(output.contains("/*payload*/"), "{output}");
        let source = "import {current} from './store';function Component(){function write(){return ++/*update*/$current;}return null;}";
        let (output, _) = transform(
            source,
            Path::new("Comment.svelte"),
            "Component",
            MoonshineModuleTarget::Dom,
        )
        .expect("valid update component");
        assert!(output.contains("/*update*/"), "{output}");
    }
}
