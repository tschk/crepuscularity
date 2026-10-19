//! Compile authored JSX modules without evaluating their JavaScript.
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_transformer::{JsxOptions, JsxRuntime, TransformOptions, Transformer};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Copy, Debug)]
pub enum MoonshineModuleTarget {
    Dom,
    Ssr,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoonshineModuleLanguage {
    JavaScript,
    TypeScript,
}

#[derive(Debug)]
pub struct MoonshineModule {
    /// External plain CSS blocks, emitted as inventoried assets by the CLI.
    pub styles: Vec<String>,
    pub source: String,
    pub language: MoonshineModuleLanguage,
    pub imports: Vec<String>,
    pub has_default_export: bool,
    pub has_module_declarations: bool,
}

struct Choice {
    test: oxc_span::Span,
    test_jsx: bool,
    test_choice: bool,
    yes: oxc_span::Span,
    no: oxc_span::Span,
    yes_jsx: bool,
    no_jsx: bool,
    yes_choice: bool,
    no_choice: bool,
}

struct LogicalChoice {
    left: oxc_span::Span,
    right: oxc_span::Span,
    operator: &'static str,
    left_jsx: bool,
    left_choice: bool,
    right_jsx: bool,
    right_choice: bool,
}

enum BranchChoice {
    Conditional(Choice),
    Logical(LogicalChoice),
}

fn module_preamble_end(program: &Program<'_>) -> usize {
    program
        .directives
        .last()
        .map(|node| node.span.end as usize)
        .or_else(|| program.hashbang.as_ref().map(|node| node.span.end as usize))
        .unwrap_or(0)
}

fn has_jsx_branch(value: &Expression<'_>) -> bool {
    match value.get_inner_expression() {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ConditionalExpression(node) => {
            has_jsx_branch(&node.test)
                || has_jsx_branch(&node.consequent)
                || has_jsx_branch(&node.alternate)
        }
        Expression::LogicalExpression(node) => {
            has_jsx_branch(&node.left) || has_jsx_branch(&node.right)
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ModuleHints<'s> {
    pub solid_list_helpers: &'s [String],
    pub solid_default_export: bool,
    pub solid_component_imports: &'s [String],
    pub source_globals: Option<&'s crate::source_globals::SourceGlobals>,
    pub svelte_ir_policy: crate::svelte_javascript::IrPolicy,
    pub svelte_component_props: bool,
    pub ordered_svelte_fragments: bool,
    pub intrinsic_namespace: Option<&'s str>,
    pub dynamic_tag_attribute: Option<&'s str>,
    pub dynamic_namespace_attribute: Option<&'s str>,
    pub dynamic_map: Option<&'s str>,
    pub intrinsic_reference: Option<&'s str>,
    pub ssr_transparent_helpers: &'s [String],
    pub const_scope: Option<&'s str>,
    pub lazy_prop_markers: &'s [&'s str],
    pub element_spread_markers: &'s [String],
    pub directive_group: Option<&'s str>,
    pub directive_each: Option<(&'s str, &'s str)>,
    pub directive_bindings: &'s [String],
    pub directives: &'s [crate::svelte_directives::Directive],
    pub scoped_class: Option<(&'s str, &'s str)>,
    pub class_directive_helper: Option<&'s str>,
    pub prop_helper: Option<&'s str>,
    pub property_keys: &'s [(&'s str, &'s str)],
    pub text_component: Option<&'s str>,
    pub textarea_default_helper: Option<&'s str>,
    pub inline_snippet_names: &'s [String],
}

#[derive(Default)]
struct Inspect<'s> {
    const_scope: Option<&'s str>,
    directive_group: Option<&'s str>,
    directive_bindings: &'s [String],
    intrinsic_reference: Option<&'s str>,
    intrinsic_namespace: Option<&'s str>,
    dynamic_tag_attribute: Option<&'s str>,
    dynamic_namespace_attribute: Option<&'s str>,
    dynamic_map: Option<&'s str>,
    directives: &'s [crate::svelte_directives::Directive],
    svelte_component_props: bool,
    source: &'s str,
    property_keys: &'s [(&'s str, &'s str)],
    text_component: Option<&'s str>,
    textarea_default_helper: Option<&'s str>,
    inline_snippet_names: &'s [String],
    comments: Vec<oxc_span::Span>,
    names: HashSet<String>,
    imports: Vec<String>,
    edits: Vec<(usize, usize, String)>,
    ssr: bool,
    ordered_svelte_fragments: bool,
    solid_source: bool,
    solid_list_helpers: &'s [String],
    ssr_erased_attributes: Vec<oxc_span::Span>,
    error: Option<String>,
    has_jsx: bool,
    has_fragments: bool,
    has_component_children: bool,
    once_spans: Vec<(oxc_span::Span, bool)>,
    choices: Vec<BranchChoice>,
    has_components: bool,
    binds_global_this: bool,
}
impl Inspect<'_> {
    fn reject(&mut self, message: &str) {
        self.error.get_or_insert_with(|| message.into());
    }
    fn reject_react_runtime(&mut self, source: &str) {
        if matches!(source, "react" | "react-dom")
            || source.starts_with("react/")
            || source.starts_with("react-dom/")
        {
            self.reject("native JSX does not execute the React renderer or hooks; use a React island with adapter=\"react\". Use import type/export type for type-only references");
        }
    }
    fn reactive_source(&mut self, source: &StringLiteral<'_>) -> bool {
        if source.value.as_str().starts_with("solid-js/") {
            self.reject("native module bridge subset does not support solid-js subpaths; use admitted named primitives from solid-js");
            return false;
        }
        source.value == "solid-js" || (self.ssr && source.value == "@tschk/moonshine-dom/reactive")
    }
    fn rewrite_reactive(&mut self, source: &StringLiteral<'_>) {
        self.edits.push((
            source.span.start as usize,
            source.span.end as usize,
            if self.ssr && self.solid_source {
                "\"@tschk/moonshine-solid/ssr\"".into()
            } else if self.ssr {
                "\"@tschk/moonshine-dom/ssr\"".into()
            } else if self.solid_source {
                "\"@tschk/moonshine-solid/solid\"".into()
            } else {
                "\"@tschk/moonshine-dom/reactive\"".into()
            },
        ));
    }
    fn control_name(name: &str, source: &StringLiteral<'_>) -> bool {
        source.value == "solid-js" && matches!(name, "mapArray" | "indexArray")
    }
    fn rewrite_control(&mut self, source: &StringLiteral<'_>) {
        self.edits.push((
            source.span.start as usize,
            source.span.end as usize,
            "\"@tschk/moonshine-dom/control\"".into(),
        ));
    }
    fn rewrite_named(
        &mut self,
        declaration: oxc_span::Span,
        source: &StringLiteral<'_>,
        specifiers: &[(oxc_span::Span, bool)],
        prefix: &str,
        has_metadata: bool,
    ) {
        let controls = specifiers.iter().filter(|(_, control)| *control).count();
        if controls == 0 {
            self.rewrite_reactive(source);
            return;
        }
        if controls == specifiers.len() {
            self.rewrite_control(source);
            return;
        }
        if has_metadata {
            self.reject("native module bridge subset cannot split imports/reexports with attributes or phases");
            return;
        }
        let mut replacement = String::new();
        // Keep comments outside specifier spans too; grouping may move their
        // position, but must not discard authored notes between comma tokens.
        for comment in &self.comments {
            if declaration.start <= comment.start
                && comment.end <= declaration.end
                && !specifiers
                    .iter()
                    .any(|(span, _)| span.start <= comment.start && comment.end <= span.end)
            {
                replacement.push_str(&self.source[comment.start as usize..comment.end as usize]);
                replacement.push('\n');
            }
        }
        for control in [false, true] {
            let items = specifiers
                .iter()
                .filter(|(_, kind)| *kind == control)
                .map(|(span, _)| &self.source[span.start as usize..span.end as usize])
                .collect::<Vec<_>>()
                .join(", ");
            let target = if control {
                "@tschk/moonshine-dom/control"
            } else if self.ssr && self.solid_source {
                "@tschk/moonshine-solid/ssr"
            } else if self.ssr {
                "@tschk/moonshine-dom/ssr"
            } else if self.solid_source {
                "@tschk/moonshine-solid/solid"
            } else {
                "@tschk/moonshine-dom/reactive"
            };
            replacement.push_str(&format!("{prefix} {{ {items} }} from {target:?};\n"));
        }
        self.edits.push((
            declaration.start as usize,
            declaration.end as usize,
            replacement,
        ));
    }
    fn reactive_name(&mut self, name: &str, source: &StringLiteral<'_>) {
        if source.value == "solid-js" && matches!(name, "createResource" | "Resource") {
            if !self.solid_source {
                self.reject(
                    "Solid client resources require explicit @jsxImportSource solid-js provenance",
                );

            }
            return;
        }
        if self.solid_source && source.value == "solid-js" && matches!(name, "For" | "Index") {
            return;
        }
        if Self::control_name(name, source) {
            return;
        }
        if source.value == "solid-js"
            && (!self.solid_source || name == "Setter")
            && matches!(
                name,
                "createRoot" | "getOwner" | "runWithOwner" | "Owner" | "Setter"
            )
        {
            self.reject("native module bridge subset does not support this Solid primitive/type; use the native host explicitly");
            return;
        }
        if !matches!(
            name,
            "createSignal"
                | "createMemo"
                | "createRoot"
                | "createComputed"
                | "batch"
                | "untrack"
                | "onCleanup"
                | "on"
                | "getOwner"
                | "runWithOwner"
                | "createEffect"
                | "onMount"
                | "Accessor"
                | "Setter"
                | "Owner"
        ) {
            self.reject("native module bridge subset does not support this reactive export; use admitted named primitives");
        }
    }
    fn mark_choice(&mut self, value: &Expression<'_>) -> bool {
        if let Expression::LogicalExpression(node) = value.get_inner_expression() {
            if !has_jsx_branch(&node.left) && !has_jsx_branch(&node.right) {
                return false;
            }
            self.lift_choice_type(value);
            let left_choice = self.mark_choice(&node.left);
            let right_choice = self.mark_choice(&node.right);
            self.choices.push(BranchChoice::Logical(LogicalChoice {
                left: node.left.span(),
                right: node.right.span(),
                operator: node.operator.as_str(),
                left_jsx: matches!(
                    node.left.get_inner_expression(),
                    Expression::JSXElement(_) | Expression::JSXFragment(_)
                ),
                left_choice,
                right_jsx: matches!(
                    node.right.get_inner_expression(),
                    Expression::JSXElement(_) | Expression::JSXFragment(_)
                ),
                right_choice,
            }));
            return true;
        }
        let Expression::ConditionalExpression(node) = value.get_inner_expression() else {
            return false;
        };
        if !has_jsx_branch(&node.test)
            && !has_jsx_branch(&node.consequent)
            && !has_jsx_branch(&node.alternate)
        {
            return false;
        }
        let jsx = |value: &Expression<'_>| {
            matches!(
                value.get_inner_expression(),
                Expression::JSXElement(_) | Expression::JSXFragment(_)
            )
        };
        self.lift_choice_type(value);
        let test_choice = self.mark_choice(&node.test);
        let yes_choice = self.mark_choice(&node.consequent);
        let no_choice = self.mark_choice(&node.alternate);
        self.choices.push(BranchChoice::Conditional(Choice {
            test: node.test.span(),
            test_jsx: jsx(&node.test),
            test_choice,
            yes: node.consequent.span(),
            no: node.alternate.span(),
            yes_jsx: jsx(&node.consequent),
            no_jsx: jsx(&node.alternate),
            yes_choice,
            no_choice,
        }));
        true
    }
    fn lift_choice_type(&mut self, value: &Expression<'_>) {
        // The generated expression is an accessor. Lift authored value type
        // assertions to accessor type assertions, preserving the result type.
        let mut wrapper = value;
        loop {
            let annotation = match wrapper {
                Expression::ParenthesizedExpression(node) => {
                    wrapper = &node.expression;
                    continue;
                }
                Expression::TSAsExpression(node) => {
                    wrapper = &node.expression;
                    Some(node.type_annotation.span())
                }
                Expression::TSSatisfiesExpression(node) => {
                    wrapper = &node.expression;
                    Some(node.type_annotation.span())
                }
                Expression::TSNonNullExpression(node) => {
                    wrapper = &node.expression;
                    continue;
                }
                _ => None,
            };
            let Some(annotation) = annotation else {
                break;
            };
            self.edits.push((
                annotation.start as usize,
                annotation.start as usize,
                "(() => (".into(),
            ));
            self.edits.push((
                annotation.end as usize,
                annotation.end as usize,
                "))".into(),
            ));
        }
    }
    fn inline_snippet_scope(&self, expression: &Expression<'_>) -> bool {
        let Expression::CallExpression(call) = expression.get_inner_expression() else {
            return false;
        };
        if call.optional || !call.arguments.is_empty() {
            return false;
        }
        let Expression::ArrowFunctionExpression(arrow) = call.callee.get_inner_expression() else {
            return false;
        };
        if arrow.r#async || !arrow.params.items.is_empty() || arrow.params.rest.is_some() {
            return false;
        }
        arrow.body.statements.iter().any(|statement| matches!(statement,
            Statement::FunctionDeclaration(function) if function.id.as_ref().is_some_and(|id| self.inline_snippet_names.iter().any(|name| name == id.name.as_str()))
        ))
    }
    fn mark_jsx_child(&mut self, expression: &JSXExpression<'_>) {
        // Generated textarea wrappers construct their node in sibling order.
        if expression.as_expression().is_some_and(|value| {
            matches!(value.get_inner_expression(), Expression::CallExpression(call)
                if (self.textarea_default_helper.is_some_and(|name| call.callee.is_specific_id(name)) || self.dynamic_map.is_some_and(|name| call.callee.is_specific_id(name)) || self.solid_list_helpers.iter().any(|name| call.callee.is_specific_id(name))))
        }) {
            self.once_spans.push((expression.span(), false));
            return;
        }
        if expression
            .as_expression()
            .is_some_and(|value| self.inline_snippet_scope(value))
        {
            self.once_spans.push((expression.span(), false));
            return;
        }
        if expression
            .as_expression()
            .is_some_and(|value| self.mark_choice(value))
        {
            return;
        }
        if expression.as_expression().is_some_and(|value| {
            matches!(
                value.get_inner_expression(),
                Expression::JSXElement(_) | Expression::JSXFragment(_)
            )
        }) {
            self.once_spans.push((expression.span(), false));
        }
    }
    fn thunk(&mut self, expression: &JSXExpression<'_>) {
        // Intrinsic siblings are constructed eagerly; their generated wrapper must be too.
        if expression.as_expression().is_some_and(|value| {
            matches!(value.get_inner_expression(), Expression::CallExpression(call)
                if (self.textarea_default_helper.is_some_and(|name| call.callee.is_specific_id(name)) || self.dynamic_map.is_some_and(|name| call.callee.is_specific_id(name)) || self.solid_list_helpers.iter().any(|name| call.callee.is_specific_id(name))))
        }) {
            return;
        }
        if matches!(expression, JSXExpression::EmptyExpression(_)) {
            return;
        }
        let mut check = Suspends(false);
        check.visit_jsx_expression(expression);
        if check.0 {
            self.reject("await/yield in reactive JSX expressions is unsupported");
            return;
        }
        if expression
            .as_expression()
            .is_some_and(|value| self.inline_snippet_scope(value))
        {
            self.once_spans.push((expression.span(), false));
            return;
        }
        if expression
            .as_expression()
            .is_some_and(|value| self.mark_choice(value))
        {
            return;
        }
        let span = expression.span();
        self.edits
            .push((span.start as usize, span.start as usize, "() => (".into()));
        self.edits
            .push((span.end as usize, span.end as usize, ")".into()));
    }
}
struct Suspends(bool);
impl<'a> Visit<'a> for Suspends {
    fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
        self.0 = true;
    }
    fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
        self.0 = true;
    }
}
impl<'a> Visit<'a> for Inspect<'_> {
    fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
        if node.name == "require" {
            self.reject("CommonJS require is unsupported; use ESM imports");
        }
        self.names.insert(node.name.to_string());
    }
    fn visit_binding_identifier(&mut self, node: &BindingIdentifier<'a>) {
        self.binds_global_this |= node.name == "globalThis";
        self.names.insert(node.name.to_string());
    }
    fn visit_import_declaration(&mut self, node: &ImportDeclaration<'a>) {
        self.imports.push(node.source.value.to_string());
        if node.import_kind != ImportOrExportKind::Type {
            self.reject_react_runtime(node.source.value.as_str());
        }
        let target = if self.ssr {
            MoonshineModuleTarget::Ssr
        } else {
            MoonshineModuleTarget::Dom
        };
        match crate::svelte_module::rewrite_svelte_import(self.source, node, target) {
            Ok(Some(replacement)) => self.edits.push((
                node.span.start as usize,
                node.span.end as usize,
                replacement,
            )),
            Ok(None) => {}
            Err(error) => self.reject(&error),
        }
        if self.reactive_source(&node.source) {
            if let Some(specifiers) = &node.specifiers {
                if node.source.value == "solid-js" && specifiers.is_empty() {
                    self.reject(
                        "native module bridge subset requires admitted named Solid primitives",
                    );
                }
                for specifier in specifiers {
                    match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            self.reactive_name(specifier.imported.name().as_str(), &node.source)
                        }
                        _ => self
                            .reject("native module bridge subset requires named reactive imports"),
                    }
                }
            } else {
                self.reject("native module bridge subset requires named reactive imports");
            }
            let specifiers = node
                .specifiers
                .iter()
                .flatten()
                .filter_map(|specifier| match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => Some((
                        specifier.span,
                        Self::control_name(specifier.imported.name().as_str(), &node.source),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let prefix = if node.import_kind == ImportOrExportKind::Type {
                "import type"
            } else {
                "import"
            };
            self.rewrite_named(
                node.span,
                &node.source,
                &specifiers,
                prefix,
                node.phase.is_some() || node.with_clause.is_some(),
            );
        }
        walk::walk_import_declaration(self, node);
    }
    fn visit_export_named_declaration(&mut self, node: &ExportNamedDeclaration<'a>) {
        if let Some(source) = &node.source {
            self.imports.push(source.value.to_string());
            if node.export_kind != ImportOrExportKind::Type {
                self.reject_react_runtime(source.value.as_str());
            }
            let target = if self.ssr {
                MoonshineModuleTarget::Ssr
            } else {
                MoonshineModuleTarget::Dom
            };
            match crate::svelte_module::rewrite_svelte_export(self.source, node, target) {
                Ok(Some(replacement)) => self.edits.push((
                    node.span.start as usize,
                    node.span.end as usize,
                    replacement,
                )),
                Ok(None) => {}
                Err(error) => self.reject(&error),
            }
            if self.reactive_source(source) {
                if source.value == "solid-js" && node.specifiers.is_empty() {
                    self.reject(
                        "native module bridge subset requires admitted named Solid primitives",
                    );
                }
                for specifier in &node.specifiers {
                    self.reactive_name(specifier.local.name().as_str(), source);
                }
                let specifiers = node
                    .specifiers
                    .iter()
                    .map(|specifier| {
                        (
                            specifier.span,
                            Self::control_name(specifier.local.name().as_str(), source),
                        )
                    })
                    .collect::<Vec<_>>();
                let prefix = if node.export_kind == ImportOrExportKind::Type {
                    "export type"
                } else {
                    "export"
                };
                self.rewrite_named(
                    node.span,
                    source,
                    &specifiers,
                    prefix,
                    node.with_clause.is_some(),
                );
            }
        }
        walk::walk_export_named_declaration(self, node);
    }
    fn visit_export_all_declaration(&mut self, node: &ExportAllDeclaration<'a>) {
        self.imports.push(node.source.value.to_string());
        if node.export_kind != ImportOrExportKind::Type {
            self.reject_react_runtime(node.source.value.as_str());
        }
        if node.source.value == "svelte" || node.source.value.starts_with("svelte/") {
            self.reject("native Svelte reexports are unsupported; use supported named imports");
        }
        if self.reactive_source(&node.source) {
            self.reject("native module bridge subset does not support wildcard/namespace reactive reexports");
        }
        walk::walk_export_all_declaration(self, node);
    }
    fn visit_ts_import_type(&mut self, node: &TSImportType<'a>) {
        self.imports.push(node.source.value.to_string());
        if node.source.value == "svelte" || node.source.value.starts_with("svelte/") {
            self.reject(
                "native Svelte import type queries are unsupported; use supported named imports",
            );
        }
        if self.reactive_source(&node.source) {
            match &node.qualifier {
                Some(TSImportTypeQualifier::Identifier(name)) => {
                    self.reactive_name(name.name.as_str(), &node.source)
                }
                _ => self.reject(
                    "native module bridge subset requires a supported named reactive import type",
                ),
            }
            let control = matches!(&node.qualifier, Some(TSImportTypeQualifier::Identifier(name)) if Self::control_name(name.name.as_str(), &node.source));
            if control {
                self.rewrite_control(&node.source);
            } else {
                self.rewrite_reactive(&node.source);
            }
        }
        walk::walk_ts_import_type(self, node);
    }
    fn visit_ts_import_equals_declaration(&mut self, _: &TSImportEqualsDeclaration<'a>) {
        self.reject("TypeScript import-equals declarations are unsupported; use ESM imports");
    }
    fn visit_import_expression(&mut self, node: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &node.source {
            self.imports.push(source.value.to_string());
            self.reject_react_runtime(source.value.as_str());
            if source.value == "vue"
                || source.value.starts_with("vue/")
                || source.value.starts_with("@vue/")
            {
                self.reject(
                    "native dynamic Vue host imports are unsupported; use supported named imports",
                );
            }
            if source.value == "svelte" || source.value.starts_with("svelte/") {
                self.reject(
                    "native Svelte dynamic imports are unsupported; use supported named imports",
                );
            }
            if self.reactive_source(source) {
                self.reject(
                    "native module bridge subset does not support dynamic reactive imports",
                );
            }
        } else {
            self.reject("computed dynamic imports are unsupported");
        }
        walk::walk_import_expression(self, node);
    }
    fn visit_jsx_fragment(&mut self, node: &JSXFragment<'a>) {
        self.has_jsx = true;
        self.has_fragments = true;
        for child in &node.children {
            match child {
                JSXChild::ExpressionContainer(value) => self.thunk(&value.expression),
                JSXChild::Spread(_) => self.reject("JSX spread children are unsupported"),
                _ => {}
            }
        }
        walk::walk_jsx_fragment(self, node);
    }
    fn visit_jsx_element(&mut self, node: &JSXElement<'a>) {
        self.has_jsx = true;
        let group = matches!(&node.opening_element.name, JSXElementName::IdentifierReference(name) if Some(name.name.as_str()) == self.directive_group);
        let component = !group
            && matches!(
                node.opening_element.name,
                JSXElementName::IdentifierReference(_) | JSXElementName::MemberExpression(_)
            );
        self.has_components |= component
            && !matches!(&node.opening_element.name, JSXElementName::IdentifierReference(name) if Some(name.name.as_str()) == self.text_component)
            && !matches!(&node.opening_element.name, JSXElementName::IdentifierReference(name) if Some(name.name.as_str()) == self.const_scope);
        // Source SSR uses its codec only with a validated compiler namespace witness.
        let witnessed_namespace = self.svelte_component_props
            && self.intrinsic_namespace.is_some_and(|marker| {
                node.opening_element.attributes.iter().any(|attribute| {
                    matches!(attribute, JSXAttributeItem::Attribute(attribute)
                        if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == marker)
                        && matches!(&attribute.value, Some(JSXAttributeValue::StringLiteral(value))
                            if matches!(value.value.as_str(), "http://www.w3.org/1999/xhtml" | "http://www.w3.org/2000/svg" | "http://www.w3.org/1998/Math/MathML")))
                })
            });
        let source_namespace = self.ssr && witnessed_namespace;
        let supported = match &node.opening_element.name {
            JSXElementName::Identifier(name) => {
                supported_html_tag(name.name.as_str())
                    || witnessed_namespace
                    || ((self.ordered_svelte_fragments || source_namespace)
                        && matches!(name.name.as_str(), "svg" | "g" | "foreignObject" | "title"))
            }
            _ => false,
        };
        if !supported && !component && !group {
            self.reject("unsupported JSX tag: only supported HTML intrinsics are allowed");
        }
        if node.opening_element.type_arguments.is_some() {
            self.reject("JSX type arguments are unsupported");
        }
        let mut names = HashSet::new();
        for attr in &node.opening_element.attributes {
            let JSXAttributeItem::Attribute(attr) = attr else {
                self.reject("JSX spread attributes are unsupported");
                continue;
            };
            let name =
                match &attr.name {
                    JSXAttributeName::Identifier(name) => name.name.to_string(),
                    JSXAttributeName::NamespacedName(name)
                        if component
                            && name.namespace.name == "onUpdate"
                            && name.name.name.chars().all(|ch| {
                                ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
                            }) =>
                    {
                        format!("{}:{}", name.namespace.name, name.name.name)
                    }
                    _ => {
                        self.reject("namespaced JSX attributes are unsupported");
                        continue;
                    }
                };
            let name = name.as_str();
            if self.solid_source
                && !component
                && attr.value.is_none()
                && name.to_ascii_lowercase().starts_with("data-")
            {
                self.edits.push((
                    attr.span.end as usize,
                    attr.span.end as usize,
                    "=\"\"".into(),
                ));
            }
            if self.dynamic_tag_attribute == Some(name)
                || self.dynamic_namespace_attribute == Some(name)
            {
                let selector = matches!(&attr.value, Some(JSXAttributeValue::ExpressionContainer(value)) if matches!(value.expression.as_expression(), Some(Expression::Identifier(_))));
                let witnessed = self.intrinsic_namespace.is_some_and(|marker| node.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(id) if id.name == marker) && matches!(&attribute.value, Some(JSXAttributeValue::StringLiteral(value)) if matches!(value.value.as_str(), "http://www.w3.org/1999/xhtml" | "http://www.w3.org/2000/svg" | "http://www.w3.org/1998/Math/MathML")))));
                if component || !selector || !witnessed {
                    self.reject("dynamic intrinsic requires a compiler-local selector and namespace witness");
                }
            }
            if !names.insert(name.to_owned()) {
                self.reject("duplicate JSX attributes are unsupported");
            }
            if self.ssr
                && self.svelte_component_props
                && name == "defaultValue"
                && matches!(&node.opening_element.name, JSXElementName::Identifier(tag) if tag.name == "textarea")
            {
                self.ssr_erased_attributes.push(attr.span);
                continue;
            }
            if matches!(
                name,
                "__proto__" | "ref" | "innerHTML" | "outerHTML" | "dangerouslySetInnerHTML"
            ) && !(component && self.svelte_component_props)
            {
                self.reject("unsupported JSX ref/children/raw HTML attribute");
            }
            if name == "children" && (!component || !node.children.is_empty()) {
                self.reject("explicit JSX children cannot be combined with body children");
            }
            let event = !component && name.starts_with("on");
            if event
                && !matches!(
                    name,
                    "onClick"
                        | "onInput"
                        | "onChange"
                        | "onToggle"
                        | "onSubmit"
                        | "onReset"
                        | "onKeyDown"
                        | "onKeyUp"
                        | "onFocus"
                        | "onBlur"
                )
            {
                self.reject("unsupported JSX event");
            }
            match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(value)) => {
                    if component {
                        self.mark_jsx_child(&value.expression);
                    }

                    if !event
                        && !component
                        && !self.property_keys.iter().any(|(marker, _)| *marker == name)
                        && self.dynamic_tag_attribute != Some(name)
                        && self.dynamic_namespace_attribute != Some(name)
                        && self.intrinsic_reference != Some(name)
                        && !self
                            .directives
                            .iter()
                            .any(|directive| directive.attribute == name)
                        && !self.directive_bindings.iter().any(|marker| marker == name)
                    {
                        // Source SSR retains the authored literal-string fast path.
                        // Identifiers returning strings remain dynamic accessors.
                        let literal_source = self.ssr
                            && self.svelte_component_props
                            && value.expression.as_expression().is_some_and(|expression| {
                                matches!(
                                    expression.get_inner_expression(),
                                    Expression::StringLiteral(_)
                                )
                            });
                        if !literal_source {
                            self.thunk(&value.expression);
                        }
                    } else if matches!(value.expression, JSXExpression::EmptyExpression(_)) {
                        self.reject("empty JSX event handler");
                    }
                }
                Some(JSXAttributeValue::StringLiteral(_)) | None if !event => {}
                _ => self.reject("unsupported JSX attribute value"),
            }
            if self.ssr && event {
                self.ssr_erased_attributes.push(attr.span);
            }
        }
        if component {
            self.has_component_children |= !node.children.is_empty();
            for child in &node.children {
                match child {
                    JSXChild::Element(node) => self.once_spans.push((node.span, true)),
                    JSXChild::Fragment(node) => self.once_spans.push((node.span, true)),
                    JSXChild::ExpressionContainer(value) => self.mark_jsx_child(&value.expression),
                    _ => {}
                }
            }
        }
        for child in &node.children {
            match child {
                JSXChild::ExpressionContainer(value) if !component => self.thunk(&value.expression),
                JSXChild::Spread(_) => self.reject("JSX spread children are unsupported"),
                _ => {}
            }
        }
        walk::walk_jsx_element(self, node);
    }
}

pub(crate) fn supported_html_tag(name: &str) -> bool {
    matches!(
        name,
        "div"
            | "meta"
            | "link"
            | "title"
            | "span"
            | "p"
            | "button"
            | "input"
            | "textarea"
            | "pre"
            | "label"
            | "select"
            | "optgroup"
            | "option"
            | "ul"
            | "ol"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "a"
            | "img"
            | "section"
            | "article"
            | "aside"
            | "main"
            | "header"
            | "footer"
            | "nav"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "strong"
            | "em"
            | "small"
            | "br"
            | "hr"
            | "form"
            | "fieldset"
            | "legend"
            | "output"
            | "progress"
            | "meter"
            | "table"
            | "thead"
            | "tbody"
            | "tr"
            | "th"
            | "td"
    ) || crate::ir::HTML_TEXT_TAGS.contains(&name)
}

pub fn compile_moonshine_module(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<MoonshineModule, String> {
    compile_module(source, path, target, false, ModuleHints::default())
}

/// Compile authored components after verifying imported Solid function provenance.
pub fn compile_moonshine_module_with_resolver(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    resolver: &mut dyn FnMut(
        &Path,
        &str,
    ) -> Result<
        crate::solid_component_resolver::SolidComponentModule,
        String,
    >,
) -> Result<MoonshineModule, String> {
    let verified = crate::solid_component_resolver::verified_imports(source, path, resolver)?;
    let verified_default =
        crate::solid_component_resolver::verified_default_export(source, path, resolver)?;
    compile_module(
        source,
        path,
        target,
        false,
        ModuleHints {
            solid_component_imports: &verified,
            solid_default_export: verified_default,
            ..ModuleHints::default()
        },
    )
}

pub(crate) fn compile_moonshine_module_with_hints(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    hints: ModuleHints<'_>,
) -> Result<MoonshineModule, String> {
    compile_module(source, path, target, false, hints)
}

pub(crate) fn compile_moonshine_svelte_module(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    hints: ModuleHints<'_>,
) -> Result<MoonshineModule, String> {
    compile_module(source, path, target, true, hints)
}

fn compile_module(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    authored_svelte: bool,
    hints: ModuleHints<'_>,
) -> Result<MoonshineModule, String> {
    if !authored_svelte {
        if let Some(lowered) = crate::solid_lists::lower(source, path, target)? {
            return compile_module(
                &lowered.source,
                path,
                target,
                false,
                ModuleHints {
                    solid_list_helpers: &lowered.helpers,
                    ..hints
                },
            );
        }
    }
    if let Some((marker, hash)) = hints.scoped_class {
        if let Some(scoped) = crate::svelte_css::scoped::consume_with_directives(
            source,
            marker,
            hash,
            hints.class_directive_helper,
        )? {
            return compile_module(
                &scoped,
                path,
                target,
                authored_svelte,
                ModuleHints {
                    scoped_class: None,
                    ..hints
                },
            );
        }
    }
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(
        path.with_extension(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
    )
    .map_err(|e| e.to_string())?
    .with_module(true);
    let mut parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("module parse failed: {:?}", parsed.errors));
    }
    if let Some(rewritten) = solid_jsx_element_types(source, &parsed.program, target)? {
        return compile_module(&rewritten, path, target, authored_svelte, hints);
    }
    let svelte_companion = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("js")
        })
        && path
            .file_stem()
            .and_then(|stem| Path::new(stem).extension())
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svelte"));
    if !authored_svelte && svelte_companion {
        let effects = crate::svelte_effect::transform_companion_effects(source, path, target)?;
        let lowered = crate::svelte_script::transform_companion(&effects, path, target)?;
        return compile_module(&lowered, path, target, true, hints);
    }
    let selectors = crate::solid_source::selector(source, &parsed.program)?;
    let solid_source = !selectors.is_empty();
    if solid_source {
        if authored_svelte {
            return Err(
                "Solid JSX provenance is unsupported in authored framework components".into(),
            );
        }
        hints.svelte_ir_policy.configure(&mut parsed.program);
        let semantic = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !semantic.errors.is_empty() {
            return Err(format!("module semantic errors: {:?}", semantic.errors));
        }
        crate::solid_source::check_components(
            &semantic.semantic,
            hints.solid_component_imports,
            hints.solid_default_export,
        )?;
    }
    let has_module_declarations = parsed.module_record.has_module_syntax;
    let has_default_export = parsed.program.body.iter().any(|node| match node {
        Statement::ExportAllDeclaration(decl) => {
            decl.export_kind == ImportOrExportKind::Value
                && decl
                    .exported
                    .as_ref()
                    .is_some_and(|name| name.name() == "default")
        }
        Statement::ExportDefaultDeclaration(decl) => !matches!(
            decl.declaration,
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        ),
        Statement::ExportNamedDeclaration(decl) => {
            decl.export_kind == ImportOrExportKind::Value
                && decl.specifiers.iter().any(|specifier| {
                    specifier.export_kind == ImportOrExportKind::Value
                        && specifier.exported.name() == "default"
                })
        }
        _ => false,
    });
    let mut inspect = Inspect {
        const_scope: hints.const_scope,
        source,
        svelte_component_props: hints.svelte_component_props,
        directive_group: hints.directive_group,
        directive_bindings: hints.directive_bindings,
        intrinsic_reference: hints.intrinsic_reference,
        intrinsic_namespace: hints.intrinsic_namespace,
        dynamic_tag_attribute: hints.dynamic_tag_attribute,
        dynamic_namespace_attribute: hints.dynamic_namespace_attribute,
        dynamic_map: hints.dynamic_map,
        directives: hints.directives,
        property_keys: hints.property_keys,
        text_component: hints.text_component,
        textarea_default_helper: hints.textarea_default_helper,
        inline_snippet_names: hints.inline_snippet_names,
        comments: parsed
            .program
            .comments
            .iter()
            .map(|comment| comment.span)
            .collect(),
        ssr: matches!(target, MoonshineModuleTarget::Ssr),
        ordered_svelte_fragments: hints.ordered_svelte_fragments,
        solid_source,
        solid_list_helpers: hints.solid_list_helpers,
        ..Inspect::default()
    };
    inspect.edits.extend(
        selectors
            .iter()
            .map(|span| (span.start as usize, span.end as usize, String::new())),
    );
    inspect.visit_program(&parsed.program);
    if let Some(error) = inspect.error {
        return Err(error);
    }
    if authored_svelte && !inspect.ssr_erased_attributes.is_empty() {
        hints.svelte_ir_policy.configure(&mut parsed.program);
        let semantic = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !semantic.errors.is_empty() {
            return Err(format!("module semantic errors: {:?}", semantic.errors));
        }
        inspect
            .ssr_erased_attributes
            .sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        let mut spans = Vec::new();
        let mut previous_end = 0;
        for span in inspect.ssr_erased_attributes {
            if span.start >= previous_end {
                previous_end = span.end;
                spans.push(span);
            }
        }
        let mut stripped = source.to_owned();
        for span in spans.into_iter().rev() {
            stripped.replace_range(span.start as usize..span.end as usize, "");
        }
        // Reinspect without erased attribute expressions so nested JSX edits cannot overlap removal.
        let mut compiled = compile_module(&stripped, path, target, authored_svelte, hints)?;
        compiled.imports = inspect.imports;
        return Ok(compiled);
    }
    let alias = (0usize..)
        .map(|i| format!("__crepusH{i}"))
        .find(|name| !inspect.names.contains(name))
        .unwrap();
    let fragment_alias = (0usize..)
        .map(|i| format!("__crepusFragment{i}"))
        .find(|name| !inspect.names.contains(name) && name != &alias)
        .unwrap();
    let fresh = |prefix: &str| {
        (0usize..)
            .map(|i| format!("{prefix}{i}"))
            .find(|name| !inspect.names.contains(name))
            .unwrap()
    };
    let native_globals = if hints.source_globals.is_none()
        && (inspect.has_components || hints.directive_group.is_some())
        && inspect.binds_global_this
    {
        Some(crate::source_globals::SourceGlobals::with_aliases(
            &fresh("__crepusSourceRealm"),
            &fresh("__crepusSourceTypes"),
            source_type.is_typescript(),
        ))
    } else {
        None
    };
    let hints = ModuleHints {
        source_globals: hints.source_globals.or(native_globals.as_ref()),
        ..hints
    };
    if (inspect.has_components || hints.directive_group.is_some())
        && inspect.binds_global_this
        && !hints
            .source_globals
            .is_some_and(crate::source_globals::SourceGlobals::is_active)
    {
        return Err(
            "binding globalThis in component modules lacks an owned platform context".into(),
        );
    }
    let once_alias = fresh("__crepusOnce");
    let children_alias = fresh("__crepusChildren");
    let choose_alias = fresh("__crepusChoose");
    let read_alias = fresh("__crepusRead");
    let memo_alias = fresh("__crepusMemo");
    let condition_memo_alias = if authored_svelte
        && matches!(target, MoonshineModuleTarget::Dom)
        && hints.ordered_svelte_fragments
    {
        fresh("__crepusConditionMemo")
    } else {
        memo_alias.clone()
    };
    let show_alias = fresh("__crepusShow");
    let source_effect_alias = fresh("__crepusSourceEffect");
    let owner_alias = fresh("__crepusGetOwner");
    let run_alias = fresh("__crepusRunWithOwner");
    let untrack_alias = fresh("__crepusUntrack");
    for (span, markup) in &inspect.once_spans {
        inspect.edits.push((
            span.start as usize,
            span.start as usize,
            format!("{}{once_alias}(() => (", if *markup { "{" } else { "" }),
        ));
        inspect.edits.push((
            span.end as usize,
            span.end as usize,
            format!(")){}", if *markup { "}" } else { "" }),
        ));
    }
    // Preserve authored comments around ternary punctuation while replacing
    // only the actual separators (comments may themselves contain ? or :).
    let separator = |start: u32, end: u32, token: u8| -> Result<usize, String> {
        for offset in start as usize..end as usize {
            if source.as_bytes()[offset] == token
                && !parsed.program.comments.iter().any(|comment| {
                    comment.span.start as usize <= offset && offset < comment.span.end as usize
                })
            {
                return Ok(offset);
            }
        }
        Err("missing conditional separator in parsed module".into())
    };
    for choice in &inspect.choices {
        match choice {
            BranchChoice::Conditional(choice) => {
                let Choice {
                    test,
                    test_jsx,
                    test_choice,
                    yes,
                    no,
                    yes_jsx,
                    no_jsx,
                    yes_choice,
                    no_choice,
                } = choice;
                // Branch factories return accessors. A nested choose already is one;
                // reading it inside an outer memo would couple their cleanup owners.
                let open = |jsx: bool, nested: bool, memo: &str| {
                    if nested {
                        String::new()
                    } else if jsx {
                        format!("{memo}(() => ({untrack_alias}(() => (")
                    } else {
                        format!("{memo}(() => (")
                    }
                };
                let close = |jsx: bool, nested: bool| {
                    if nested {
                        ""
                    } else if jsx {
                        "))))"
                    } else {
                        "))"
                    }
                };
                inspect.edits.insert(
                    0,
                    (
                        test.start as usize,
                        test.start as usize,
                        format!(
                            "{choose_alias}(() => ({}",
                            open(
                                *test_jsx,
                                *test_choice,
                                if *test_jsx {
                                    &memo_alias
                                } else {
                                    &condition_memo_alias
                                },
                            )
                        ),
                    ),
                );
                inspect.edits.push((
                    separator(test.end, yes.start, b'?')?,
                    separator(test.end, yes.start, b'?')? + 1,
                    format!(
                        "{}), () => ({}",
                        close(*test_jsx, *test_choice),
                        open(*yes_jsx, *yes_choice, &memo_alias)
                    ),
                ));
                inspect.edits.push((
                    separator(yes.end, no.start, b':')?,
                    separator(yes.end, no.start, b':')? + 1,
                    format!(
                        "{}), () => ({}",
                        close(*yes_jsx, *yes_choice),
                        open(*no_jsx, *no_choice, &memo_alias)
                    ),
                ));
                inspect.edits.push((
                    no.end as usize,
                    no.end as usize,
                    format!("{}))", close(*no_jsx, *no_choice)),
                ));
            }
            BranchChoice::Logical(choice) => {
                let LogicalChoice {
                    left,
                    right,
                    operator,
                    left_jsx,
                    left_choice,
                    right_jsx,
                    right_choice,
                } = choice;
                let token = separator(left.end, right.start, operator.as_bytes()[0])?;
                let returned_left = format!("{read_alias} => {memo_alias}({read_alias})");
                let rhs_open = if *right_choice {
                    String::new()
                } else if *right_jsx {
                    format!("{memo_alias}(() => ({untrack_alias}(() => (")
                } else {
                    format!("{memo_alias}(() => (")
                };
                let rhs_close = if *right_choice {
                    ""
                } else if *right_jsx {
                    "))))"
                } else {
                    "))"
                };
                let lhs_open = if *left_choice {
                    String::new()
                } else if *left_jsx {
                    format!("{memo_alias}(() => ({untrack_alias}(() => (")
                } else {
                    format!("{condition_memo_alias}(() => (")
                };
                let lhs_close = if *left_choice {
                    ""
                } else if *left_jsx {
                    "))))"
                } else {
                    "))"
                };
                inspect.edits.insert(
                    0,
                    (
                        left.start as usize,
                        left.start as usize,
                        format!("{choose_alias}(() => ({lhs_open}"),
                    ),
                );
                let middle = if *operator == "&&" {
                    format!("{lhs_close}), () => ({rhs_open}")
                } else {
                    format!("{lhs_close}), {returned_left}, () => ({rhs_open}")
                };
                inspect.edits.push((token, token + operator.len(), middle));
                let suffix = if *operator == "&&" {
                    format!("{rhs_close}), {returned_left})")
                } else {
                    format!("{rhs_close}), {})", *operator == "??")
                };
                inspect
                    .edits
                    .push((right.end as usize, right.end as usize, suffix));
            }
        }
    }
    let insertion = module_preamble_end(&parsed.program);
    let host = match target {
        MoonshineModuleTarget::Dom if solid_source => "@tschk/moonshine-solid/h",
        MoonshineModuleTarget::Ssr if solid_source => "@tschk/moonshine-solid/ssr",
        MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/dom",
        MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
    };
    let mut import = format!("\nimport {{ h as {alias} }} from {host:?};\n");
    if let Some(globals) = &native_globals {
        import.push_str(&globals.imports);
        let platform = "@tschk/moonshine-dom/platform".to_owned();
        if !inspect.imports.contains(&platform) {
            inspect.imports.push(platform);
        }
    }
    if inspect.has_fragments && matches!(target, MoonshineModuleTarget::Ssr) {
        let child = format!("import({host:?}).SSRChild");
        let (parameter, result) = if source_type.is_typescript() {
            (
                format!("props: {{ children?: {child} }} | null, ...children: {child}[]"),
                format!(": {child}"),
            )
        } else {
            ("props, ...children".into(), String::new())
        };
        if hints.svelte_ir_policy == crate::svelte_javascript::IrPolicy::JavaScript {
            // Keep the existing SSRChild contract when TS annotations are erased.
            // An untyped fragment return would erase ComponentView metadata through any.
            import.push_str(&format!(
                "/** @param {{{{children?: {child}}}|null}} props\n * @param {{...{child}}} children\n * @returns {{{child}}} */\n"
            ));
        }
        import.push_str(&format!(
            "function {fragment_alias}({parameter}){result} {{ return props?.children ?? children; }}\n"
        ));
    } else if inspect.has_fragments {
        // Only generated helper plumbing uses any; authored types stay intact.
        // JSX sources receive plain JS so their extension contract is unchanged.
        let (parameter, result) = if source_type.is_typescript() {
            ("props: { children?: any }", ": any")
        } else {
            ("props", "")
        };
        import.push_str(&format!(
            "function {fragment_alias}({parameter}){result} {{ return props.children ?? []; }}\n"
        ));
    }
    if !inspect.once_spans.is_empty()
        || !inspect.choices.is_empty()
        || (hints.ordered_svelte_fragments && hints.directive_group.is_some())
    {
        let reactive = if matches!(target, MoonshineModuleTarget::Ssr) {
            "@tschk/moonshine-dom/ssr"
        } else {
            "@tschk/moonshine-dom/reactive"
        };
        import.push_str(&format!("import {{ getOwner as {owner_alias}, runWithOwner as {run_alias}, untrack as {untrack_alias} }} from {reactive:?};\n"));
        let (generic, parameter, variable) = if source_type.is_typescript() {
            ("<T>", "fn: () => T", "value: T")
        } else {
            ("", "fn", "value")
        };
        import.push_str(&format!("function {once_alias}{generic}({parameter}) {{ const owner = {owner_alias}(); let ready = false; let {variable}; return () => {{ if (!ready) {{ value = {run_alias}(owner, () => {untrack_alias}(fn)); ready = true; }} return value; }}; }}\n"));
    }
    if !inspect.choices.is_empty() {
        let reactive = if matches!(target, MoonshineModuleTarget::Ssr) {
            "@tschk/moonshine-dom/ssr"
        } else {
            "@tschk/moonshine-dom/reactive"
        };
        // Scheduled structural reads must let parent removal dispose child branches first.
        let memo = if authored_svelte && matches!(target, MoonshineModuleTarget::Dom) {
            "createRenderMemo"
        } else {
            "createMemo"
        };
        import.push_str(&format!(
            "import {{ {memo} as {memo_alias} }} from {reactive:?};\n"
        ));
        if condition_memo_alias != memo_alias {
            import.push_str(&format!(
                "import {{ createMemo as {condition_memo_alias} }} from \"@tschk/moonshine\";\n"
            ));
        }
        let attached_selection = authored_svelte && matches!(target, MoonshineModuleTarget::Ssr);
        if !attached_selection {
            let control = if solid_source && matches!(target, MoonshineModuleTarget::Dom) {
                "@tschk/moonshine-solid/control"
            } else {
                "@tschk/moonshine-dom/control"
            };
            let name = if solid_source && matches!(target, MoonshineModuleTarget::Dom) {
                "createSolidShow"
            } else {
                "Show"
            };
            import.push_str(&format!(
                "import {{ {name} as {show_alias} }} from {control:?};\n"
            ));
        }
        let parameters = if source_type.is_typescript() {
            "test: () => (() => any), yes: (read: () => any) => any, no: (read: () => any) => any, nullish = false"
        } else {
            "test, yes, no, nullish = false"
        };
        // Svelte SSR branches must remain under the component owner so bound-prop
        // settlement invalidates them. Cache truthiness separately to retain an
        // existing branch when only its truthy (or non-nullish) raw value changes.
        let selection = if attached_selection {
            format!(
                "const active = {memo_alias}(() => nullish ? raw() != null : !!raw()); return {memo_alias}(() => active() ? yes(raw) : no(raw));"
            )
        } else {
            let schedule = if hints.directive_group.is_some() {
                ", read => read"
            } else {
                ""
            };
            format!(
                "return {show_alias}({{ when: () => nullish ? raw() != null : raw(), children: () => yes(raw), fallback: () => no(raw) }}{schedule});"
            )
        };
        if authored_svelte
            && matches!(target, MoonshineModuleTarget::Dom)
            && hints.ordered_svelte_fragments
        {
            let range = fresh("__crepusChoiceRange");
            let prepare = fresh("__crepusChoicePrepare");
            let scheduler = fresh("__crepusChoiceScheduler");
            let scope = fresh("__crepusChoiceScope");
            import.push_str(&format!("import {{getRenderScheduler as {scheduler},createComponentScope as {scope}}} from \"@tschk/moonshine-dom/reactive\";\n"));
            import.push_str(&format!("import {{createInsertionRange as {range},prepareInsertionRange as {prepare}}} from \"@tschk/moonshine-dom/dom\";\n"));
            let view_type = if source_type.is_typescript() {
                ": ReturnType<typeof setup> | undefined"
            } else {
                ""
            };
            // Retain native setup under the template owner while giving its scheduler
            // group the persistent range job as its structural ancestor.
            import.push_str(&format!("import {{markRuneSourceEffect as {source_effect_alias}}} from \"@tschk/moonshine-svelte/pre\";\n"));
            import.push_str(&format!("function {choose_alias}({parameters}) {{ const owner={owner_alias}();const setup=()=>{{const raw=test();const select={show_alias}({{when:()=>nullish?raw()!=null:raw(),children:()=>{{{source_effect_alias}();return yes(raw);}},fallback:()=>{{{source_effect_alias}();return no(raw);}}}},read=>read,{condition_memo_alias});return()=>{{const branch=select();return branch?.();}};}};let view{view_type};let ready=false;const node={range}(()=>{{if(!ready){{const scheduler={scheduler}();view={run_alias}(owner,()=>{scope}(()=>scheduler?scheduler.branch(setup):setup()));ready=true;}}return view?.();}},{memo_alias});{prepare}(node);return()=>node;}}\n"));
        } else {
            import.push_str(&format!("function {choose_alias}({parameters}) {{ const read = {once_alias}(() => {{ const raw = test(); {selection} }}); return () => {{ const branch = read()(); return branch?.(); }}; }}\n"));
        }
    }
    if inspect.has_component_children {
        if source_type.is_typescript() {
            import.push_str(&format!("function {children_alias}<const T extends readonly (()=>unknown)[],const S extends boolean>(parts:T,single:S):()=>S extends true?ReturnType<T[number]>:ReturnType<T[number]>[] {{return (()=>single?parts[0]():parts.map(fn=>fn())) as ()=>S extends true?ReturnType<T[number]>:ReturnType<T[number]>[];}}\n"));
        } else {
            import.push_str(&format!("function {children_alias}(parts,single) {{ return () => single ? parts[0]() : parts.map(fn => fn()); }}\n"));
        }
    }
    if inspect.has_jsx {
        // All source offsets belong to the original input, including the preamble.
        inspect.edits.push((insertion, insertion, import));
    }
    let mut rewritten = source.to_owned();
    inspect.edits.sort_by_key(|edit| (edit.0, edit.1));
    for (start, end, text) in inspect.edits.into_iter().rev() {
        rewritten.replace_range(start..end, &text);
    }
    let mut directive_block_alias = None;
    if let Some(group) = hints.directive_group {
        let grouped = crate::svelte_directives::lower_groups(
            &rewritten,
            crate::svelte_directives::GroupHints {
                group,
                bindings: hints.directive_bindings,
                intrinsic_reference: hints.intrinsic_reference,
                dynamic_tag_attribute: hints.dynamic_tag_attribute,
                dynamic_namespace_attribute: hints.dynamic_namespace_attribute,
                dynamic_map: hints.dynamic_map,
                class_directive_helper: hints.class_directive_helper,
                each: hints.directive_each,
                directives: hints.directives,
                text: hints.text_component,
                textarea_default_helper: hints.textarea_default_helper,
                choose: &choose_alias,
                once: &once_alias,
                ordered: hints.ordered_svelte_fragments,
                property_keys: hints.property_keys,
                intrinsic_namespace: hints.intrinsic_namespace,
                element_spread_markers: hints.element_spread_markers,
            },
            hints.source_globals.unwrap_or(&Default::default()),
        )?;
        directive_block_alias = Some(grouped.block_alias);
        rewritten = grouped.source;
    }
    if let Some(marker) = hints.const_scope {
        rewritten = crate::svelte_const_emit::prepare(&rewritten, marker, source_type)?;
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &rewritten, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("reactive module parse failed: {:?}", parsed.errors));
    }
    let mut program = parsed.program;
    hints.svelte_ir_policy.configure(&mut program);
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);
    if !semantic.errors.is_empty() {
        return Err(format!("module semantic errors: {:?}", semantic.errors));
    }
    let mut options = TransformOptions::from_target("esnext")
        .map_err(|error| format!("module target: {error}"))?;
    options.jsx = JsxOptions::enable();
    options.jsx.runtime = JsxRuntime::Classic;
    options.jsx.pragma = Some(alias.clone());
    options.jsx.pragma_frag = Some(fragment_alias.clone());
    options.jsx.display_name_plugin = false;
    // Namespace-shaped model listeners were validated as component props above.
    options.jsx.throw_if_namespace = false;
    // Native h establishes reactive effects even when its returned node is unused.
    options.jsx.pure = false;
    let scoping = semantic.semantic.into_scoping();
    // Oxc enables its TypeScript erasure pass from this flag. The AST has
    // already been parsed and semantically checked as TS; retain those nodes
    // while running only JSX lowering, then restore its source type for output.
    program.source_type = program.source_type.with_javascript(true);
    let transformed =
        Transformer::new(&allocator, path, &options).build_with_scoping(scoping, &mut program);
    if !transformed.errors.is_empty() {
        return Err(format!("module transform failed: {:?}", transformed.errors));
    }
    program.source_type = source_type;
    let generated = Codegen::new().build(&program).code;
    let mut cached_helpers = vec![once_alias.as_str(), choose_alias.as_str()];
    cached_helpers.extend(hints.prop_helper);
    let source = component_getters(
        &generated,
        (source_type, target),
        &alias,
        &fragment_alias,
        &cached_helpers,
        &children_alias,
        hints,
    )?;
    let source = if matches!(target, MoonshineModuleTarget::Ssr) {
        transparent_ssr_helpers(&source, source_type, &alias, hints.ssr_transparent_helpers)?
    } else {
        source
    };
    let source = if let Some(marker) = hints.const_scope {
        crate::svelte_const_emit::finish(
            &source,
            marker,
            &alias,
            &fragment_alias,
            directive_block_alias.as_deref(),
            source_type,
        )?
    } else {
        source
    };
    Ok(MoonshineModule {
        styles: Vec::new(),
        source,
        language: if source_type.is_typescript() {
            MoonshineModuleLanguage::TypeScript
        } else {
            MoonshineModuleLanguage::JavaScript
        },
        imports: inspect.imports,
        has_default_export,
        has_module_declarations,
    })
}

/// The admitted Solid namespace is type-only and renderer-specific, not a JSX runtime ABI.
fn solid_jsx_element_types(
    source: &str,
    program: &Program<'_>,
    target: MoonshineModuleTarget,
) -> Result<Option<String>, String> {
    let declarations = program.body.iter().filter_map(|statement| {
        let Statement::ImportDeclaration(import) = statement else { return None; };
        (import.source.value == "solid-js" && import.specifiers.as_ref().is_some_and(|specifiers| {
            specifiers.iter().any(|specifier| matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier)
                if specifier.imported.name() == "JSX" && (import.import_kind == ImportOrExportKind::Type || specifier.import_kind == ImportOrExportKind::Type)))
        })).then_some(import)
    }).collect::<Vec<_>>();
    if declarations.is_empty() {
        return Ok(None);
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(program);
    if !built.errors.is_empty() {
        return Err("invalid Solid JSX type scope".into());
    }
    let semantic = built.semantic;
    let mut aliases = HashSet::new();
    let mut alias_names = HashSet::new();
    let mut edits = Vec::new();
    let (host, child) = match target {
        MoonshineModuleTarget::Dom => ("@tschk/moonshine-dom/dom", "DOMChild"),
        MoonshineModuleTarget::Ssr => ("@tschk/moonshine-dom/ssr", "SSRChild"),
    };
    for import in declarations {
        if import.with_clause.is_some() || import.phase.is_some() {
            return Err("Solid JSX types do not support import attributes".into());
        }
        let mut retained = Vec::new();
        let mut namespaces = String::new();
        for specifier in import
            .specifiers
            .as_ref()
            .expect("selected import has named specifiers")
        {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                return Err("Solid JSX types require named type-only imports".into());
            };
            if specifier.imported.name() == "JSX"
                && (import.import_kind == ImportOrExportKind::Type
                    || specifier.import_kind == ImportOrExportKind::Type)
            {
                alias_names.insert(specifier.local.name.as_str());
                aliases.insert(
                    specifier
                        .local
                        .symbol_id
                        .get()
                        .expect("semantic builder assigns imported type symbols"),
                );
                namespaces.push_str(&format!(
                    "declare namespace {} {{ type Element = import({host:?}).{child}; }}\n",
                    specifier.local.name
                ));
            } else {
                retained.push(&source[specifier.span.start as usize..specifier.span.end as usize]);
            }
        }
        let mut replacement = program
            .comments
            .iter()
            .filter(|comment| {
                import.span.start <= comment.span.start && comment.span.end <= import.span.end
            })
            .map(|comment| {
                format!(
                    "{}\n",
                    &source[comment.span.start as usize..comment.span.end as usize]
                )
            })
            .collect::<String>();
        if !retained.is_empty() {
            replacement.push_str(&format!(
                "import {}{{{}}} from 'solid-js';\n",
                if import.import_kind == ImportOrExportKind::Type {
                    "type "
                } else {
                    ""
                },
                retained.join(",")
            ));
        }
        replacement.push_str(&namespaces);
        edits.push((
            import.span.start as usize,
            import.span.end as usize,
            replacement,
        ));
    }
    let nodes = semantic.nodes();
    for node in nodes.iter() {
        let oxc_ast::AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic.scoping().get_reference(
            id.reference_id
                .get()
                .expect("semantic builder assigns type references"),
        );
        let imported = reference
            .symbol_id()
            .is_some_and(|symbol| aliases.contains(&symbol));
        let invalid_value =
            reference.symbol_id().is_none() && alias_names.contains(id.name.as_str());
        if !imported && !invalid_value {
            continue;
        }
        let mut qualified = None;
        for parent in nodes.ancestors(node.id()) {
            if let oxc_ast::AstKind::TSQualifiedName(name) = parent.kind() {
                if qualified.is_some()
                    || name.left.span() != id.span
                    || name.right.name != "Element"
                {
                    return Err("Solid JSX type bridge supports only JSX.Element".into());
                }
                qualified = Some(name.span);
            }
            if let oxc_ast::AstKind::TSTypeReference(ty) = parent.kind() {
                if ty.type_arguments.is_some() {
                    return Err("Solid JSX.Element does not accept type arguments".into());
                }
                break;
            }
        }
        if qualified.is_none() || !reference.is_type() || reference.flags().is_value_as_type() {
            return Err("Solid JSX namespace is type-only; only JSX.Element is supported".into());
        }
    }
    let mut output = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok(Some(output))
}

/// Detect authored modules from Oxc's module record, including parse recovery.
/// Template JSX without import/export syntax or an explicit selector stays on the template frontend.
pub fn try_compile_moonshine_module(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<Option<MoonshineModule>, String> {
    if !is_authored_module(source, path)? {
        return Ok(None);
    }
    compile_moonshine_module(source, path, target).map(Some)
}

/// Detect authored entries and verify imported Solid components through the source resolver.
pub fn try_compile_moonshine_module_with_resolver(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    resolver: &mut dyn FnMut(
        &Path,
        &str,
    ) -> Result<
        crate::solid_component_resolver::SolidComponentModule,
        String,
    >,
) -> Result<Option<MoonshineModule>, String> {
    if !is_authored_module(source, path)? {
        return Ok(None);
    }
    compile_moonshine_module_with_resolver(source, path, target, resolver).map(Some)
}

fn is_authored_module(source: &str, path: &Path) -> Result<bool, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(
        path.with_extension(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
    )
    .map_err(|error| error.to_string())?
    .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.module_record.has_module_syntax
        && !crate::solid_source::has_selector(source, &parsed.program)
    {
        return Ok(false);
    }
    Ok(true)
}

/// Compiler-owned SSR helpers share their authored consumer's context identity.
/// Run after descriptor/implicit-child normalization, never on arbitrary components.
fn transparent_ssr_helpers(
    source: &str,
    source_type: SourceType,
    host: &str,
    helpers: &[String],
) -> Result<String, String> {
    if helpers.is_empty() {
        return Ok(source.to_owned());
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid emitted SSR helper syntax".into());
    }
    // Trusted, previously checked IR: build bindings without imposing TS-only
    // duplicate-member restrictions on authored JavaScript retained in the IR.
    let built = SemanticBuilder::new().build(&parsed.program);
    let semantic = built.semantic;
    let scopes = semantic.scoping();
    let root = scopes.root_scope_id();
    let host_symbol = scopes.get_binding(root, host.into());
    let mut edits = Vec::new();
    for node in semantic.nodes().iter() {
        let oxc_ast::AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::Identifier(callee) = call.callee.get_inner_expression() else {
            continue;
        };
        if callee.name != host
            || host_symbol.is_none()
            || scopes
                .get_reference(callee.reference_id.get().expect("bound host reference"))
                .symbol_id()
                != host_symbol
        {
            continue;
        }
        let Some(Argument::Identifier(helper)) = call.arguments.first() else {
            continue;
        };
        if !helpers.iter().any(|name| name == helper.name.as_str()) {
            continue;
        }
        let symbol = scopes.get_binding(root, helper.name.as_str().into());
        if symbol.is_none()
            || scopes
                .get_reference(helper.reference_id.get().expect("bound helper reference"))
                .symbol_id()
                != symbol
        {
            continue;
        }
        if call.optional
            || call.arguments.len() != 2
            || matches!(call.arguments.get(1), Some(Argument::SpreadElement(_)))
        {
            return Err("invalid normalized SSR helper call".into());
        }
        let argument = &call.arguments[1];
        let props = argument.span();
        // Match h's nullish-to-empty-object contract. Object literals cannot be
        // nullish; other expressions are evaluated once, without assuming that
        // future normalized descriptor producers can never return undefined.
        let nullish = !matches!(
            argument,
            Argument::ObjectExpression(_) | Argument::NullLiteral(_)
        );
        if matches!(argument, Argument::NullLiteral(_)) {
            edits.push((props.start as usize, props.end as usize, "{}".into()));
        } else if nullish {
            edits.push((props.end as usize, props.end as usize, " ?? {})".into()));
        }
        let trivia = parsed
            .program
            .comments
            .iter()
            .filter(|comment| {
                comment.span.start >= call.span.start && comment.span.end <= props.start
            })
            .map(|comment| {
                format!(
                    "{}\n",
                    &source[comment.span.start as usize..comment.span.end as usize]
                )
            })
            .collect::<String>();
        edits.push((
            call.span.start as usize,
            props.start as usize,
            format!("{trivia}{}({}", helper.name, if nullish { "(" } else { "" }),
        ));
    }
    let mut output = source.to_owned();
    edits.sort_by_key(|edit| edit.0);
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    Ok(output)
}

// The host reads component props as ordinary values. Getters retain that API
// while tracking caller expressions in reactive reads. Arrow descriptor getters
// preserve the authored lexical this/arguments/super context.
fn component_getters(
    source: &str,
    source_kind: (SourceType, MoonshineModuleTarget),
    alias: &str,
    fragment_alias: &str,
    cached_helpers: &[&str],
    children_alias: &str,
    hints: ModuleHints<'_>,
) -> Result<String, String> {
    let (source_type, target) = source_kind;
    let realm = hints
        .source_globals
        .map_or("globalThis", |globals| globals.realm.as_str());
    let types = hints
        .source_globals
        .map_or("globalThis", |globals| globals.types.as_str());
    let mut textarea_memo = "__crepusTextareaMemo".to_string();
    while source.contains(&textarea_memo) {
        textarea_memo.push('_');
    }
    let mut textarea_runtime = "__crepusTextareaRenderMemo".to_string();
    while source.contains(&textarea_runtime) {
        textarea_runtime.push('_');
    }
    let mut textarea_owner = "__crepusTextareaGetOwner".to_string();
    while source.contains(&textarea_owner) {
        textarea_owner.push('_');
    }
    let mut textarea_run = "__crepusTextareaRunWithOwner".to_string();
    while source.contains(&textarea_run) {
        textarea_run.push('_');
    }
    let mut props_alias = "__crepusComponentProps".to_string();
    while source.contains(&props_alias) {
        props_alias.push('_');
    }
    struct Getters<'s> {
        source: &'s str,
        realm: &'s str,
        types: &'s str,
        props_alias: &'s str,
        typed_props: bool,
        props_used: bool,
        alias: &'s str,
        fragment_alias: &'s str,
        intrinsic_namespace: Option<&'s str>,
        dynamic_tag_attribute: Option<&'s str>,
        dynamic_namespace_attribute: Option<&'s str>,
        namespace_alias: &'s str,
        namespace_used: bool,
        namespace_error: bool,
        source_alias: &'s str,
        source_child: &'s str,
        source_used: bool,
        ssr: bool,
        host_references: usize,
        direct_fragments: usize,
        cached_helpers: &'s [&'s str],
        children_alias: &'s str,
        property_keys: &'s [(&'s str, &'s str)],
        text_component: Option<&'s str>,
        svelte_component_props: bool,
        lazy_prop_markers: &'s [&'s str],
        edits: Vec<(usize, usize, String)>,
        asserted_assignments: bool,
        textarea_enabled: bool,
        textarea_memo: &'s str,
        textarea_used: bool,
        comments: Vec<oxc_span::Span>,
    }
    impl Getters<'_> {
        fn intrinsic_value(&mut self, node: &CallExpression<'_>) {
            if !self.textarea_enabled
                || !node.callee.is_specific_id(self.alias)
                || !matches!(node.arguments.first(), Some(Argument::StringLiteral(_)))
            {
                return;
            }
            let Some(Argument::ObjectExpression(props)) = node.arguments.get(1) else {
                return;
            };
            for property in &props.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                if property.computed
                    || property
                        .key
                        .static_name()
                        .is_none_or(|name| name != "value")
                    || !matches!(
                        property.value.get_inner_expression(),
                        Expression::ArrowFunctionExpression(_)
                    )
                {
                    continue;
                }
                // Retain the component owner while lazily caching the field value,
                // so replacing a row object with the same field preserves a dirty input.
                let span = property.value.span();
                self.edits.push((
                    span.start as usize,
                    span.start as usize,
                    format!("{}(", self.textarea_memo),
                ));
                self.edits
                    .push((span.end as usize, span.end as usize, ")".into()));
                self.textarea_used = true;
            }
        }
        fn textarea_value(&mut self, node: &CallExpression<'_>) {
            if !self.textarea_enabled
                || !node.callee.is_specific_id(self.alias)
                || node.arguments.len() != 3
            {
                return;
            }
            if !matches!(node.arguments.first(), Some(Argument::StringLiteral(tag)) if tag.value=="textarea")
            {
                return;
            }
            let Some(Argument::CallExpression(child)) = node.arguments.get(2) else {
                return;
            };
            if !child.callee.is_specific_id(self.alias)
                || !matches!(child.arguments.first(), Some(Argument::Identifier(name)) if Some(name.name.as_str())==self.text_component)
            {
                return;
            }
            let (start, prefix) = match &node.arguments[1] {
                Argument::NullLiteral(value) => (value.span.start as usize, "{value:"),
                Argument::ObjectExpression(object) => {
                    if object.properties.iter().any(|property| matches!(property, ObjectPropertyKind::ObjectProperty(property) if property.key.static_name().is_some_and(|name|name=="value"))) { return; }
                    match object.properties.last() {
                        Some(property) => {
                            // Namespace metadata is removed by the sibling edit pass.
                            // Keep its original end offset, but do not leave a leading
                            // comma when it was the only property in the object.
                            let only_namespace = object.properties.iter().all(|entry| {
                                matches!(entry, ObjectPropertyKind::ObjectProperty(property)
                                    if self.intrinsic_namespace.is_some_and(|marker|
                                        property.key.static_name().is_some_and(|key| key == marker)))
                            });
                            (
                                property.span().end as usize,
                                if only_namespace { "value:" } else { ",value:" },
                            )
                        }
                        None => (object.span.start as usize + 1, "value:"),
                    }
                }
                _ => return,
            };
            let child_span = child.span;
            let trivia = self
                .comments
                .iter()
                .filter(|span| span.start as usize >= start && span.end <= child_span.start)
                .map(|span| format!("{}\n", &self.source[span.start as usize..span.end as usize]))
                .collect::<String>();
            self.edits.push((
                start,
                child_span.start as usize,
                format!("{trivia}{prefix}{}(", self.textarea_memo),
            ));
            self.edits.push((
                child_span.end as usize,
                child_span.end as usize,
                ")}".into(),
            ));
            self.textarea_used = true;
        }
    }
    impl<'a> Visit<'a> for Getters<'_> {
        fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
            self.host_references += usize::from(node.name == self.alias);
        }
        fn visit_assignment_expression(&mut self, node: &AssignmentExpression<'a>) {
            self.asserted_assignments |= matches!(
                &node.left,
                AssignmentTarget::TSAsExpression(_) | AssignmentTarget::TSSatisfiesExpression(_)
            );
            walk::walk_assignment_expression(self, node);
        }
        fn visit_call_expression(&mut self, node: &CallExpression<'a>) {
            if let Some(marker) = self.intrinsic_namespace {
                if node.callee.is_specific_id(self.alias) {
                    if let Some(Argument::ObjectExpression(props)) = node.arguments.get(1) {
                        for (index, entry) in props.properties.iter().enumerate() {
                            let ObjectPropertyKind::ObjectProperty(property) = entry else {
                                continue;
                            };
                            if !property.key.static_name().is_some_and(|key| key == marker) {
                                continue;
                            }
                            let valid = matches!(
                                node.arguments.first(),
                                Some(Argument::StringLiteral(_))
                            ) && matches!(&property.value, Expression::StringLiteral(value) if matches!(value.value.as_str(), "http://www.w3.org/1999/xhtml" | "http://www.w3.org/2000/svg" | "http://www.w3.org/1998/Math/MathML"));
                            if !valid {
                                self.namespace_error = true;
                                continue;
                            }
                            let dynamic = props.properties.iter().find_map(|entry| {
                                let ObjectPropertyKind::ObjectProperty(property) = entry else {
                                    return None;
                                };
                                (property.key.static_name().is_some_and(|name| {
                                    Some(name.as_ref()) == self.dynamic_tag_attribute
                                }))
                                .then_some(&property.value)
                            });
                            let dynamic_namespace = props.properties.iter().find_map(|entry| {
                                let ObjectPropertyKind::ObjectProperty(property) = entry else {
                                    return None;
                                };
                                property
                                    .key
                                    .static_name()
                                    .is_some_and(|name| {
                                        Some(name.as_ref()) == self.dynamic_namespace_attribute
                                    })
                                    .then_some(&property.value)
                            });
                            if dynamic_namespace
                                .is_some_and(|value| !matches!(value, Expression::Identifier(_)))
                                || (dynamic_namespace.is_some() && dynamic.is_none())
                            {
                                self.namespace_error = true;
                                continue;
                            }
                            if let Some(value) = dynamic {
                                if !self.ssr {
                                    let tag = &self.source
                                        [value.span().start as usize..value.span().end as usize];
                                    let namespace = match &property.value {
                                        Expression::StringLiteral(value) => value.value.as_str(),
                                        _ => "",
                                    };
                                    let element_type = crate::source_globals::dynamic_element_type(
                                        self.types, tag, namespace,
                                    );
                                    self.edits.push((
                                        node.span.start as usize,
                                        node.span.start as usize,
                                        "(".into(),
                                    ));
                                    self.edits.push((
                                        node.span.end as usize,
                                        node.span.end as usize,
                                        format!(") as {element_type}"),
                                    ));
                                }
                                if !matches!(value, Expression::Identifier(_)) {
                                    self.namespace_error = true;
                                    continue;
                                }
                                let span = node.arguments[0].span();
                                self.edits.push((
                                    span.start as usize,
                                    span.end as usize,
                                    self.source
                                        [value.span().start as usize..value.span().end as usize]
                                        .to_owned(),
                                ));
                                let metadata = |entry: &ObjectPropertyKind<'_>| matches!(entry, ObjectPropertyKind::ObjectProperty(property) if property.key.static_name().is_some_and(|name| name == marker || Some(name.as_ref()) == self.dynamic_tag_attribute || Some(name.as_ref()) == self.dynamic_namespace_attribute));
                                let mut at = 0;
                                while at < props.properties.len() {
                                    if !metadata(&props.properties[at]) {
                                        at += 1;
                                        continue;
                                    }
                                    let first = at;
                                    while at < props.properties.len()
                                        && metadata(&props.properties[at])
                                    {
                                        at += 1;
                                    }
                                    let begin = if at == props.properties.len() && first > 0 {
                                        props.properties[first - 1].span().end
                                    } else {
                                        props.properties[first].span().start
                                    };
                                    let end = if at < props.properties.len() {
                                        props.properties[at].span().start
                                    } else {
                                        props.properties[at - 1].span().end
                                    };
                                    let trivia = self
                                        .comments
                                        .iter()
                                        .filter(|span| span.start >= begin && span.end <= end)
                                        .map(|span| {
                                            format!(
                                                "{}\n",
                                                &self.source
                                                    [span.start as usize..span.end as usize]
                                            )
                                        })
                                        .collect::<String>();
                                    self.edits.push((begin as usize, end as usize, trivia));
                                }
                            } else {
                                let start = if index + 1 == props.properties.len() && index > 0 {
                                    props.properties[index - 1].span().end
                                } else {
                                    property.span.start
                                };
                                let end = if index + 1 < props.properties.len() {
                                    props.properties[index + 1].span().start
                                } else {
                                    property.span.end
                                };
                                let trivia = self
                                    .comments
                                    .iter()
                                    .filter(|span| span.start >= start && span.end <= end)
                                    .map(|span| {
                                        format!(
                                            "{}\n",
                                            &self.source[span.start as usize..span.end as usize]
                                        )
                                    })
                                    .collect::<String>();
                                self.edits.push((start as usize, end as usize, trivia));
                            }
                            if !self.ssr {
                                let callee = node.callee.span();
                                self.edits.push((
                                    callee.start as usize,
                                    callee.end as usize,
                                    self.namespace_alias.to_owned(),
                                ));
                                let tag = node.arguments[0].span();
                                self.edits.push((
                                    tag.start as usize,
                                    tag.start as usize,
                                    if let Some(namespace) = dynamic_namespace {
                                        let namespace = &self.source[namespace.span().start as usize..namespace.span().end as usize];
                                        format!("{namespace},")
                                    } else if let Some(value) = dynamic {
                                        let tag = &self.source[value.span().start as usize..value.span().end as usize];
                                        let namespace = &self.source[property.value.span().start as usize..property.value.span().end as usize];
                                        if matches!(&property.value, Expression::StringLiteral(value) if value.value == "http://www.w3.org/1999/xhtml") {
                                            format!("(({tag} as string)===\"svg\"?\"http://www.w3.org/2000/svg\":{namespace}),")
                                        } else { format!("{namespace},") }
                                    } else {
                                        format!("{},", &self.source[property.value.span().start as usize..property.value.span().end as usize])
                                    },
                                ));
                                self.namespace_used = true;
                            } else if self.svelte_component_props {
                                // The physical compiler marker proves intrinsic provenance;
                                // component/function tags retain the existing host factory.
                                let callee = node.callee.span();
                                self.edits.push((
                                    callee.start as usize,
                                    callee.end as usize,
                                    self.source_alias.to_owned(),
                                ));
                                for child in node.arguments.iter().skip(2) {
                                    let span = child.span();
                                    let accessor = child.as_expression().is_some_and(|value| {
                                        matches!(
                                            value.get_inner_expression(),
                                            Expression::ArrowFunctionExpression(_)
                                        )
                                    });
                                    self.edits.push((
                                        span.start as usize,
                                        span.start as usize,
                                        if accessor {
                                            format!("{}(", self.source_child)
                                        } else {
                                            format!("{}(()=> (", self.source_child)
                                        },
                                    ));
                                    self.edits.push((
                                        span.end as usize,
                                        span.end as usize,
                                        if accessor { ")".into() } else { "))".into() },
                                    ));
                                }
                                self.source_used = true;
                            }
                        }
                    }
                }
            }
            if self.ssr
                && node.callee.is_specific_id(self.alias)
                && matches!(node.arguments.first(), Some(Argument::Identifier(name)) if name.name == self.fragment_alias)
            {
                if let Some(Argument::NullLiteral(props)) = node.arguments.get(1) {
                    self.direct_fragments += 1;
                    // Compiler-owned fragments are transparent; real components retain h's scope.
                    let trivia = self
                        .comments
                        .iter()
                        .filter(|span| {
                            span.start >= node.span.start && span.end <= props.span.start
                        })
                        .map(|span| {
                            format!("{}\n", &self.source[span.start as usize..span.end as usize])
                        })
                        .collect::<String>();
                    self.edits.push((
                        node.span.start as usize,
                        props.span.start as usize,
                        format!("{trivia}{}(", self.fragment_alias),
                    ));
                }
            }
            self.intrinsic_value(node);
            self.textarea_value(node);
            if node.callee.is_specific_id(self.alias) {
                if let Some(Argument::ObjectExpression(object)) = node.arguments.get(1) {
                    for property in &object.properties {
                        let ObjectPropertyKind::ObjectProperty(property) = property else {
                            continue;
                        };
                        for (marker, expression) in self.property_keys {
                            if matches!(&property.key, PropertyKey::StaticIdentifier(id) if id.name == *marker)
                            {
                                let key = property.key.span();
                                self.edits.push((
                                    key.start as usize,
                                    key.end as usize,
                                    format!("[{expression}]"),
                                ));
                            }
                        }
                    }
                }
            }
            if node.callee.is_specific_id(self.alias)
                && !matches!(node.arguments.first(), Some(Argument::Identifier(name)) if Some(name.name.as_str()) == self.text_component)
                && !matches!(node.arguments.first(), Some(Argument::Identifier(name)) if name.name == self.fragment_alias)
                && matches!(
                    node.arguments.first(),
                    Some(Argument::Identifier(_) | Argument::StaticMemberExpression(_))
                )
            {
                let props_open = if self.typed_props {
                    self.props_used = true;
                    let callee = node.callee.span();
                    self.edits.push((
                        callee.start as usize,
                        callee.end as usize,
                        self.props_alias.to_owned(),
                    ));
                    format!("{}Descriptors(", self.props_alias)
                } else {
                    format!(
                        "{realm}.Object.create({realm}.Object.prototype, ",
                        realm = self.realm
                    )
                };
                if let Some(props) = node.arguments.get(1) {
                    if let Argument::ObjectExpression(object) = props {
                        for property in &object.properties {
                            if let ObjectPropertyKind::ObjectProperty(property) = property {
                                let key = property.key.span();
                                let value = property.value.span();
                                let prototype_key = match &property.key {
                                    PropertyKey::StaticIdentifier(name) => name.name == "__proto__",
                                    PropertyKey::StringLiteral(name) => name.value == "__proto__",
                                    _ => false,
                                };
                                if self.svelte_component_props
                                    && !property.computed
                                    && prototype_key
                                {
                                    // An ordinary component prop must be an own
                                    // descriptor key, never an object prototype setter.
                                    if !self
                                        .property_keys
                                        .iter()
                                        .any(|(marker, _)| *marker == "__proto__")
                                    {
                                        self.edits.push((
                                            key.start as usize,
                                            key.end as usize,
                                            "[\"__proto__\"]".into(),
                                        ));
                                    }
                                }
                                let eager = self.svelte_component_props
                                    && self.ssr
                                    && !property.key.static_name().is_some_and(|name| {
                                        self.lazy_prop_markers.contains(&name.as_ref())
                                    });
                                // Classic JSX codegen can compact item: item into
                                // shorthand. Its key/value spans overlap, so copy
                                // the identifier into one insertion after the key.
                                if property.shorthand {
                                    let value =
                                        &self.source[value.start as usize..value.end as usize];
                                    self.edits.push((
                                        key.end as usize,
                                        key.end as usize,
                                        if eager {
                                            format!(": {{ enumerable: true, value: ({value}) }}")
                                        } else {
                                            format!(
                                                ": {{ enumerable: true, get: () => ({value}) }}"
                                            )
                                        },
                                    ));
                                    continue;
                                }
                                let cached = matches!(property.value.get_inner_expression(), Expression::CallExpression(call) if self.cached_helpers.iter().any(|helper| call.callee.is_specific_id(helper)));
                                self.edits.push((
                                    key.end as usize,
                                    value.start as usize,
                                    if eager && !cached {
                                        // SSR evaluates named props in the parent even when unread.
                                        ": { enumerable: true, value: (".into()
                                    } else if cached {
                                        ": { enumerable: true, get: ".into()
                                    } else {
                                        ": { enumerable: true, get: () => (".into()
                                    },
                                ));
                                self.edits.push((
                                    value.end as usize,
                                    value.end as usize,
                                    if cached { " }".into() } else { ") }".into() },
                                ));
                            }
                        }
                    }
                    if let Argument::ObjectExpression(object) = props {
                        self.edits.push((
                            object.span.start as usize,
                            object.span.start as usize,
                            props_open.clone(),
                        ));
                        if node.arguments.len() == 2 {
                            self.edits.push((
                                object.span.end as usize,
                                object.span.end as usize,
                                ")".into(),
                            ));
                        }
                    }
                    if node.arguments.len() == 2 && matches!(props, Argument::NullLiteral(_)) {
                        let span = props.span();
                        self.edits
                            .push((span.start as usize, span.end as usize, "{}".into()));
                    }
                    if node.arguments.len() > 2 {
                        let first = node.arguments[2].span();
                        let last = node.arguments.last().unwrap().span();
                        let (start, comma) = match props {
                            Argument::ObjectExpression(object) => (
                                object.span.end as usize - 1,
                                if object.properties.is_empty() {
                                    ""
                                } else {
                                    ","
                                },
                            ),
                            _ => (props.span().start as usize, ""),
                        };
                        let open = if matches!(props, Argument::ObjectExpression(_)) {
                            String::new()
                        } else {
                            format!("{props_open}{{")
                        };
                        self.edits.push((
                            start,
                            first.start as usize,
                            format!(
                                "{open}{comma}children: {{ enumerable: true, get: {}([",
                                self.children_alias
                            ),
                        ));
                        for argument in node.arguments.iter().skip(2) {
                            if !argument.as_expression().is_some_and(|value| matches!(value.get_inner_expression(), Expression::CallExpression(call) if self.cached_helpers.iter().any(|helper| call.callee.is_specific_id(helper))))
                            {
                                let span = argument.span();
                                self.edits.push((
                                    span.start as usize,
                                    span.start as usize,
                                    "() => (".into(),
                                ));
                                self.edits
                                    .push((span.end as usize, span.end as usize, ")".into()));
                            }
                        }
                        self.edits.push((
                            last.end as usize,
                            last.end as usize,
                            format!("], {}) }}}})", node.arguments.len() == 3),
                        ));
                    }
                }
            }
            walk::walk_call_expression(self, node);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("lowered module parse failed: {:?}", parsed.errors));
    }
    // Codegen normally decodes names, but this pass also accepts original
    // spellings. Reserve semantic names so an authored Unicode escape cannot
    // capture the compiler-owned import alias.
    let reserved = if hints.intrinsic_namespace.is_some() {
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        semantic
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                oxc_ast::AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
                oxc_ast::AstKind::IdentifierReference(id) => Some(id.name.to_string()),
                _ => None,
            })
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let mut namespace_alias = "__crepusIntrinsicNS".to_owned();
    while reserved.contains(namespace_alias.as_str()) || source.contains(&namespace_alias) {
        namespace_alias.push('_');
    }
    let fresh_source = |base: &str| {
        let mut name = base.to_owned();
        while reserved.contains(&name) || source.contains(&name) {
            name.push('_');
        }
        name
    };
    let source_alias = fresh_source("__crepusSourceIntrinsic");
    let source_child = fresh_source("__crepusSourceChild");
    let source_peek = fresh_source("__crepusSourceConsumer");
    let source_enter = fresh_source("__crepusSourceWithConsumer");
    let mut getters = Getters {
        source,
        realm,
        types: hints
            .source_globals
            .map_or("globalThis", |globals| globals.types.as_str()),
        props_alias: &props_alias,
        typed_props: source_type.is_typescript() && !hints.svelte_component_props,
        props_used: false,
        alias,
        fragment_alias,
        intrinsic_namespace: hints.intrinsic_namespace,
        dynamic_tag_attribute: hints.dynamic_tag_attribute,
        dynamic_namespace_attribute: hints.dynamic_namespace_attribute,
        namespace_alias: &namespace_alias,
        namespace_used: false,
        namespace_error: false,
        source_alias: &source_alias,
        source_child: &source_child,
        source_used: false,
        ssr: matches!(target, MoonshineModuleTarget::Ssr),
        host_references: 0,
        direct_fragments: 0,
        cached_helpers,
        children_alias,
        property_keys: hints.property_keys,
        text_component: hints.text_component,
        svelte_component_props: hints.svelte_component_props,
        lazy_prop_markers: hints.lazy_prop_markers,
        edits: Vec::new(),
        asserted_assignments: false,
        textarea_enabled: hints.svelte_component_props
            && matches!(target, MoonshineModuleTarget::Dom),
        textarea_memo: &textarea_memo,
        textarea_used: false,
        comments: parsed
            .program
            .comments
            .iter()
            .map(|comment| comment.span)
            .collect(),
    };
    getters.visit_program(&parsed.program);
    if getters.namespace_error {
        return Err("invalid compiler-owned intrinsic namespace metadata".into());
    }
    if getters.direct_fragments > 0 && getters.host_references == getters.direct_fragments {
        for statement in &parsed.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.specifiers.as_ref().is_some_and(|specifiers| specifiers.iter().any(|specifier| matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.local.name == alias))) {
                getters.edits.push((import.span.start as usize, import.span.end as usize, String::new()));
            }
        }
    }
    let textarea_used = getters.textarea_used;
    let mut output = source.to_owned();
    getters.edits.sort_by_key(|edit| (edit.0, edit.1));
    for (start, end, value) in getters.edits.into_iter().rev() {
        output.replace_range(start..end, &value);
    }
    if getters.source_used {
        let (generic, parameter) = if source_type.is_typescript() {
            ("<T>", "read:()=>T")
        } else {
            ("", "read")
        };
        output = format!("import {{hSource as {source_alias}}} from \"@tschk/moonshine-dom/ssr\";\nimport {{peekComponentContext as {source_peek},withComponentConsumer as {source_enter}}} from \"@tschk/moonshine-dom/component\";\nfunction {source_child}{generic}({parameter}){{const consumer={source_peek}();return()=>consumer?{source_enter}(consumer,read):read();}}\n{output}");
    }
    if getters.namespace_used {
        let namespace_factory = if hints.dynamic_namespace_attribute.is_some() {
            "hSourceNS"
        } else {
            "hNS"
        };
        output = format!(
            "import {{{namespace_factory} as {namespace_alias}}} from \"@tschk/moonshine-dom/dom\";\n{output}"
        );
    }
    if getters.props_used {
        let (host, child) = match target {
            MoonshineModuleTarget::Dom => ("@tschk/moonshine-dom/dom", "DOMChild"),
            MoonshineModuleTarget::Ssr => ("@tschk/moonshine-dom/ssr", "SSRChild"),
        };
        let helpers = format!(
            "\nfunction {props_alias}Descriptors<D>(value:D):{{[K in keyof {types}.NoInfer<D> as K extends symbol ? never : K]:{types}.NoInfer<D>[K]}}{{return value;}}\nfunction {props_alias}<P extends object>(component:(props:P)=>import({host:?}).{child},descriptors:{{[K in keyof P]:{{enumerable?:boolean,get:()=>P[K]}}}}):import({host:?}).{child}{{return {alias}(component,{realm}.Object.create({realm}.Object.prototype,descriptors));}}\n"
        );
        output.insert_str(module_preamble_end(&parsed.program), &helpers);
    }
    if textarea_used {
        let (reader_type, memo_type) = if source_type.is_typescript() {
            (": any", ": any")
        } else {
            ("", "")
        };
        // Attach the lazy memo to the construction owner, not the transient value effect.
        output = format!(
            "import {{createRenderMemo as {textarea_runtime},getOwner as {textarea_owner},runWithOwner as {textarea_run}}} from \"@tschk/moonshine-dom/reactive\";\nfunction {textarea_memo}(read{reader_type}){{const owner={textarea_owner}();let memo{memo_type};return()=>{{if(!memo)memo={textarea_run}(owner,()=>{textarea_runtime}(read));return memo();}};}}\n{output}"
        );
    }
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, &output, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("component props parse failed: {:?}", parsed.errors));
    }
    hints.svelte_ir_policy.configure(&mut parsed.program);
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !semantic.errors.is_empty() {
        return Err(format!(
            "component props semantic errors: {:?}",
            semantic.errors
        ));
    }
    let mut output = Codegen::new().build(&parsed.program).code;
    if getters.asserted_assignments {
        // Oxc drops required TS assignment-target parentheses on its final print.
        struct Parentheses(Vec<Span>);
        impl<'a> Visit<'a> for Parentheses {
            fn visit_assignment_expression(&mut self, node: &AssignmentExpression<'a>) {
                if matches!(
                    &node.left,
                    AssignmentTarget::TSAsExpression(_)
                        | AssignmentTarget::TSSatisfiesExpression(_)
                ) {
                    self.0.push(node.left.span());
                }
                walk::walk_assignment_expression(self, node);
            }
        }
        let parsed = Parser::new(&allocator, &output, source_type).parse();
        if !parsed.errors.is_empty() {
            return Err(format!(
                "printed assignment parse failed: {:?}",
                parsed.errors
            ));
        }
        let mut parentheses = Parentheses(Vec::new());
        parentheses.visit_program(&parsed.program);
        parentheses.0.sort_by_key(|span| span.start);
        for span in parentheses.0.into_iter().rev() {
            output.insert(span.end as usize, ')');
            output.insert(span.start as usize, '(');
        }
    }
    Ok(output)
}

#[cfg(test)]
mod svelte_component_prop_tests {
    use super::*;
    #[test]
    fn source_choices_mark_only_owned_svelte_dom_branch_factories() {
        let source =
            "export default function App(){return <>{shown ? <><p/></> : <><span/></>}</>}";
        let hints = ModuleHints {
            ordered_svelte_fragments: true,
            ..ModuleHints::default()
        };
        let dom = compile_moonshine_svelte_module(
            source,
            Path::new("choice.tsx"),
            MoonshineModuleTarget::Dom,
            hints,
        )
        .unwrap()
        .source;
        assert!(
            dom.contains("markRuneSourceEffect as __crepusSourceEffect0"),
            "{dom}"
        );
        assert_eq!(dom.matches("__crepusSourceEffect0()").count(), 2, "{dom}");
        let server = compile_moonshine_svelte_module(
            source,
            Path::new("choice.tsx"),
            MoonshineModuleTarget::Ssr,
            hints,
        )
        .unwrap()
        .source;
        assert!(!server.contains("markRuneSourceEffect"), "{server}");
        let generic =
            compile_moonshine_module(source, Path::new("choice.tsx"), MoonshineModuleTarget::Dom)
                .unwrap()
                .source;
        assert!(!generic.contains("markRuneSourceEffect"), "{generic}");
    }
    #[test]
    fn source_namespace_witness_admits_foreign_intrinsics_but_plain_jsx_does_not() {
        for tag in ["circle", "path", "mi"] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let source = format!(
                    "export default()=> <{tag} Witness=\"http://www.w3.org/1999/xhtml\"/>;"
                );
                assert!(compile_moonshine_svelte_module(
                    &source,
                    Path::new("source.tsx"),
                    target,
                    ModuleHints {
                        svelte_component_props: true,
                        ordered_svelte_fragments: true,
                        intrinsic_namespace: Some("Witness"),
                        ..ModuleHints::default()
                    }
                )
                .is_ok());
                assert!(compile_moonshine_module(
                    &format!("export default()=> <{tag}/>;"),
                    Path::new("plain.tsx"),
                    target
                )
                .is_err());
            }
        }
    }

    #[test]
    fn source_ssr_svg_requires_a_valid_physical_namespace_witness() {
        for tag in ["svg", "g", "foreignObject"] {
            for attributes in ["", "Witness=\"urn:unknown\""] {
                let source =
                    format!("export default function App(){{return <{tag} {attributes}/>}}");
                assert!(
                    compile_moonshine_svelte_module(
                        &source,
                        Path::new("witness.tsx"),
                        MoonshineModuleTarget::Ssr,
                        ModuleHints {
                            intrinsic_namespace: Some("Witness"),
                            svelte_component_props: true,
                            ..ModuleHints::default()
                        }
                    )
                    .is_err(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn intrinsic_namespace_alias_reserves_decoded_authored_identifiers() {
        let source = r#"const __crepusIntrinsic\u004eS=7;h("div",{Witness:"http://www.w3.org/1999/xhtml"});"#;
        let output = component_getters(
            source,
            (SourceType::tsx(), MoonshineModuleTarget::Dom),
            "h",
            "Fragment",
            &[],
            "children",
            ModuleHints {
                intrinsic_namespace: Some("Witness"),
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(output.contains("hNS as __crepusIntrinsicNS_"), "{output}");
        assert!(
            output.contains("__crepusIntrinsicNS_(\"http://www.w3.org/1999/xhtml\""),
            "{output}"
        );
        assert!(!output.contains("Witness:"), "{output}");
    }

    #[test]
    fn intrinsic_namespace_metadata_is_validated_then_erased_on_server() {
        let hints = ModuleHints {
            intrinsic_namespace: Some("Witness"),
            ..ModuleHints::default()
        };
        let output = component_getters(
            "h(\"a\",{Witness:\"http://www.w3.org/1999/xhtml\",title:label})",
            (SourceType::tsx(), MoonshineModuleTarget::Ssr),
            "h",
            "Fragment",
            &[],
            "children",
            hints,
        )
        .unwrap();
        assert!(!output.contains("Witness"), "{output}");
        assert!(!output.contains("hNS"), "{output}");
        assert!(output.contains("title: label"), "{output}");
        let invalid = component_getters(
            "h(\"a\",{Witness:\"unowned\"})",
            (SourceType::tsx(), MoonshineModuleTarget::Ssr),
            "h",
            "Fragment",
            &[],
            "children",
            hints,
        )
        .unwrap_err();
        assert!(
            invalid.contains("invalid compiler-owned intrinsic namespace metadata"),
            "{invalid}"
        );
    }

    #[test]
    fn authored_intrinsic_value_caches_fields_without_eager_evaluation() {
        let source = "h(\"input\",{id:identity,value:()=>row.label,onInput:listener})";
        for (target, authored, expected) in [
            (MoonshineModuleTarget::Dom, true, true),
            (MoonshineModuleTarget::Dom, false, false),
            (MoonshineModuleTarget::Ssr, true, false),
        ] {
            let output = component_getters(
                source,
                (SourceType::tsx(), target),
                "h",
                "Fragment",
                &[],
                "children",
                ModuleHints {
                    svelte_component_props: authored,
                    ..ModuleHints::default()
                },
            )
            .unwrap();
            assert_eq!(
                output.contains("value: __crepusTextareaMemo(() => row.label)"),
                expected,
                "{output}"
            );
            assert!(output.contains("id: identity"));
            assert!(output.contains("onInput: listener"));
            if expected {
                assert!(output.contains("if (!memo)"));
                assert!(output.contains("runWithOwner as"));
            }
        }
        let literal = component_getters(
            "h(\"input\",{value:()=> (()=>named())})",
            (SourceType::tsx(), MoonshineModuleTarget::Dom),
            "h",
            "Fragment",
            &[],
            "children",
            ModuleHints {
                svelte_component_props: true,
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(
            literal.contains("value: __crepusTextareaMemo(() => (() => named()))"),
            "{literal}"
        );
    }

    #[test]
    fn generated_textarea_children_keep_the_same_once_boundary_as_intrinsic_siblings() {
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            "const view=<form>{reset(<textarea/>,()=>value)}</form>",
            SourceType::tsx(),
        )
        .parse();
        assert!(parsed.errors.is_empty());
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let expression = semantic
            .nodes()
            .iter()
            .find_map(|node| match node.kind() {
                oxc_ast::AstKind::JSXExpressionContainer(value) => Some(&value.expression),
                _ => None,
            })
            .unwrap();
        for helper in [None, Some("reset")] {
            let mut inspect = Inspect {
                textarea_default_helper: helper,
                ..Inspect::default()
            };
            inspect.mark_jsx_child(expression);
            assert_eq!(inspect.once_spans.len(), usize::from(helper.is_some()));
            inspect.thunk(expression);
            assert_eq!(inspect.edits.len(), if helper.is_some() { 0 } else { 2 });
        }
    }

    #[test]
    fn authored_dom_textarea_text_accessor_becomes_cached_live_value() {
        for source in [
            "h(\"textarea\",null,h(Text,{parts:[()=>value]}))",
            "h(\"textarea\",{},h(Text,{parts:[()=>value]}))",
            "h(\"textarea\",{id:identity,defaultValue:reset, /* tail */}, /* gap */h(Text,{parts:[()=>value]}))",
            "h(\"textarea\",{id:identity,defaultValue:reset, // tail\n}, // gap\nh(Text,{parts:[()=>value]}))",
        ] {
            let out = component_getters(
                source,
                (SourceType::tsx(), MoonshineModuleTarget::Dom),
                "h",
                "Fragment",
                &[],
                "children",
                ModuleHints {
                    svelte_component_props: true,
                    text_component: Some("Text"),
                    ..ModuleHints::default()
                },
            )
            .unwrap();
            assert!(
                out.contains("createRenderMemo as __crepusTextareaRenderMemo"),
                "{out}"
            );
            assert!(
                out.contains("const owner = __crepusTextareaGetOwner()"),
                "{out}"
            );
            assert!(out.contains("memo = __crepusTextareaRunWithOwner(owner, () => __crepusTextareaRenderMemo(read))"), "{out}");
            assert!(
                out.contains("value: __crepusTextareaMemo(h(Text, { parts: [() => value] }))"),
                "{out}"
            );
            assert_eq!(out.matches("h(Text").count(), 1);
            if source.contains("defaultValue") {
                assert!(out.contains("defaultValue: reset"));
                // Final Codegen drops comments; successful parsing checks their
                // trivia did not swallow the inserted value punctuation.
                assert!(out.contains("id: identity"));
            }
        }
    }

    #[test]
    fn textarea_namespace_metadata_and_live_value_edits_remain_disjoint() {
        for props in [
            r#"{Witness:"http://www.w3.org/1999/xhtml"}"#,
            r#"{Witness:"http://www.w3.org/1999/xhtml",id:identity,defaultValue:reset}"#,
            r#"{id:identity,defaultValue:reset,Witness:"http://www.w3.org/1999/xhtml"}"#,
            r#"{/* before */ Witness:"http://www.w3.org/1999/xhtml" /* after */}"#,
            "{Witness:\"http://www.w3.org/1999/xhtml\", // marker\nid:identity, /* real */ defaultValue:reset, // tail\n}",
            "{id:identity,defaultValue:reset, /* before */ Witness:\"http://www.w3.org/1999/xhtml\" // tail\n}",
            r#"{Witness:"http://www.w3.org/1999/xhtml",value:existing}"#,
            r#"{value:existing,Witness:"http://www.w3.org/1999/xhtml"}"#,
        ] {
            let source =
                format!("h(\"textarea\",{props}, /* gap */ h(Text,{{parts:[()=>value]}}))");
            for source_type in [SourceType::tsx(), SourceType::jsx()] {
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let output = component_getters(
                        &source,
                        (source_type, target),
                        "h",
                        "Fragment",
                        &[],
                        "children",
                        ModuleHints {
                            intrinsic_namespace: Some("Witness"),
                            svelte_component_props: true,
                            text_component: Some("Text"),
                            ..ModuleHints::default()
                        },
                    )
                    .unwrap();
                    assert!(!output.contains("Witness"), "{output}");
                    let dom = matches!(target, MoonshineModuleTarget::Dom);
                    assert_eq!(output.contains("hNS as"), dom, "{output}");
                    assert_eq!(
                        output.contains("value: __crepusTextareaMemo("),
                        dom && !props.contains("value:existing"),
                        "{output}"
                    );
                    if props.contains("id:identity") {
                        assert!(output.contains("id: identity"), "{output}");
                        assert!(output.contains("defaultValue: reset"), "{output}");
                    }
                    if props.contains("value:existing") {
                        assert!(output.contains("value: existing"), "{output}");
                    }
                    let allocator = Allocator::default();
                    assert!(
                        Parser::new(&allocator, &output, source_type)
                            .parse()
                            .errors
                            .is_empty(),
                        "{output}"
                    );
                }
            }
        }
    }

    #[test]
    fn textarea_namespace_live_value_full_module_parses_for_js_and_ts_targets() {
        for attributes in [
            r#"Witness="http://www.w3.org/1999/xhtml""#,
            r#"Witness="http://www.w3.org/1999/xhtml" id={identity}"#,
            r#"id={identity} Witness="http://www.w3.org/1999/xhtml""#,
            r#"Witness="http://www.w3.org/1999/xhtml" value={existing}"#,
        ] {
            let source = format!(
                "const value='text',identity='id',existing='kept';function Text(){{return 'child'}}export default function View(){{return <textarea {attributes}><Text parts={{[()=>value]}}/></textarea>}}"
            );
            for extension in ["jsx", "tsx"] {
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let output = compile_moonshine_svelte_module(
                        &source,
                        &Path::new("Textarea").with_extension(extension),
                        target,
                        ModuleHints {
                            intrinsic_namespace: Some("Witness"),
                            svelte_component_props: true,
                            text_component: Some("Text"),
                            ..ModuleHints::default()
                        },
                    )
                    .unwrap()
                    .source;
                    assert!(!output.contains("Witness"), "{output}");
                    let allocator = Allocator::default();
                    let source_type = if extension == "jsx" {
                        SourceType::jsx()
                    } else {
                        SourceType::tsx()
                    };
                    assert!(
                        Parser::new(&allocator, &output, source_type)
                            .parse()
                            .errors
                            .is_empty(),
                        "{output}"
                    );
                }
            }
        }
    }

    #[test]
    fn textarea_live_lowering_is_target_metadata_and_dynamic_child_specific() {
        for (source, target, authored) in [
            (
                "h(\"textarea\",null,h(Text,{parts:[()=>value]}))",
                MoonshineModuleTarget::Ssr,
                true,
            ),
            (
                "h(\"textarea\",null,h(Text,{parts:[()=>value]}))",
                MoonshineModuleTarget::Dom,
                false,
            ),
            (
                "h(\"textarea\",null,\"literal\")",
                MoonshineModuleTarget::Dom,
                true,
            ),
            (
                "h(\"textarea\",{value:existing},h(Text,{parts:[()=>value]}))",
                MoonshineModuleTarget::Dom,
                true,
            ),
            (
                "h(\"div\",null,h(Text,{parts:[()=>value]}))",
                MoonshineModuleTarget::Dom,
                true,
            ),
        ] {
            let out = component_getters(
                source,
                (SourceType::tsx(), target),
                "h",
                "Fragment",
                &[],
                "children",
                ModuleHints {
                    svelte_component_props: authored,
                    text_component: Some("Text"),
                    ..ModuleHints::default()
                },
            )
            .unwrap();
            assert!(
                !out.contains("createRenderMemo as __crepusTextareaRenderMemo"),
                "{out}"
            );
        }
        let out = component_getters(
            "const __crepusTextareaMemo=0;h(\"textarea\",null,h(Text,{parts:[()=>value]}))",
            (SourceType::tsx(), MoonshineModuleTarget::Dom),
            "h",
            "Fragment",
            &[],
            "children",
            ModuleHints {
                svelte_component_props: true,
                text_component: Some("Text"),
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(out.contains("function __crepusTextareaMemo_("));
    }

    #[test]
    fn textarea_lazy_memo_is_hygienic_and_plain_js_compatible() {
        let source = "const __crepusTextareaMemo=0,__crepusTextareaRenderMemo=0,__crepusTextareaGetOwner=0,__crepusTextareaRunWithOwner=0;h(\"textarea\",null,h(Text,{parts:[()=>value]}))";
        let out = component_getters(
            source,
            (SourceType::jsx(), MoonshineModuleTarget::Dom),
            "h",
            "Fragment",
            &[],
            "children",
            ModuleHints {
                svelte_component_props: true,
                text_component: Some("Text"),
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(
            out.contains("createRenderMemo as __crepusTextareaRenderMemo_"),
            "{out}"
        );
        assert!(
            out.contains("getOwner as __crepusTextareaGetOwner_"),
            "{out}"
        );
        assert!(
            out.contains("runWithOwner as __crepusTextareaRunWithOwner_"),
            "{out}"
        );
        assert!(
            out.contains("function __crepusTextareaMemo_(read)"),
            "{out}"
        );
        let owner = out
            .find("const owner = __crepusTextareaGetOwner_()")
            .unwrap();
        let accessor = out[owner..].find("return () =>").unwrap() + owner;
        let initialize = out.find("if (!memo)").unwrap();
        assert!(owner < accessor && accessor < initialize, "{out}");
    }

    #[test]
    fn inline_snippet_scope_requires_explicit_metadata_and_direct_declaration() {
        let names = vec!["generatedSnippet".to_string()];
        for (source, expected) in [
            ("(()=>{function generatedSnippet(){};return value})()", true),
            ("(()=>{function unrelated(){};return value})()", false),
            (
                "(()=>{function outer(){function generatedSnippet(){}};return value})()",
                false,
            ),
            (
                "(()=>{function generatedSnippet(){};return value})(argument)",
                false,
            ),
        ] {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty());
            let Statement::ExpressionStatement(statement) = &parsed.program.body[0] else {
                panic!("expected expression");
            };
            let inspect = Inspect {
                inline_snippet_names: &names,
                ..Inspect::default()
            };
            assert_eq!(
                inspect.inline_snippet_scope(&statement.expression),
                expected,
                "{source}"
            );
            assert!(!Inspect::default().inline_snippet_scope(&statement.expression));
        }
    }

    #[test]
    fn authored_component_props_allow_reserved_intrinsic_names_only_with_hint() {
        let source = "export default function C(){return <Child ref={value} innerHTML={value} outerHTML={value} dangerouslySetInnerHTML={value} __proto__={value}/>};";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled = compile_moonshine_svelte_module(
                source,
                Path::new("props.tsx"),
                target,
                ModuleHints {
                    svelte_component_props: true,
                    ..ModuleHints::default()
                },
            )
            .unwrap();
            assert!(
                compiled.source.contains("[\"__proto__\"]:"),
                "{}",
                compiled.source
            );
            assert!(compiled.source.contains("ref:"));
            assert!(compiled.source.contains("innerHTML:"));
            assert!(compile_moonshine_module(source, Path::new("props.tsx"), target).is_err());
            assert!(compile_moonshine_svelte_module(
                "export default function C(){return <div ref={value}/>}",
                Path::new("props.tsx"),
                target,
                ModuleHints {
                    svelte_component_props: true,
                    ..ModuleHints::default()
                }
            )
            .is_err());
        }
    }
    #[test]
    fn prototype_keys_keep_cached_getters_and_symbol_key_mapping() {
        let source = "h(Child,{\"__proto__\":cached(()=>value),marker:binding})";
        let output = component_getters(
            source,
            (SourceType::tsx(), MoonshineModuleTarget::Dom),
            "h",
            "Fragment",
            &["cached"],
            "children",
            ModuleHints {
                svelte_component_props: true,
                property_keys: &[("marker", "referenceSymbol")],
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(output.contains("[\"__proto__\"]:"), "{output}");
        assert!(output.contains("get: cached("), "{output}");
        assert!(output.contains("[referenceSymbol]:"), "{output}");
        assert!(!output.contains("get: () => cached"));
    }
    #[test]
    fn vue_property_keys_rewrite_intrinsic_and_typed_component_markers() {
        let source = "import Child from \"./child.vue\";\nexport default function C(){return <div><input elMarker={cb}/><Child compMarker={binding} label={text}/></div>}";
        let compiled = compile_moonshine_module_with_hints(
            source,
            Path::new("refs.tsx"),
            MoonshineModuleTarget::Dom,
            ModuleHints {
                property_keys: &[("elMarker", "\"ref\""), ("compMarker", "referenceSymbol")],
                ..ModuleHints::default()
            },
        )
        .unwrap();
        assert!(compiled.source.contains("[\"ref\"]: cb"), "{}", compiled.source);
        assert!(
            compiled.source.contains("[referenceSymbol]: {"),
            "{}",
            compiled.source
        );
        assert!(
            compiled
                .source
                .contains("as K extends symbol ? never : K"),
            "{}",
            compiled.source
        );
    }
}

#[cfg(test)]
mod svelte_ssr_choice_tests {
    use super::*;

    fn compile(source: &str, target: MoonshineModuleTarget, authored: bool) -> String {
        let output = compile_module(
            source,
            Path::new("choice.tsx"),
            target,
            authored,
            ModuleHints::default(),
        )
        .unwrap()
        .source;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}\n{:?}", parsed.errors);
        output
    }

    #[test]
    fn authored_server_choices_use_attached_boolean_selection() {
        let output = compile(
            "export default function C(){return <main>{enabled ? <div/> : <span/>}</main>}",
            MoonshineModuleTarget::Ssr,
            true,
        );
        assert!(output.contains("createMemo as __crepusMemo"), "{output}");
        assert!(output.contains("const active = __crepusMemo0("), "{output}");
        assert!(
            output.contains("nullish ? raw() != null : !!raw()"),
            "{output}"
        );
        assert!(output.contains("active() ? yes(raw) : no(raw)"), "{output}");
        assert!(output.contains("const branch = read()();"), "{output}");
        assert!(!output.contains("Show as"), "{output}");
        assert!(!output.contains("@tschk/moonshine-dom/control"), "{output}");
    }

    #[test]
    fn generic_server_and_all_dom_choices_keep_show() {
        for (target, authored) in [
            (MoonshineModuleTarget::Ssr, false),
            (MoonshineModuleTarget::Dom, false),
            (MoonshineModuleTarget::Dom, true),
        ] {
            let output = compile(
                "export default function C(){return <main>{enabled ? <div/> : <span/>}</main>}",
                target,
                authored,
            );
            assert!(output.contains("Show as __crepusShow"), "{output}");
            let memo = if authored && matches!(target, MoonshineModuleTarget::Dom) {
                "createRenderMemo"
            } else {
                "createMemo"
            };
            assert!(
                output.contains(&format!("{memo} as __crepusMemo")),
                "{output}"
            );
            assert!(
                output.contains("when: () => nullish ? raw() != null : raw()"),
                "{output}"
            );
            assert!(!output.contains("const active ="), "{output}");
        }
    }

    #[test]
    fn authored_server_logical_choices_preserve_nullish_boundary() {
        for (operator, nullish) in [("&&", false), ("||", false), ("??", true)] {
            let source = format!(
                "export default function C(){{return <main>{{value {operator} <div/>}}</main>}}"
            );
            let output = compile(&source, MoonshineModuleTarget::Ssr, true);
            assert!(
                output.contains("nullish ? raw() != null : !!raw()"),
                "{output}"
            );
            assert!(output.contains("active() ? yes(raw) : no(raw)"), "{output}");
            assert_eq!(output.contains(", true)"), nullish, "{output}");
            assert!(!output.contains("Show as"), "{output}");
        }
    }
}

#[cfg(test)]
mod react_boundary_tests {
    use super::*;

    #[test]
    fn native_targets_reject_react_runtime_imports_and_reexports() {
        for source in [
            "import {useState} from 'react';export default()=> <div/>;",
            "import {type ReactNode as Node}from'react';export type Props=Node;",
            "export{type ReactNode as Node}from'react';",
            "import React from 'react';export default()=> <div/>;",
            "import * as React from 'react';export const value=React;",
            "import 'react';export const value=1;",
            "import {} from 'react';export const value=1;",
            "import {type ReactNode,useState as state}from'react';export const value=state;",
            "import{jsx}from'react/jsx-runtime';export const value=jsx;",
            "import{createRoot}from'react-dom/client';export const value=createRoot;",
            "export{useState as state}from'react';",
            "export{type ReactNode,useState}from'react';",
            "export{}from'react-dom';",
            "export*from'react';",
            "export*as React from'react';",
            "export const load=()=>import('react');",
            "export const load=()=>import('react-dom/server');",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error =
                    compile_moonshine_module(source, Path::new("ReactBoundary.tsx"), target)
                        .unwrap_err();
                assert!(
                    error.contains("React renderer or hooks"),
                    "{source}: {error}"
                );
                assert!(error.contains("adapter=\"react\""), "{source}: {error}");
                assert!(
                    error.contains("Use import type/export type"),
                    "{source}: {error}"
                );
            }
        }
    }

    #[test]
    fn react_types_and_unrelated_packages_remain_external_without_runtime_admission() {
        for source in [
            "import type React from'react';export type Props=React.ReactNode;",
            "import type{ReactNode as Node}from'react';export type Props=Node;",
            "import type * as React from'react';export type Props=React.ReactNode;",
            "export type{ReactNode as Node}from'react';",
            "export type*from'react';",
            "export type*as React from'react-dom';",
            "export type Node=import('react').ReactNode;export type Module=typeof import('react-dom/client');",
            "import{value}from'reactivity';export{value};",
            "import{value}from'react-domish';export{value};",
            "export{value}from'@vendor/react';",
            "export const load=()=>import('reactive-package');",
            "function useState(value:number){return value}export default()=> <div>{useState(1)}</div>;",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                compile_moonshine_module(source, Path::new("ReactTypes.tsx"), target)
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod transparent_ssr_tests {
    use super::*;

    #[test]
    fn known_helpers_are_transparent_but_lookalikes_and_shadowed_bindings_are_not() {
        let source = "import {h} from 'runtime';function Each(props){return props.children()}function EachLookalike(props){return props.children()}const a=h(/* keep */Each,null);const b=h(EachLookalike,{});function shadow(Each){return h(Each,{})}function shadowHost(h){return h(Each,{})}";
        let output =
            transparent_ssr_helpers(source, SourceType::tsx(), "h", &["Each".into()]).unwrap();
        assert!(output.contains("/* keep */\nEach({})"), "{output}");
        assert!(output.contains("h(EachLookalike,{})"));
        assert!(output.contains("function shadow(Each){return h(Each,{})}"));
        assert!(output.contains("function shadowHost(h){return h(Each,{})}"));
    }

    #[test]
    fn nullish_props_match_host_shape_and_expression_is_evaluated_once() {
        let source = "import {h} from 'runtime';function Each(props){return props}const a=h(Each,/* null props */null);const b=h(Each,readProps());const c=h(Each,{value:1});";
        let output =
            transparent_ssr_helpers(source, SourceType::tsx(), "h", &["Each".into()]).unwrap();
        assert!(output.contains("/* null props */\nEach({})"), "{output}");
        assert!(output.contains("Each((readProps() ?? {}))"), "{output}");
        assert_eq!(output.matches("readProps()").count(), 1);
        assert!(output.contains("Each({value:1})"), "{output}");
    }

    #[test]
    fn malformed_normalized_helper_calls_fail_closed() {
        for call in [
            "h(Each)",
            "h(Each,{},child)",
            "h?.(Each,{})",
            "h(Each,...props)",
        ] {
            let source = format!(
                "import {{h}} from 'runtime';function Each(props){{return props.children()}}{call}"
            );
            assert!(
                transparent_ssr_helpers(&source, SourceType::tsx(), "h", &["Each".into()])
                    .unwrap_err()
                    .contains("invalid normalized SSR helper call")
            );
        }
    }

    #[test]
    fn both_languages_preserve_implicit_children_props_and_real_components() {
        let helpers = ["Each".to_string(), "Text".to_string()];
        let source = "import {getContext,setContext} from '@tschk/moonshine-dom/component';function Each(props){return props.children()}function Text(props){return props.parts.map(fn=>fn()).join('')}function Child(props){return <i>{getContext('key')}</i>}export default function App(){setContext('key','initial');return <Each unused={1}><Text parts={[()=>{setContext('key','changed');return getContext('key')} ]}/><Child/></Each>}";
        for policy in [
            crate::svelte_javascript::IrPolicy::Typed,
            crate::svelte_javascript::IrPolicy::JavaScript,
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let compiled = compile_moonshine_svelte_module(
                    source,
                    Path::new("Proof.tsx"),
                    target,
                    ModuleHints {
                        svelte_ir_policy: policy,
                        svelte_component_props: true,
                        text_component: Some("Text"),
                        ssr_transparent_helpers: &helpers,
                        ..ModuleHints::default()
                    },
                )
                .unwrap();
                let output = &compiled.source;
                assert!(output.contains("children:"), "{output}");
                assert!(output.contains("enumerable: true"), "{output}");
                assert!(output.contains("(Child,"), "{output}");
                if matches!(target, MoonshineModuleTarget::Ssr) {
                    assert!(!output.contains("(Each,"), "{output}");
                    assert!(!output.contains("(Text,"), "{output}");
                    assert!(
                        output.contains("Each((globalThis.Object.create"),
                        "{output}"
                    );
                } else {
                    assert!(output.contains("(Each,"), "{output}");
                }
            }
        }
    }
}

#[cfg(test)]
mod head_intrinsic_tests {
    #[test]
    fn document_head_intrinsics_are_supported() {
        for name in ["meta", "link", "title"] {
            assert!(super::supported_html_tag(name));
        }
    }
}

#[cfg(test)]
mod dynamic_intrinsic_tests {
    use super::*;
    #[test]
    fn mathml_witness_admits_intrinsics_without_widening_html_validation() {
        for tag in ["math", "mi", "annotation-xml", "mglyph", "custommath"] {
            let source=format!("export default()=> <{tag} Witness=\"http://www.w3.org/1998/Math/MathML\">x</{tag}>;");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = compile_moonshine_svelte_module(
                    &source,
                    Path::new("Math.tsx"),
                    target,
                    ModuleHints {
                        svelte_component_props: true,
                        intrinsic_namespace: Some("Witness"),
                        ..ModuleHints::default()
                    },
                )
                .unwrap()
                .source;
                assert!(!output.contains("Witness:"), "{output}");
                assert_eq!(
                    output.contains("hNS as"),
                    matches!(target, MoonshineModuleTarget::Dom)
                );
            }
        }
        assert!(compile_moonshine_module(
            "export default()=> <mi/>;",
            Path::new("Plain.tsx"),
            MoonshineModuleTarget::Dom
        )
        .is_err());
    }

    #[test]
    fn private_namespace_override_is_consumed_only_at_dom_creation() {
        for attributes in [
            r#"DynNs={ns} DynTag={tag} Witness="http://www.w3.org/1999/xhtml""#,
            r#"Witness="http://www.w3.org/1999/xhtml" xmlns={ns} DynTag={tag} DynNs={ns}"#,
        ] {
            let source=format!("const tag='div';const ns:unknown=undefined;export default()=> <div {attributes}/>;");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = compile_moonshine_svelte_module(
                    &source,
                    Path::new("Namespace.tsx"),
                    target,
                    ModuleHints {
                        svelte_component_props: true,
                        intrinsic_namespace: Some("Witness"),
                        dynamic_tag_attribute: Some("DynTag"),
                        dynamic_namespace_attribute: Some("DynNs"),
                        ..ModuleHints::default()
                    },
                )
                .unwrap()
                .source;
                assert!(!output.contains("DynNs:"), "{output}");
                assert!(!output.contains("DynTag:"), "{output}");
                assert!(!output.contains("Witness:"), "{output}");
                assert_eq!(
                    output.contains("hSourceNS as"),
                    matches!(target, MoonshineModuleTarget::Dom)
                );
                assert!(!output.contains("`${ns}`"), "{output}");
                let allocator = Allocator::default();
                assert!(
                    Parser::new(&allocator, &output, SourceType::tsx())
                        .parse()
                        .errors
                        .is_empty(),
                    "{output}"
                );
            }
        }
    }

    #[test]
    fn private_dynamic_tag_reuses_intrinsic_metadata_without_leaking_attributes() {
        for attributes in [
            r#"DynTag={tag} Witness="http://www.w3.org/1999/xhtml""#,
            r#"Witness="http://www.w3.org/1999/xhtml" DynTag={tag}"#,
            r#"title="kept" DynTag={tag} Witness="http://www.w3.org/1999/xhtml""#,
            r#"DynTag={tag} title="kept" Witness="http://www.w3.org/1999/xhtml""#,
        ] {
            let source = format!(
                "const tag='span';export default function C(){{return <div {attributes}/>}}"
            );
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = compile_moonshine_svelte_module(
                    &source,
                    Path::new("Dynamic.tsx"),
                    target,
                    ModuleHints {
                        svelte_component_props: true,
                        intrinsic_namespace: Some("Witness"),
                        dynamic_tag_attribute: Some("DynTag"),
                        ..ModuleHints::default()
                    },
                )
                .unwrap()
                .source;
                assert!(!output.contains("DynTag:"), "{output}");
                assert!(!output.contains("Witness:"), "{output}");
                let allocator = Allocator::default();
                assert!(
                    Parser::new(&allocator, &output, SourceType::tsx())
                        .parse()
                        .errors
                        .is_empty(),
                    "{output}"
                );
                assert!(
                    output.contains(if matches!(target, MoonshineModuleTarget::Dom) {
                        "hNS as"
                    } else {
                        "hSource as"
                    }),
                    "{output}"
                );
            }
        }
    }
}
