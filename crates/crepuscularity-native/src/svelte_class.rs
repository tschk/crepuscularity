//! Class-rune initializers and direct constructor first assignments.
use crate::{svelte_script::has_runtime_binding, MoonshineModuleTarget};
use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::{ast::*, AstBuilder, AstKind};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::{Semantic, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span, SPAN};
use oxc_syntax::{number::NumberBase, operator::UnaryOperator};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Clone, Copy)]
enum Rune {
    State,
    Raw,
    Derived,
    By,
}

#[derive(Clone, Copy)]
struct PrivateAccess {
    field: u32,
    rune: Rune,
    constructor: bool,
    callable: bool,
}

#[derive(Clone)]
struct ConstructorInit {
    body: u32,
    assignment: u32,
    assignment_end: u32,
    field: u32,
    name: String,
    declared: bool,
}

pub(crate) struct Plan {
    public_probes: HashMap<(u32, u32), u32>,
    constructor_inits: Vec<ConstructorInit>,
    private_accesses: HashMap<u32, PrivateAccess>,
    fields: HashMap<u32, Rune>,
    prefix: String,
}

pub(crate) fn plan(
    source: &str,
    path: &Path,
    semantic: &Semantic<'_>,
) -> Result<Option<Plan>, String> {
    if semantic.classes().len() == 0 {
        return Ok(None);
    }
    let mut fields = HashMap::new();
    for node in semantic.nodes().iter() {
        let AstKind::PropertyDefinition(field) = node.kind() else {
            continue;
        };
        let Some(Expression::CallExpression(call)) =
            field.value.as_ref().map(Expression::get_inner_expression)
        else {
            continue;
        };
        let (id, member) = match call.callee.get_inner_expression() {
            Expression::Identifier(id) => (id, None),
            Expression::StaticMemberExpression(member) => {
                let Expression::Identifier(id) = member.object.get_inner_expression() else {
                    continue;
                };
                (id, Some(member.property.name.as_str()))
            }
            _ => continue,
        };
        if has_runtime_binding(semantic, id) {
            continue;
        }
        let rune = match (id.name.as_str(), member) {
            ("$state", None) => Rune::State,
            ("$state", Some("raw")) => Rune::Raw,
            ("$derived", None) => Rune::Derived,
            ("$derived", Some("by")) => Rune::By,
            _ => continue,
        };
        if !matches!(field.value, Some(Expression::CallExpression(_)))
            || field.r#static
            || field.computed
            || !field.decorators.is_empty()
            || field.readonly
            || field.definite
            || field.declare
            || field.optional
            || field.r#override
            || field
                .accessibility
                .is_some_and(|access| access != TSAccessibility::Public)
            || !matches!(
                field.key,
                PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_)
            )
        {
            return Err(format!("{}: unsupported Svelte class rune field: only public/private instance field initializers are supported",path.display()));
        }
        if call.optional
            || (matches!(rune, Rune::Derived | Rune::By) && call.type_arguments.is_some())
            || call
                .type_arguments
                .as_ref()
                .is_some_and(|types| types.params.len() != 1)
            || call.arguments.len() > 1
            || (call.arguments.is_empty() && !matches!(rune, Rune::State | Rune::Raw))
            || call
                .arguments
                .first()
                .is_some_and(|arg| arg.as_expression().is_none())
        {
            return Err(format!(
                "{}: unsupported Svelte class rune initializer arguments",
                path.display()
            ));
        }
        if matches!(rune, Rune::By) {
            let argument = call.arguments[0]
                .as_expression()
                .expect("validated nonspread argument");
            if !matches!(
                argument,
                Expression::Identifier(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::FunctionExpression(_)
            ) || matches!(argument,Expression::ArrowFunctionExpression(function) if function.r#async)
                || matches!(argument,Expression::FunctionExpression(function) if function.r#async||function.generator)
            {
                return Err(format!(
                    "{}: unsupported Svelte class derived callback",
                    path.display()
                ));
            }
        }
        if matches!(call.callee.get_inner_expression(),Expression::StaticMemberExpression(member) if member.optional)
        {
            return Err(format!(
                "{}: unsupported optional Svelte class rune call",
                path.display()
            ));
        }
        fields.insert(field.span.start, rune);
    }
    let mut constructor_inits = Vec::new();
    let mut constructor_calls = HashMap::new();
    for node in semantic.nodes().iter() {
        let AstKind::Class(class) = node.kind() else {
            continue;
        };
        for element in &class.body.body {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };
            if method.kind != MethodDefinitionKind::Constructor {
                continue;
            }
            let Some(body) = &method.value.body else {
                continue;
            };
            let mut prior_writes = ConstructorWrites::default();
            prior_writes.visit_function_body(body);
            for statement in &body.statements {
                let Statement::ExpressionStatement(statement) = statement else {
                    continue;
                };
                let Expression::AssignmentExpression(assignment) = &statement.expression else {
                    continue;
                };
                let (name, private) = match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member)
                        if matches!(
                            member.object.get_inner_expression(),
                            Expression::ThisExpression(_)
                        ) && !member.optional =>
                    {
                        (member.property.name.to_string(), false)
                    }
                    AssignmentTarget::PrivateFieldExpression(member)
                        if matches!(
                            member.object.get_inner_expression(),
                            Expression::ThisExpression(_)
                        ) && !member.optional =>
                    {
                        (member.field.name.to_string(), true)
                    }
                    _ => continue,
                };
                let prior = prior_writes.writes.iter().any(|(start, key, is_private)| {
                    *start < assignment.span.start && key == &name && *is_private == private
                });
                let Expression::CallExpression(call) = assignment.right.get_inner_expression()
                else {
                    continue;
                };
                let Some(rune) = rune_call(call, semantic) else {
                    continue;
                };
                if !assignment.operator.is_assign()
                    || !matches!(assignment.right, Expression::CallExpression(_))
                {
                    return Err(format!(
                        "{}: unsupported constructor rune placement; use a direct first assignment",
                        path.display()
                    ));
                }
                if prior {
                    return Err(format!(
                        "{}: constructor rune field has a prior assignment: {name}",
                        path.display()
                    ));
                }
                if class.body.body.iter().any(|element| matches!(element,
                    ClassElement::PropertyDefinition(field) if !private && field.r#static && field.value.is_some()
                        && field.key.static_name().is_some_and(|key| key == name.as_str()))) {
                    return Err(format!("{}: constructor rune field already has a static initializer: {name}", path.display()));
                }
                let declaration = class.body.body.iter().find_map(|element| match element {
                    ClassElement::PropertyDefinition(field)
                        if !field.r#static
                            && match &field.key {
                                PropertyKey::PrivateIdentifier(key) => {
                                    private && key.name == name.as_str()
                                }
                                PropertyKey::StaticIdentifier(key) => {
                                    !private && key.name == name.as_str()
                                }
                                _ => false,
                            } =>
                    {
                        Some(field)
                    }
                    _ => None,
                });
                if private && declaration.is_none() {
                    return Err(format!(
                        "{}: missing private constructor rune declaration",
                        path.display()
                    ));
                }
                if let Some(field) = declaration {
                    if field.value.is_some() || fields.contains_key(&field.span.start) {
                        return Err(format!(
                            "{}: constructor rune field already has an initializer: {name}",
                            path.display()
                        ));
                    }
                    if field.readonly
                        || field.declare
                        || field.optional
                        || field.definite
                        || field.r#override
                        || !field.decorators.is_empty()
                        || field.computed
                        || field
                            .accessibility
                            .is_some_and(|access| access != TSAccessibility::Public)
                    {
                        return Err(format!(
                            "{}: unsupported constructor rune field modifiers: {name}",
                            path.display()
                        ));
                    }
                }
                validate_constructor_call(call, rune, path)?;
                let field = declaration.map_or(assignment.span.start, |field| field.span.start);
                fields.insert(field, rune);
                constructor_calls.insert(field, call);
                constructor_inits.push(ConstructorInit {
                    body: class.body.span.start,
                    assignment: assignment.span.start,
                    assignment_end: assignment.span.end,
                    field,
                    name,
                    declared: declaration.is_some(),
                });
            }
        }
    }
    if fields.is_empty() {
        return Ok(None);
    }
    let mut private_accesses = HashMap::new();
    let classes = semantic.classes();
    for (class_id, _) in classes.iter_enumerated() {
        for reference in classes.iter_private_identifiers(class_id) {
            let Some(owner) = classes
                .ancestors(class_id)
                .find(|id| classes.has_private_definition(*id, reference.name))
            else {
                continue;
            };
            let ids = classes.get_element_ids(owner, reference.name, true);
            let Some(element) = ids.first().map(|id| &classes.elements[owner][*id]) else {
                continue;
            };
            let Some(field) = semantic.nodes().iter().find_map(|node| match node.kind() {
                AstKind::PropertyDefinition(field) if matches!(&field.key, PropertyKey::PrivateIdentifier(key) if key.span == element.span) => Some(field),
                _ => None,
            }) else { continue; };
            let Some(rune) = fields.get(&field.span.start).copied() else {
                continue;
            };
            if owner != class_id {
                return Err(format!("{}: unsupported outer lexical private rune reference in nested class; pinned Svelte exposes its internal cell", path.display()));
            }

            let callable = if let Some(call) = constructor_calls.get(&field.span.start) {
                private_callable_initializer(field, rune, Some(call.as_ref()))
            } else {
                private_callable(field, rune)
            };
            let mut constructor = false;
            for ancestor in semantic.nodes().ancestors(reference.id) {
                match ancestor.kind() {
                    AstKind::ArrowFunctionExpression(_) => break,
                    // $derived(expression) moves its argument into a tracked
                    // callback even when its declaration is in the constructor.
                    AstKind::CallExpression(call)
                        if matches!(rune_call(call, semantic), Some(Rune::Derived | Rune::By)) =>
                    {
                        break
                    }
                    AstKind::Function(function) => {
                        constructor = matches!(semantic.nodes().parent_kind(function.node_id.get()), AstKind::MethodDefinition(method) if method.kind == MethodDefinitionKind::Constructor);
                        break;
                    }
                    // A class field executes outside its constructor's lexical body.
                    AstKind::PropertyDefinition(_) => break,
                    _ => {}
                }
            }
            private_accesses.insert(
                reference.span.start,
                PrivateAccess {
                    field: field.span.start,
                    rune,
                    constructor,
                    callable,
                },
            );
        }
    }
    let typed_public_fields: HashSet<u32> = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::PropertyDefinition(field)
                if field.type_annotation.is_some()
                    && !matches!(field.key, PropertyKey::PrivateIdentifier(_)) =>
            {
                Some(field.span.start)
            }
            _ => None,
        })
        .collect();
    let mut public_probes = HashMap::new();
    for node in semantic.nodes().iter() {
        let (span, name) = match node.kind() {
            AstKind::StaticMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                (member.span, member.property.name.as_str())
            }
            AstKind::ComputedMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                let Expression::StringLiteral(key) = member.expression.get_inner_expression()
                else {
                    continue;
                };
                (member.span, key.value.as_str())
            }
            _ => continue,
        };
        let mut context = false;
        let mut probe_span = span;
        for ancestor in semantic.nodes().ancestors(node.id()) {
            match ancestor.kind() {
                AstKind::AssignmentExpression(assignment) if assignment.left.span() == span => {
                    break
                }
                AstKind::UpdateExpression(_) => break,
                AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Delete => break,
                AstKind::CallExpression(call)
                    if call.callee.get_inner_expression().span() == span =>
                {
                    probe_span = call.span;
                }
                AstKind::TaggedTemplateExpression(tag)
                    if tag.tag.get_inner_expression().span() == span =>
                {
                    probe_span = tag.span;
                }
                // TypeScript treats deferred function bodies separately, including IIFEs.
                AstKind::ArrowFunctionExpression(_) => break,
                AstKind::Function(function) => {
                    if !matches!(semantic.nodes().parent_kind(function.node_id.get()), AstKind::MethodDefinition(method) if method.kind == MethodDefinitionKind::Constructor)
                    {
                        break;
                    }
                    context = true;
                }
                AstKind::PropertyDefinition(field) => {
                    if field.r#static
                        || !field
                            .value
                            .as_ref()
                            .is_some_and(|value| value.span().contains_inclusive(span))
                    {
                        break;
                    }
                    context = true;
                }
                AstKind::Class(class)
                    if !class
                        .super_class
                        .as_ref()
                        .is_some_and(|value| value.span().contains_inclusive(span)) =>
                {
                    break
                }
                AstKind::ClassBody(body) => {
                    if context {
                        if let Some(init) = constructor_inits.iter().find(|init| {
                            init.body == body.span.start
                                && init.name == name
                                && init.declared
                                && typed_public_fields.contains(&init.field)
                                && span.start < init.assignment_end
                        }) {
                            public_probes.insert((probe_span.start, probe_span.end), init.field);
                        }
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    let mut prefix = "__crepusClass".to_string();
    // Oxc has decoded escapes; raw source matching alone misses valid names
    // such as \u005f\u005fcrepusClassCell0 and outer private identifiers.
    let names: Vec<&str> = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.as_str()),
            AstKind::IdentifierReference(id) => Some(id.name.as_str()),
            AstKind::IdentifierName(id) => Some(id.name.as_str()),
            AstKind::PrivateIdentifier(id) => Some(id.name.as_str()),
            _ => None,
        })
        .collect();
    while source.contains(&prefix) || names.iter().any(|name| name.starts_with(&prefix)) {
        prefix.push('_');
    }
    Ok(Some(Plan {
        fields,
        prefix,
        private_accesses,
        constructor_inits,
        public_probes,
    }))
}

/// Check original typed blocks before generated code loses their byte ranges.
/// One combined scope preserves module-to-instance rune shadowing.
pub(crate) fn validate_language(
    source: &str,
    path: &Path,
    typed_ranges: &[Span],
    source_type: SourceType,
) -> Result<(), String> {
    if typed_ranges.is_empty() {
        return Ok(());
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte class language-validation source: {:?}",
            path.display(),
            parsed.errors
        ));
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte class language-validation scope: {:?}",
            path.display(),
            built.errors
        ));
    }
    if let Some(plan) = plan(source, path, &built.semantic)? {
        for init in plan.constructor_inits {
            if !init.declared
                && typed_ranges
                    .iter()
                    .any(|range| range.start <= init.body && init.body < range.end)
            {
                return Err(format!("{}: undeclared public constructor rune field in TypeScript: {}; declare the authored field",path.display(),init.name));
            }
        }
    }
    Ok(())
}

pub(crate) fn lower<'a>(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    plan: Plan,
) -> Result<String, String> {
    let mut visitor = Lower {
        allocator,
        constructor_inits: plan.constructor_inits,
        constructor_types: HashMap::new(),
        public_storage: HashMap::new(),
        public_probes: plan.public_probes,
        private_accesses: plan.private_accesses,
        identity_used: false,
        wrapper_used: false,
        fields: plan.fields,
        prefix: plan.prefix,
        target,
        serial: 0,
        imports: Vec::new(),
        error: None,
    };
    visitor.visit_program(program);
    if let Some(error) = visitor.error {
        return Err(format!("{}: {error}", path.display()));
    }
    let mut imports = String::new();
    for (export, alias, entry) in visitor.imports {
        imports.push_str(&format!(
            "import {{ {export} as {alias} }} from '@tschk/moonshine-svelte/{entry}';\n"
        ));
    }
    if visitor.identity_used {
        imports.push_str(&format!(
            "function {}Identity<T>(value:T):T {{return value}}\n",
            visitor.prefix
        ));
    }
    if visitor.wrapper_used {
        imports.push_str(&format!("function {}WithCell<C extends {{peek():unknown}},R>(cell:C,write:(cell:C)=>R):R{{return write(cell)}}\n", visitor.prefix));
    }
    Ok(imports + &Codegen::new().with_source_text(source).build(program).code)
}

struct Lower<'a> {
    public_storage: HashMap<u32, String>,
    public_probes: HashMap<(u32, u32), u32>,
    constructor_types: HashMap<u32, (TSType<'a>, TSType<'a>)>,
    constructor_inits: Vec<ConstructorInit>,
    private_accesses: HashMap<u32, PrivateAccess>,
    identity_used: bool,
    wrapper_used: bool,
    allocator: &'a Allocator,
    fields: HashMap<u32, Rune>,
    prefix: String,
    target: MoonshineModuleTarget,
    serial: usize,
    imports: Vec<(&'static str, String, &'static str)>,
    error: Option<String>,
}
impl<'a> VisitMut<'a> for Lower<'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        if matches!(self.target, MoonshineModuleTarget::Dom) {
            if let Some(storage) = self
                .public_probes
                .get(&(expression.span().start, expression.span().end))
                .and_then(|field| self.public_storage.get(field))
                .cloned()
            {
                walk_mut::walk_expression(self, expression);
                // The brand guard preserves computed-getter dispatch before the
                // physical field exists; the raw read preserves TS initialization checks.
                *expression = self.template(
                    &format!("((#{storage} in this && this.#{storage}), CELL)"),
                    expression.clone_in(self.allocator),
                    None,
                );
                return;
            }
        }
        if let Expression::AssignmentExpression(assignment) = expression {
            // Its cloned initializer is visited at the planned field, then moved
            // back to this exact statement after the existing field lowerer runs.
            if self
                .constructor_inits
                .iter()
                .any(|init| init.assignment == assignment.span.start)
            {
                return;
            }

            // TypeScript wrappers do not change the assignment target. Resolve
            // before visiting children, or a wrapped target becomes a value read.
            let member = match &assignment.left {
                AssignmentTarget::PrivateFieldExpression(member) => Some(member),
                _ => assignment.left.get_expression().and_then(|expression| {
                    match expression.get_inner_expression() {
                        Expression::PrivateFieldExpression(member) => Some(member),
                        _ => None,
                    }
                }),
            };
            if let Some(member) = member {
                if let Some(access) = self.access(member) {
                    if member.optional {
                        self.error =
                            Some("unsupported optional private rune assignment receiver".into());
                        return;
                    }
                    if !assignment.operator.is_assign()
                        && !matches!(
                            member.object.get_inner_expression(),
                            Expression::ThisExpression(_)
                        )
                    {
                        self.error = Some("unsupported non-this private rune compound/logical receiver; a single-evaluation cell capture is required".into());
                        return;
                    }
                    if assignment.left.get_expression().is_some() {
                        let member = member.clone_in(self.allocator);
                        if matches!(access.rune, Rune::State) {
                            *expression =
                                self.wrapped_state_assignment(assignment, &member, access);
                        } else {
                            // Preserve the authored type assertion/satisfies check on .value.
                            walk_mut::walk_assignment_expression(self, assignment);
                            if access.constructor
                                && matches!(access.rune, Rune::Raw)
                                && !assignment.operator.is_assign()
                            {
                                let mut read = match &assignment.left {
                                    AssignmentTarget::TSAsExpression(value) => {
                                        Expression::TSAsExpression(value.clone_in(self.allocator))
                                    }
                                    AssignmentTarget::TSSatisfiesExpression(value) => {
                                        Expression::TSSatisfiesExpression(
                                            value.clone_in(self.allocator),
                                        )
                                    }
                                    AssignmentTarget::TSNonNullExpression(value) => {
                                        Expression::TSNonNullExpression(
                                            value.clone_in(self.allocator),
                                        )
                                    }
                                    AssignmentTarget::TSTypeAssertion(value) => {
                                        Expression::TSTypeAssertion(value.clone_in(self.allocator))
                                    }
                                    _ => unreachable!("wrapped target"),
                                };
                                let cell = Expression::PrivateFieldExpression(
                                    member.clone_in(self.allocator),
                                );
                                *read.get_inner_expression_mut() =
                                    self.template("CELL.peek()", cell, None);
                                let logical = assignment.operator.to_logical_operator();
                                if let Some(operator) = assignment.operator.to_binary_operator() {
                                    assignment.right = self.template(
                                        &format!("CELL {} RHS", operator.as_str()),
                                        read.clone_in(self.allocator),
                                        Some(assignment.right.clone_in(self.allocator)),
                                    );
                                }
                                assignment.operator =
                                    oxc_syntax::operator::AssignmentOperator::Assign;
                                if let Some(operator) = logical {
                                    let write = Expression::AssignmentExpression(
                                        assignment.clone_in(self.allocator),
                                    );
                                    *expression = self.template(
                                        &format!("CELL {} RHS", operator.as_str()),
                                        read,
                                        Some(write),
                                    );
                                }
                            }
                        }
                        return;
                    }
                    let mut cell =
                        Expression::PrivateFieldExpression(member.clone_in(self.allocator));
                    if let Expression::PrivateFieldExpression(member) = &mut cell {
                        self.visit_expression(&mut member.object);
                    }
                    let mut rhs = assignment.right.clone_in(self.allocator);
                    self.visit_expression(&mut rhs);
                    if assignment.operator.is_assign() {
                        *expression = if matches!(access.rune, Rune::State) {
                            self.template("CELL.assign(RHS)", cell, Some(rhs))
                        } else {
                            self.template("CELL.value=RHS", cell, Some(rhs))
                        };
                        return;
                    }
                    let read =
                        if access.constructor && matches!(access.rune, Rune::State | Rune::Raw) {
                            "CELL.peek()"
                        } else {
                            "CELL.value"
                        };
                    let write = if matches!(access.rune, Rune::State) {
                        "CELL.assign(RHS)"
                    } else {
                        "(CELL.value=RHS)"
                    };
                    if let Some(operator) = assignment.operator.to_logical_operator() {
                        *expression = self.template(
                            &format!("{read} {} {write}", operator.as_str()),
                            cell,
                            Some(rhs),
                        );
                        return;
                    }
                    if let Some(operator) = assignment.operator.to_binary_operator() {
                        let write = if matches!(access.rune, Rune::State) {
                            format!("CELL.assign({read} {} RHS)", operator.as_str())
                        } else {
                            format!("CELL.value={read} {} RHS", operator.as_str())
                        };
                        *expression = self.template(&write, cell, Some(rhs));
                        return;
                    }
                    self.error = Some("unsupported private rune assignment operator".into());
                    return;
                }
            }
        }
        if let Expression::PrivateFieldExpression(member) = expression {
            if let Some(access) = self.access(member) {
                if member.optional {
                    self.error = Some("unsupported optional private rune receiver".into());
                    return;
                }
                self.visit_expression(&mut member.object);
                let cell = expression.clone_in(self.allocator);
                *expression = self.template(
                    if access.constructor && matches!(access.rune, Rune::State | Rune::Raw) {
                        "CELL.peek()"
                    } else {
                        "CELL.value"
                    },
                    cell,
                    None,
                );
                return;
            }
        }
        walk_mut::walk_expression(self, expression);
    }

    fn visit_chain_expression(&mut self, chain: &mut ChainExpression<'a>) {
        if let ChainElement::PrivateFieldExpression(member) = &chain.expression {
            if self.access(member).is_some() {
                self.error = Some("unsupported optional private rune receiver".into());
                return;
            }
        }
        walk_mut::walk_chain_expression(self, chain);
    }

    fn visit_call_expression(&mut self, call: &mut CallExpression<'a>) {
        let access = match call.callee.get_inner_expression() {
            Expression::PrivateFieldExpression(member) => self.access(member),
            _ => None,
        };
        if let Some(access) = access {
            if !self.validate_call(access) {
                return;
            }
            walk_mut::walk_call_expression(self, call);
            self.identity_used = true;
            call.callee = self.template(
                &format!("{}Identity(CELL)", self.prefix),
                call.callee.clone_in(self.allocator),
                None,
            );
        } else {
            walk_mut::walk_call_expression(self, call);
        }
    }
    fn visit_tagged_template_expression(&mut self, tag: &mut TaggedTemplateExpression<'a>) {
        let access = match tag.tag.get_inner_expression() {
            Expression::PrivateFieldExpression(member) => self.access(member),
            _ => None,
        };
        if let Some(access) = access {
            if !self.validate_call(access) {
                return;
            }
            walk_mut::walk_tagged_template_expression(self, tag);
            self.identity_used = true;
            tag.tag = self.template(
                &format!("{}Identity(CELL)", self.prefix),
                tag.tag.clone_in(self.allocator),
                None,
            );
        } else {
            walk_mut::walk_tagged_template_expression(self, tag);
        }
    }

    fn visit_simple_assignment_target(&mut self, target: &mut SimpleAssignmentTarget<'a>) {
        // Updates and destructuring targets use this visitor too. In particular,
        // a wrapped constructor update must remain a .value lvalue, not peek().
        let member = match &*target {
            SimpleAssignmentTarget::PrivateFieldExpression(member) => Some(member),
            _ => target.get_expression().and_then(|expression| {
                match expression.get_inner_expression() {
                    Expression::PrivateFieldExpression(member) => Some(member),
                    _ => None,
                }
            }),
        };
        if let Some(member) = member {
            if self.access(member).is_some() {
                if member.optional {
                    self.error =
                        Some("unsupported optional private rune assignment receiver".into());
                    return;
                }
                let mut cell = Expression::PrivateFieldExpression(member.clone_in(self.allocator));
                if let Expression::PrivateFieldExpression(member) = &mut cell {
                    self.visit_expression(&mut member.object);
                }
                let value = self.template("CELL.value", cell, None);
                if let Some(inner) = target.get_expression_mut() {
                    *inner.get_inner_expression_mut() = value;
                } else if let Expression::StaticMemberExpression(member) = value {
                    *target = SimpleAssignmentTarget::StaticMemberExpression(member);
                }
                return;
            }
        }
        walk_mut::walk_simple_assignment_target(self, target);
    }

    fn visit_class(&mut self, class: &mut Class<'a>) {
        let has_fields=class.body.body.iter().any(|element|matches!(element,ClassElement::PropertyDefinition(field) if self.fields.contains_key(&field.span.start))) || self.constructor_inits.iter().any(|init| init.body == class.body.span.start);
        if has_fields && (!class.decorators.is_empty() || class.declare || class.r#abstract) {
            self.error =
                Some("unsupported decorated, declared or abstract Svelte rune class".into());
            return;
        }
        walk_mut::walk_class(self, class);
    }

    fn visit_class_body(&mut self, body: &mut ClassBody<'a>) {
        let inits: Vec<_> = self
            .constructor_inits
            .iter()
            .filter(|init| init.body == body.span.start)
            .cloned()
            .collect();
        for init in &inits {
            let private = body.body.iter().any(|element| matches!(element,ClassElement::PropertyDefinition(field) if field.span.start==init.field && matches!(field.key,PropertyKey::PrivateIdentifier(_))));
            if !private {
                self.serial += 1;
                self.public_storage
                    .insert(init.field, format!("{}{}", self.prefix, self.serial));
            }
        }
        let mut synthetic_index = 0;
        for init in &inits {
            let Some(value) = constructor_assignment(body, init.assignment)
                .map(|assignment| assignment.right.clone_in(self.allocator))
            else {
                self.error = Some("missing planned constructor assignment".into());
                return;
            };
            if init.declared {
                let Some(field) = body.body.iter_mut().find_map(|element| match element {
                    ClassElement::PropertyDefinition(field) if field.span.start == init.field => {
                        Some(field)
                    }
                    _ => None,
                }) else {
                    self.error = Some("missing constructor rune field declaration".into());
                    return;
                };
                field.value = Some(value);
            } else {
                let text = self
                    .allocator
                    .alloc_str(&format!("class C {{ {}; }}", init.name));
                let mut parsed = Parser::new(self.allocator, text, SourceType::ts()).parse();
                if !parsed.errors.is_empty() {
                    self.error = Some("unsupported constructor rune property name".into());
                    return;
                }
                SyntheticSpans.visit_program(&mut parsed.program);
                let Statement::ClassDeclaration(class) = &mut parsed.program.body[0] else {
                    unreachable!("constructor declaration skeleton")
                };
                let ClassElement::PropertyDefinition(mut field) = class.body.body.remove(0) else {
                    unreachable!("constructor field skeleton")
                };
                field.span = Span::new(init.field, init.field);
                field.value = Some(value);
                // Pinned Svelte inserts undeclared public backing/accessors before
                // original members, so later dynamic computed methods still win.
                body.body
                    .insert(synthetic_index, ClassElement::PropertyDefinition(field));
                synthetic_index += 1;
            }
        }
        for element in &body.body {
            let ClassElement::PropertyDefinition(field) = element else {
                continue;
            };
            if !self.fields.contains_key(&field.span.start) {
                continue;
            }
            let PropertyKey::StaticIdentifier(key) = &field.key else {
                continue;
            };
            let name = key.name.as_str();
            let collisions = body
                .body
                .iter()
                .filter(|element| match element {
                    ClassElement::PropertyDefinition(other) => {
                        !other.r#static && other.key.static_name().is_some_and(|key| key == name)
                    }
                    ClassElement::MethodDefinition(other) => {
                        !other.r#static
                            && !other.computed
                            && other.key.static_name().is_some_and(|key| key == name)
                    }
                    ClassElement::AccessorProperty(other) => {
                        !other.r#static && other.key.static_name().is_some_and(|key| key == name)
                    }
                    _ => false,
                })
                .count();
            if collisions > 1 {
                self.error = Some(format!(
                    "unsupported Svelte class rune field collision: {name}"
                ));
                return;
            }
        }
        // Preserve the authored type as a union member on constructor storage.
        // This retains TypeScript's exact definite-initialization flags (including
        // void versus undefined); cell access and writes retain the exact cell shape.
        for init in &inits {
            if let Some(field) = body.body.iter().find_map(|element| match element {
                ClassElement::PropertyDefinition(field) if field.span.start == init.field => {
                    Some(field)
                }
                _ => None,
            }) {
                if let Some(annotation) = &field.type_annotation {
                    let rune = self.fields[&init.field];
                    let private = matches!(field.key, PropertyKey::PrivateIdentifier(_));
                    let cell = self.cell_type(&annotation.type_annotation, rune, private);
                    self.constructor_types.insert(
                        init.field,
                        (annotation.type_annotation.clone_in(self.allocator), cell),
                    );
                }
            }
        }
        // Recurse first so nested class fields are transformed once.
        walk_mut::walk_class_body(self, body);
        let mut next = oxc_allocator::Vec::new_in(self.allocator);
        let mut accessors = oxc_allocator::Vec::new_in(self.allocator);
        for element in body.body.drain(..) {
            let ClassElement::PropertyDefinition(mut field) = element else {
                next.push(element);
                continue;
            };
            let Some(rune) = self.fields.get(&field.span.start).copied() else {
                next.push(ClassElement::PropertyDefinition(field));
                continue;
            };
            if matches!(field.key, PropertyKey::PrivateIdentifier(_)) {
                self.lower_private_field(&mut field, rune);
                next.push(ClassElement::PropertyDefinition(field));
                continue;
            }
            let PropertyKey::StaticIdentifier(key) = &field.key else {
                unreachable!("validated public key")
            };
            let name = key.name.to_string();
            let original = field.value.take().expect("validated field initializer");
            let Expression::CallExpression(call) = original.get_inner_expression() else {
                unreachable!("validated rune call")
            };
            let mut argument = call
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .map(|argument| argument.clone_in(self.allocator))
                .unwrap_or_else(|| {
                    let ast = AstBuilder::new(self.allocator);
                    ast.expression_unary(
                        SPAN,
                        UnaryOperator::Void,
                        ast.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                    )
                });
            let ast = AstBuilder::new(self.allocator);
            let mut type_arguments = call.type_arguments.clone_in(self.allocator);
            if call.arguments.is_empty() {
                if let Some(types) = &mut type_arguments {
                    let ty = types.params[0].clone_in(self.allocator);
                    types.params[0] = ast.ts_type_union_type(
                        SPAN,
                        ast.vec_from_iter([ty, ast.ts_type_undefined_keyword(SPAN)]),
                    );
                }
            }
            if matches!(self.target, MoonshineModuleTarget::Ssr)
                && matches!(rune, Rune::State | Rune::Raw)
            {
                if let Some(types) = &type_arguments {
                    let ty = &types.params[0];
                    if field.type_annotation.is_none() {
                        field.type_annotation =
                            Some(ast.alloc_ts_type_annotation(SPAN, ty.clone_in(self.allocator)));
                    }
                    // Validate the initializer while retaining the rune's explicit return type.
                    // `satisfies` alone narrows to the argument and can hide an invalid outer field annotation.
                    argument = ast.expression_ts_as(
                        SPAN,
                        ast.expression_ts_satisfies(SPAN, argument, ty.clone_in(self.allocator)),
                        ty.clone_in(self.allocator),
                    );
                }
                field.value = Some(argument);
                next.push(ClassElement::PropertyDefinition(field));
                continue;
            }
            let storage = if let Some(storage) = self.public_storage.get(&field.span.start) {
                storage.clone()
            } else {
                self.serial += 1;
                format!("{}{}", self.prefix, self.serial)
            };

            let (export, entry) = match (self.target, rune) {
                (MoonshineModuleTarget::Ssr, _) => ("createServerDerived", "ssr"),
                (_, Rune::State) => ("createState", "runes"),
                (_, Rune::Raw) => ("createRawState", "runes"),
                _ => ("createDerived", "runes"),
            };
            let helper = if let Some((_, alias, _)) = self
                .imports
                .iter()
                .find(|(name, _, path)| *name == export && *path == entry)
            {
                alias.clone()
            } else {
                let alias = format!("{}Cell{}", self.prefix, self.imports.len());
                self.imports.push((export, alias.clone(), entry));
                alias
            };
            let annotation = if field.type_annotation.is_some() {
                ": { value: unknown }"
            } else {
                ""
            };
            let access = if self.constructor_types.contains_key(&field.span.start) {
                format!("(this.#{storage} as {{value:unknown}})")
            } else {
                format!("this.#{storage}")
            };
            let text=self.allocator.alloc_str(&format!("class C {{ #{storage}{annotation} = {helper}(undefined); get {name}() {{return {access}.value}} set {name}(value) {{{access}.value=value}} }}"));
            let mut skeleton = Parser::new(self.allocator, text, SourceType::ts()).parse();
            if !skeleton.errors.is_empty() {
                self.error = Some("invalid generated class field skeleton".into());
                return;
            }
            SyntheticSpans.visit_program(&mut skeleton.program);
            let Statement::ClassDeclaration(class) = &mut skeleton.program.body[0] else {
                unreachable!("class skeleton")
            };
            if let Some(annotation) = &field.type_annotation {
                TypeFill {
                    allocator: self.allocator,
                    original: annotation.type_annotation.clone_in(self.allocator),
                }
                .visit_class(class);
            }
            let ClassElement::PropertyDefinition(backing) = &mut class.body.body[0] else {
                unreachable!("backing skeleton")
            };
            backing.span = field.span;
            if let Some(annotation) = &field.type_annotation {
                let TSType::TSTypeLiteral(literal) = &mut backing
                    .type_annotation
                    .as_mut()
                    .expect("typed backing skeleton")
                    .type_annotation
                else {
                    unreachable!("backing type literal")
                };
                let TSSignature::TSPropertySignature(property) = &mut literal.members[0] else {
                    unreachable!("backing value type")
                };
                property.type_annotation = Some(annotation.clone_in(self.allocator));
            }
            let Expression::CallExpression(helper_call) =
                backing.value.as_mut().expect("helper call")
            else {
                unreachable!("helper skeleton")
            };
            let argument = if matches!(rune, Rune::Derived) {
                let mut arrow = Parser::new(self.allocator, "()=>undefined", SourceType::ts())
                    .parse_expression()
                    .expect("arrow skeleton");
                SyntheticSpans.visit_expression(&mut arrow);
                let Expression::ArrowFunctionExpression(function) = &mut arrow else {
                    unreachable!("arrow")
                };
                let Statement::ExpressionStatement(statement) = &mut function.body.statements[0]
                else {
                    unreachable!("expression arrow body")
                };
                statement.expression = argument;
                arrow
            } else {
                argument
            };
            helper_call.arguments[0] = Argument::from(argument);
            helper_call.type_arguments = type_arguments;
            if helper_call.type_arguments.is_none() {
                if let Some(annotation) = &field.type_annotation {
                    let ast = AstBuilder::new(self.allocator);
                    helper_call.type_arguments = Some(ast.alloc_ts_type_parameter_instantiation(
                        SPAN,
                        ast.vec1(annotation.type_annotation.clone_in(self.allocator)),
                    ));
                }
            }
            for replacement in class.body.body.drain(..) {
                if matches!(replacement, ClassElement::MethodDefinition(_)) {
                    accessors.push(replacement)
                } else {
                    next.push(replacement)
                }
            }
        }
        accessors.extend(next);
        body.body = accessors;
        for init in &inits {
            let Some(index) = body.body.iter().position(|element| matches!(element, ClassElement::PropertyDefinition(field) if field.span.start == init.field)) else { self.error=Some("missing lowered constructor storage".into()); return };
            let ClassElement::PropertyDefinition(field) = &mut body.body[index] else {
                unreachable!("located constructor storage")
            };
            let Some(mut value) = field.value.take() else {
                self.error = Some("missing constructor storage initializer".into());
                return;
            };
            let ast = AstBuilder::new(self.allocator);
            if matches!(self.target, MoonshineModuleTarget::Dom) {
                if let Some((original, cell)) = self.constructor_types.get(&init.field) {
                    value = ast.expression_ts_satisfies(SPAN, value, cell.clone_in(self.allocator));
                    field.type_annotation = Some(ast.alloc_ts_type_annotation(
                        SPAN,
                        ast.ts_type_union_type(
                            SPAN,
                            ast.vec_from_iter([
                                cell.clone_in(self.allocator),
                                original.clone_in(self.allocator),
                            ]),
                        ),
                    ));
                }
            }
            let target = match &field.key {
                PropertyKey::PrivateIdentifier(key) => {
                    AssignmentTarget::PrivateFieldExpression(ast.alloc_private_field_expression(
                        SPAN,
                        ast.expression_this(SPAN),
                        (**key).clone_in(self.allocator),
                        false,
                    ))
                }
                PropertyKey::StaticIdentifier(key) => {
                    AssignmentTarget::StaticMemberExpression(ast.alloc_static_member_expression(
                        SPAN,
                        ast.expression_this(SPAN),
                        (**key).clone_in(self.allocator),
                        false,
                    ))
                }
                _ => {
                    self.error = Some("unsupported constructor storage key".into());
                    return;
                }
            };
            if !init.declared {
                SyntheticSpans.visit_span(&mut field.span);
            }
            // SSR state/raw undeclared public properties are created only at the
            // authored assignment; adding a runtime field changes key insertion order.
            if !init.declared
                && matches!(self.target, MoonshineModuleTarget::Ssr)
                && matches!(self.fields.get(&init.field), Some(Rune::State | Rune::Raw))
            {
                body.body.remove(index);
            }
            let Some(assignment) = constructor_assignment_mut(body, init.assignment) else {
                self.error = Some("missing constructor initialization statement".into());
                return;
            };
            assignment.left = target;
            assignment.right = value;
        }
    }
}

/// Writes in constructor control flow count, but deferred function/class bodies
/// do not. Destructuring targets deliberately remain outside this pinned rule.
#[derive(Default)]
struct ConstructorWrites {
    writes: Vec<(u32, String, bool)>,
}
fn constructor_member(expression: &Expression<'_>) -> Option<(String, bool)> {
    match expression.get_inner_expression() {
        Expression::StaticMemberExpression(member)
            if matches!(
                member.object.get_inner_expression(),
                Expression::ThisExpression(_)
            ) =>
        {
            Some((member.property.name.to_string(), false))
        }
        Expression::PrivateFieldExpression(member)
            if matches!(
                member.object.get_inner_expression(),
                Expression::ThisExpression(_)
            ) =>
        {
            Some((member.field.name.to_string(), true))
        }
        Expression::ComputedMemberExpression(member)
            if matches!(
                member.object.get_inner_expression(),
                Expression::ThisExpression(_)
            ) =>
        {
            match member.expression.get_inner_expression() {
                Expression::StringLiteral(key) => Some((key.value.to_string(), false)),
                _ => None,
            }
        }
        _ => None,
    }
}
impl<'a> Visit<'a> for ConstructorWrites {
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, class: &Class<'a>) {
        // Extends is evaluated in the enclosing constructor immediately. Class
        // bodies (including computed method keys) stay outside the pinned rule.
        if let Some(super_class) = &class.super_class {
            self.visit_expression(super_class);
        }
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        let key = match &assignment.left {
            AssignmentTarget::StaticMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                Some((member.property.name.to_string(), false))
            }
            AssignmentTarget::PrivateFieldExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                Some((member.field.name.to_string(), true))
            }
            AssignmentTarget::ComputedMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                match member.expression.get_inner_expression() {
                    Expression::StringLiteral(key) => Some((key.value.to_string(), false)),
                    _ => None,
                }
            }
            other => other.get_expression().and_then(constructor_member),
        };
        if let Some((name, private)) = key {
            self.writes.push((assignment.span.start, name, private))
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        let key = match &update.argument {
            SimpleAssignmentTarget::StaticMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                Some((member.property.name.to_string(), false))
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                Some((member.field.name.to_string(), true))
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member)
                if matches!(
                    member.object.get_inner_expression(),
                    Expression::ThisExpression(_)
                ) =>
            {
                match member.expression.get_inner_expression() {
                    Expression::StringLiteral(key) => Some((key.value.to_string(), false)),
                    _ => None,
                }
            }
            other => other.get_expression().and_then(constructor_member),
        };
        if let Some((name, private)) = key {
            self.writes.push((update.span.start, name, private))
        }
        walk::walk_update_expression(self, update);
    }
}

fn rune_call(call: &CallExpression<'_>, semantic: &Semantic<'_>) -> Option<Rune> {
    let (id, member) = match call.callee.get_inner_expression() {
        Expression::Identifier(id) => (id, None),
        Expression::StaticMemberExpression(member) => {
            let Expression::Identifier(id) = member.object.get_inner_expression() else {
                return None;
            };
            (id, Some(member.property.name.as_str()))
        }
        _ => return None,
    };
    if has_runtime_binding(semantic, id) {
        return None;
    }
    match (id.name.as_str(), member) {
        ("$state", None) => Some(Rune::State),
        ("$state", Some("raw")) => Some(Rune::Raw),
        ("$derived", None) => Some(Rune::Derived),
        ("$derived", Some("by")) => Some(Rune::By),
        _ => None,
    }
}
fn validate_constructor_call(
    call: &CallExpression<'_>,
    rune: Rune,
    path: &Path,
) -> Result<(), String> {
    if call.optional
        || matches!(call.callee.get_inner_expression(),Expression::StaticMemberExpression(member) if member.optional)
        || call.arguments.len() > 1
        || call
            .arguments
            .first()
            .is_some_and(|arg| arg.as_expression().is_none())
        || (call.arguments.is_empty() && matches!(rune, Rune::Derived | Rune::By))
        || (call.type_arguments.is_some() && matches!(rune, Rune::Derived | Rune::By))
        || call
            .type_arguments
            .as_ref()
            .is_some_and(|types| types.params.len() != 1)
    {
        return Err(format!(
            "{}: unsupported constructor rune initializer arguments",
            path.display()
        ));
    }
    if matches!(rune, Rune::By) {
        let Some(argument) = call.arguments.first().and_then(Argument::as_expression) else {
            return Err(format!(
                "{}: missing constructor derived callback",
                path.display()
            ));
        };
        if !matches!(
            argument,
            Expression::Identifier(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::FunctionExpression(_)
        ) || matches!(argument,Expression::ArrowFunctionExpression(function) if function.r#async)
            || matches!(argument,Expression::FunctionExpression(function) if function.r#async||function.generator)
        {
            return Err(format!(
                "{}: unsupported constructor derived callback",
                path.display()
            ));
        }
    }
    Ok(())
}
fn constructor_assignment<'b, 'a>(
    body: &'b ClassBody<'a>,
    start: u32,
) -> Option<&'b AssignmentExpression<'a>> {
    body.body.iter().find_map(|element| {
        let ClassElement::MethodDefinition(method) = element else {
            return None;
        };
        if method.kind != MethodDefinitionKind::Constructor {
            return None;
        };
        method
            .value
            .body
            .as_ref()?
            .statements
            .iter()
            .find_map(|statement| match statement {
                Statement::ExpressionStatement(statement) => match &statement.expression {
                    Expression::AssignmentExpression(assignment)
                        if assignment.span.start == start =>
                    {
                        Some(assignment.as_ref())
                    }
                    _ => None,
                },
                _ => None,
            })
    })
}
fn constructor_assignment_mut<'b, 'a>(
    body: &'b mut ClassBody<'a>,
    start: u32,
) -> Option<&'b mut AssignmentExpression<'a>> {
    body.body.iter_mut().find_map(|element| {
        let ClassElement::MethodDefinition(method) = element else {
            return None;
        };
        if method.kind != MethodDefinitionKind::Constructor {
            return None;
        };
        method
            .value
            .body
            .as_mut()?
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                Statement::ExpressionStatement(statement) => match &mut statement.expression {
                    Expression::AssignmentExpression(assignment)
                        if assignment.span.start == start =>
                    {
                        Some(assignment.as_mut())
                    }
                    _ => None,
                },
                _ => None,
            })
    })
}

fn private_callable(field: &PropertyDefinition<'_>, rune: Rune) -> bool {
    let call = match &field.value {
        Some(Expression::CallExpression(call)) => Some(call.as_ref()),
        _ => None,
    };
    private_callable_initializer(field, rune, call)
}
fn private_callable_initializer(
    field: &PropertyDefinition<'_>,
    rune: Rune,
    call: Option<&CallExpression<'_>>,
) -> bool {
    fn safe_this(param: Option<&TSThisParameter<'_>>) -> bool {
        param.is_none_or(|param| {
            param.type_annotation.as_ref().is_some_and(|annotation| {
                matches!(
                    annotation.type_annotation,
                    TSType::TSVoidKeyword(_)
                        | TSType::TSUnknownKeyword(_)
                        | TSType::TSAnyKeyword(_)
                )
            })
        })
    }
    if let Some(annotation) = &field.type_annotation {
        return matches!(&annotation.type_annotation, TSType::TSFunctionType(function) if safe_this(function.this_param.as_deref()));
    }
    // A by callback's callable shape is not the derived VALUE's callable shape.
    if matches!(rune, Rune::By) {
        return false;
    }
    let Some(call) = call else { return false };
    match call
        .arguments
        .first()
        .and_then(Argument::as_expression)
        .map(Expression::get_inner_expression)
    {
        Some(Expression::ArrowFunctionExpression(_)) => true,
        Some(Expression::FunctionExpression(function)) => safe_this(function.this_param.as_deref()),
        _ => false,
    }
}

impl<'a> Lower<'a> {
    fn wrapped_state_assignment(
        &mut self,
        assignment: &AssignmentExpression<'a>,
        member: &PrivateFieldExpression<'a>,
        access: PrivateAccess,
    ) -> Expression<'a> {
        struct Suspends(bool);
        impl<'a> Visit<'a> for Suspends {
            fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
                self.0 = true;
            }
            fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
                self.0 = true;
            }
            fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
        }
        let mut suspends = Suspends(false);
        suspends.visit_expression(&assignment.right);
        if suspends.0 {
            // shortcut: wrapped state writes use synchronous callbacks; add inline lowering for await/yield.
            self.error =
                Some("unsupported await/yield RHS in wrapped private state assignment".into());
            return Expression::AssignmentExpression(
                AstBuilder::new(self.allocator).alloc(assignment.clone_in(self.allocator)),
            );
        }
        self.wrapper_used = true;
        let captured = format!("{}Captured", self.prefix);
        let assigned = format!("{}Assigned", self.prefix);
        let wrote = format!("{}Wrote", self.prefix);
        let ast = AstBuilder::new(self.allocator);
        let mut cell =
            Expression::PrivateFieldExpression(ast.alloc(member.clone_in(self.allocator)));
        if let Expression::PrivateFieldExpression(member) = &mut cell {
            self.visit_expression(&mut member.object);
        }
        let captured_cell = ast.expression_identifier(SPAN, self.allocator.alloc_str(&captured));
        let mut operation = assignment.clone_in(self.allocator);
        let mut read = match &assignment.left {
            AssignmentTarget::TSAsExpression(value) => {
                Expression::TSAsExpression(value.clone_in(self.allocator))
            }
            AssignmentTarget::TSSatisfiesExpression(value) => {
                Expression::TSSatisfiesExpression(value.clone_in(self.allocator))
            }
            AssignmentTarget::TSNonNullExpression(value) => {
                Expression::TSNonNullExpression(value.clone_in(self.allocator))
            }
            AssignmentTarget::TSTypeAssertion(value) => {
                Expression::TSTypeAssertion(value.clone_in(self.allocator))
            }
            _ => unreachable!("wrapped target"),
        };
        if let Some(inner) = operation.left.get_expression_mut() {
            *inner.get_inner_expression_mut() =
                self.template("CELL.value", captured_cell.clone_in(self.allocator), None);
        }
        self.visit_expression(&mut operation.right);
        let logical = assignment.operator.to_logical_operator();
        let binary = assignment.operator.to_binary_operator();
        *read.get_inner_expression_mut() = self.template(
            if access.constructor {
                "CELL.peek()"
            } else {
                "CELL.value"
            },
            captured_cell,
            None,
        );
        if logical.is_some() || (access.constructor && binary.is_some()) {
            operation.operator = oxc_syntax::operator::AssignmentOperator::Assign;
            if let Some(operator) = binary {
                operation.right = self.template(
                    &format!("CELL {} RHS", operator.as_str()),
                    read.clone_in(self.allocator),
                    Some(operation.right.clone_in(self.allocator)),
                );
            }
        }
        let mut operation = Expression::AssignmentExpression(ast.alloc(operation));
        if let Some(operator) = logical {
            // Short-circuit reads can be historical: only return live peek after a write.
            operation = self.template(
                &format!("CELL {} ({wrote}=true,RHS)", operator.as_str()),
                read,
                Some(operation),
            );
        }
        let body = if logical.is_some() {
            format!("let {wrote}=false;const {assigned}=RHS;return ({wrote}?{captured}.peek():{assigned}) as unknown as typeof {assigned};")
        } else {
            format!(
                "const {assigned}=RHS;return {captured}.peek() as unknown as typeof {assigned};"
            )
        };
        self.template(
            &format!("{}WithCell(CELL,({captured})=>{{{body}}})", self.prefix),
            cell,
            Some(operation),
        )
    }
    fn validate_call(&mut self, access: PrivateAccess) -> bool {
        if access.constructor && matches!(access.rune, Rune::State | Rune::Raw) {
            self.error =
                Some("unsupported constructor private state/raw call or tag receiver".into());
            return false;
        }
        if !access.callable {
            self.error = Some("unsupported private rune callable type; first cohort requires inline no-this callable annotation or inferred arrow/function (aliases/imports remain unsupported)".into());
            return false;
        }
        true
    }
    fn private_active(&self, access: PrivateAccess) -> bool {
        matches!(self.target, MoonshineModuleTarget::Dom)
            || matches!(access.rune, Rune::Derived | Rune::By)
    }
    fn template(
        &mut self,
        text: &str,
        cell: Expression<'a>,
        rhs: Option<Expression<'a>>,
    ) -> Expression<'a> {
        let cell = if matches!(self.target, MoonshineModuleTarget::Dom) {
            if let Expression::PrivateFieldExpression(member) = &cell {
                self.private_accesses
                    .get(&member.field.span.start)
                    .and_then(|access| self.constructor_types.get(&access.field))
                    .map(|(_, ty)| {
                        AstBuilder::new(self.allocator).expression_ts_as(
                            SPAN,
                            cell.clone_in(self.allocator),
                            ty.clone_in(self.allocator),
                        )
                    })
                    .unwrap_or(cell)
            } else {
                cell
            }
        } else {
            cell
        };
        let text = self.allocator.alloc_str(text);
        let parsed = Parser::new(self.allocator, text, SourceType::ts()).parse_expression();
        let Ok(mut expression) = parsed else {
            self.error = Some("invalid private class AST skeleton".into());
            return cell;
        };
        SyntheticSpans.visit_expression(&mut expression);
        struct Fill<'a> {
            allocator: &'a Allocator,
            cell: Expression<'a>,
            rhs: Option<Expression<'a>>,
        }
        impl<'a> VisitMut<'a> for Fill<'a> {
            fn visit_expression(&mut self, expression: &mut Expression<'a>) {
                if let Expression::Identifier(id) = expression {
                    if id.name == "CELL" {
                        *expression = self.cell.clone_in(self.allocator);
                        return;
                    }
                    if id.name == "RHS" {
                        if let Some(rhs) = &self.rhs {
                            *expression = rhs.clone_in(self.allocator);
                            return;
                        }
                    }
                }
                walk_mut::walk_expression(self, expression);
            }
        }
        Fill {
            allocator: self.allocator,
            cell,
            rhs,
        }
        .visit_expression(&mut expression);
        expression
    }
    fn cell_type(&mut self, original: &TSType<'a>, rune: Rune, private: bool) -> TSType<'a> {
        let skeleton = self.template(
            "null as {value:unknown;assign(next:unknown):unknown;peek():unknown}",
            AstBuilder::new(self.allocator).expression_identifier(SPAN, "unused"),
            None,
        );
        let Expression::TSAsExpression(skeleton) = skeleton else {
            unreachable!("cell type skeleton")
        };
        let mut ty = skeleton.unbox().type_annotation;
        TypeFill {
            allocator: self.allocator,
            original: original.clone_in(self.allocator),
        }
        .visit_ts_type(&mut ty);
        if let TSType::TSTypeLiteral(literal) = &mut ty {
            if !private {
                literal.members.truncate(1)
            } else {
                if !matches!(rune, Rune::State) {
                    literal.members.remove(1);
                }
                if matches!(rune, Rune::Derived | Rune::By) {
                    literal.members.pop();
                }
            }
        }
        ty
    }
    fn access(&self, member: &PrivateFieldExpression<'_>) -> Option<PrivateAccess> {
        self.private_accesses
            .get(&member.field.span.start)
            .copied()
            .filter(|access| self.private_active(*access))
    }
    fn lower_private_field(&mut self, field: &mut PropertyDefinition<'a>, rune: Rune) {
        let Some(Expression::CallExpression(call)) = field.value.take() else {
            self.error = Some("invalid planned private initializer".into());
            return;
        };
        let ast = AstBuilder::new(self.allocator);
        let argument = call
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .map(|argument| argument.clone_in(self.allocator))
            .unwrap_or_else(|| {
                self.template("void 0", ast.expression_identifier(SPAN, "unused"), None)
            });
        let mut types = call.type_arguments.clone_in(self.allocator);
        if call.arguments.is_empty() {
            if let Some(types) = &mut types {
                let ty = types.params[0].clone_in(self.allocator);
                types.params[0] = ast.ts_type_union_type(
                    SPAN,
                    ast.vec_from_iter([ty, ast.ts_type_undefined_keyword(SPAN)]),
                );
            }
        }
        if matches!(self.target, MoonshineModuleTarget::Ssr)
            && matches!(rune, Rune::State | Rune::Raw)
        {
            let mut argument = argument;
            if let Some(types) = &types {
                if field.type_annotation.is_none() {
                    field.type_annotation =
                        Some(ast.alloc_ts_type_annotation(
                            SPAN,
                            types.params[0].clone_in(self.allocator),
                        ));
                }
                argument = ast.expression_ts_as(
                    SPAN,
                    ast.expression_ts_satisfies(
                        SPAN,
                        argument,
                        types.params[0].clone_in(self.allocator),
                    ),
                    types.params[0].clone_in(self.allocator),
                );
            }
            field.value = Some(argument);
            return;
        }
        let (export, entry) = match (self.target, rune) {
            (MoonshineModuleTarget::Ssr, _) => ("createServerDerived", "ssr"),
            (_, Rune::State) => ("createPrivateState", "runes"),
            (_, Rune::Raw) => ("createPrivateRawState", "runes"),
            _ => ("createDerived", "runes"),
        };
        let alias = if let Some((_, alias, _)) = self
            .imports
            .iter()
            .find(|(name, _, path)| *name == export && *path == entry)
        {
            alias.clone()
        } else {
            let alias = format!("{}Cell{}", self.prefix, self.imports.len());
            self.imports.push((export, alias.clone(), entry));
            alias
        };
        let argument = if matches!(rune, Rune::Derived) {
            self.template("()=>CELL", argument, None)
        } else {
            argument
        };
        let mut expression = self.template(&format!("{alias}(CELL)"), argument, None);
        if let Expression::CallExpression(helper) = &mut expression {
            helper.type_arguments = types;
            if helper.type_arguments.is_none() {
                if let Some(annotation) = &field.type_annotation {
                    helper.type_arguments = Some(ast.alloc_ts_type_parameter_instantiation(
                        SPAN,
                        ast.vec1(annotation.type_annotation.clone_in(self.allocator)),
                    ));
                }
            }
        }
        if let Some(annotation) = &mut field.type_annotation {
            let original = annotation.type_annotation.clone_in(self.allocator);
            let skeleton = self.template(
                "null as {value:unknown;assign(next:unknown):unknown;peek():unknown}",
                ast.expression_identifier(SPAN, "unused"),
                None,
            );
            if let Expression::TSAsExpression(skeleton) = skeleton {
                let mut ty = skeleton.unbox().type_annotation;
                TypeFill {
                    allocator: self.allocator,
                    original,
                }
                .visit_ts_type(&mut ty);
                // Raw/derived cells do not expose assign: use the smaller matching shape.
                if let TSType::TSTypeLiteral(literal) = &mut ty {
                    if !matches!(rune, Rune::State) {
                        literal.members.remove(1);
                    }
                    if matches!(rune, Rune::Derived | Rune::By) {
                        literal.members.pop();
                    }
                }
                annotation.type_annotation = ty;
            }
        }
        field.value = Some(expression);
    }
}

struct TypeFill<'a> {
    allocator: &'a Allocator,
    original: TSType<'a>,
}
impl<'a> VisitMut<'a> for TypeFill<'a> {
    fn visit_ts_type(&mut self, ty: &mut TSType<'a>) {
        if matches!(ty, TSType::TSUnknownKeyword(_)) {
            *ty = self.original.clone_in(self.allocator);
        } else {
            walk_mut::walk_ts_type(self, ty);
        }
    }
}

struct SyntheticSpans;
impl<'a> VisitMut<'a> for SyntheticSpans {
    fn visit_span(&mut self, span: &mut Span) {
        *span = SPAN;
    }
}
