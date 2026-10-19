//! Expression metadata shared by authored Svelte prop, event, and text lowering.
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{ScopeFlags, Semantic};

pub(crate) fn shorthand_property(name: &str, value: &str) -> String {
    // A non-computed __proto__ key would set the object's prototype.
    let key = if name == "__proto__" {
        "[\"__proto__\"]"
    } else {
        name
    };
    format!("{key}: {value}")
}

struct Calls<'s, 'a> {
    semantic: &'s Semantic<'a>,
    memo: bool,
    local: bool,
    state: bool,
}
impl Calls<'_, '_> {
    fn pure(&self, expression: &Expression<'_>) -> bool {
        match expression.get_inner_expression() {
            Expression::Identifier(id) => self
                .semantic
                .scoping()
                .get_reference(id.reference_id.get().unwrap())
                .symbol_id()
                .is_none(),
            Expression::StaticMemberExpression(member) => {
                if member.property.name == "tracking"
                    && matches!(member.object.get_inner_expression(),Expression::Identifier(id)if id.name=="$effect"&&self.semantic.scoping().get_reference(id.reference_id.get().unwrap()).symbol_id().is_none())
                {
                    return false;
                }
                self.pure(&member.object)
            }
            Expression::ComputedMemberExpression(member) => self.pure(&member.object),
            Expression::CallExpression(call) => self.pure_call(call),
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_) => true,
            _ => false,
        }
    }
    fn pure_call(&self, call: &CallExpression<'_>) -> bool {
        self.pure(&call.callee)
            && call.arguments.iter().all(|argument| match argument {
                Argument::SpreadElement(spread) => self.pure(&spread.argument),
                _ => argument
                    .as_expression()
                    .is_some_and(|expression| self.pure(expression)),
            })
    }
}
impl<'a> Visit<'a> for Calls<'_, 'a> {
    fn visit_spread_element(&mut self, spread: &SpreadElement<'a>) {
        // Iteration can execute user code even when the spread operand is a global.
        self.memo = true;
        self.state = true;
        walk::walk_spread_element(self, spread);
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.state |= !self.pure(&member.object);
        walk::walk_static_member_expression(self, member);
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        self.state |= !self.pure(&member.object);
        walk::walk_computed_member_expression(self, member);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        self.memo |= !self.pure(&call.callee) || self.local;
    }
    fn visit_tagged_template_expression(&mut self, tag: &TaggedTemplateExpression<'a>) {
        self.memo |= !self.pure(&tag.tag);
        walk::walk_tagged_template_expression(self, tag);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(symbol) = self
            .semantic
            .scoping()
            .get_reference(id.reference_id.get().unwrap())
            .symbol_id()
        else {
            return;
        };
        self.local = true;
        let flags = self.semantic.scoping().symbol_flags(symbol);
        let declaration = self.semantic.symbol_declaration(symbol);
        let stable = !self
            .semantic
            .symbol_references(symbol)
            .any(|reference| reference.is_write())
            && match declaration.kind() {
                AstKind::Function(_) => true,
                AstKind::VariableDeclarator(decl) => decl.init.as_ref().is_some_and(|value| {
                    matches!(
                        value.get_inner_expression(),
                        Expression::ArrowFunctionExpression(_)
                            | Expression::FunctionExpression(_)
                            | Expression::NumericLiteral(_)
                            | Expression::StringLiteral(_)
                            | Expression::BooleanLiteral(_)
                            | Expression::NullLiteral(_)
                            | Expression::BigIntLiteral(_)
                    )
                }),
                _ => false,
            };
        self.state |= !flags.is_function() && !stable;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

pub(crate) fn metadata<'a>(expression: &Expression<'a>, semantic: &Semantic<'a>) -> (bool, bool) {
    let mut calls = Calls {
        semantic,
        memo: false,
        local: false,
        state: false,
    };
    calls.visit_expression(expression);
    (calls.memo, calls.state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use oxc_span::SourceType;

    fn classify(expression: &str) -> (bool, bool) {
        // Keep authored bindings outside the reader: a generated wrapper must not
        // turn global callees into local bindings or expose function bodies.
        let source = format!(
            "import {{untrack}} from 'svelte';let value=$state(1);let obj={{}};let fn=()=>obj;let tag=()=>obj;let Box=class{{}};let trace=[];({expression});"
        );
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::tsx()).parse();
        assert!(
            parsed.errors.is_empty(),
            "{expression}: {:?}",
            parsed.errors
        );
        let built = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(built.errors.is_empty(), "{expression}: {:?}", built.errors);
        let Statement::ExpressionStatement(statement) = parsed.program.body.last().unwrap() else {
            panic!("expected expression statement");
        };
        metadata(&statement.expression, &built.semantic)
    }

    #[test]
    fn authored_text_call_classification_matches_svelte_5_57() {
        // Pinned compiler output uses template_effect argument memoization for
        // exactly the true cases; constructors alone do not count as calls.
        for (expression, memo) in [
            ("value", false),
            ("fn()", true),
            ("fn?.()", true),
            ("obj.method()", true),
            ("Math.abs(-1)", false),
            ("Math.abs(value)", true),
            ("String(value)", true),
            ("Number(1)", false),
            ("Boolean(value)", true),
            ("new Date()", false),
            ("new Box(value)", false),
            ("tag`x${value}`", true),
            ("(()=>fn())", false),
            ("(function(){return fn()})", false),
            ("(()=>fn())()", true),
            ("untrack(()=>fn())", true),
            ("(trace.push(value),obj)", true),
            ("(trace.push(1),obj)", true),
            ("value && Math.abs(1)", true),
            ("Math.abs(1) && value", false),
            ("new Box(fn())", true),
            ("obj[fn()]", true),
            ("value + fn()", true),
        ] {
            assert_eq!(classify(expression).0, memo, "{expression}");
        }
    }

    #[test]
    fn spread_iteration_counts_as_call_and_state_without_entering_function_bodies() {
        for expression in [
            "[...globalThis.items]",
            "({...obj})",
            "Math.max(...globalThis.items)",
        ] {
            assert_eq!(classify(expression), (true, true), "{expression}");
        }
        assert_eq!(classify("()=>[...obj]"), (false, false));
    }
}
