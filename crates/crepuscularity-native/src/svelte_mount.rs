//! Preserve lexical SFC types while retaining a hoisted default component.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

pub(crate) fn finish(
    source: &str,
    component: &str,
    props: &str,
    exports: &str,
    target: MoonshineModuleTarget,
    fresh: impl Fn(&str) -> String,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<(String, &'static str), String> {
    let types = &globals.types;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid compiled Svelte mount metadata syntax".into());
    }
    struct Returns<'s> {
        props: &'s str,
        spans: Vec<Span>,
    }
    impl<'a> Visit<'a> for Returns<'_> {
        fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
            if body.statements.iter().any(|statement| {
                matches!(statement, Statement::TSTypeAliasDeclaration(alias) if alias.id.name == self.props)
            }) {
                for statement in &body.statements {
                    if let Statement::ReturnStatement(ret) = statement {
                        if ret.argument.is_some() {
                            self.spans.push(ret.span);
                        }
                    }
                }
            }
            walk::walk_function_body(self, body);
        }
    }
    let mut returns = Returns {
        props,
        spans: vec![],
    };
    returns.visit_program(&parsed.program);
    if returns.spans.len() != 1 {
        return Err("Svelte mount metadata requires one lexical component return".into());
    }
    let implementation = fresh("__crepusSvelteImplementation");
    let view = fresh("__crepusSvelteView");
    let view_type = fresh("__crepusSvelteViewType");
    let factory_type = fresh("__crepusSvelteFactoryType");
    let span = returns.spans[0];
    let expression = source[span.start as usize + "return".len()..span.end as usize]
        .trim_end()
        .trim_end_matches(';');
    let mut edits = vec![(
        span.start as usize,
        span.end as usize,
        format!(
        "const {view}={expression};return {view} as {view_type}<typeof {view},{exports},{props}>;"
    ),
    )];
    let function = parsed
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
            let id = function.id.as_ref()?;
            (id.name == component).then_some((export, function, id))
        })
        .ok_or("Svelte mount metadata requires the named default component")?;
    edits.push((
        function.0.span.start as usize,
        function.1.span.start as usize,
        String::new(),
    ));
    edits.push((
        function.2.span.start as usize,
        function.2.span.end as usize,
        implementation.clone(),
    ));
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    let runtime = match target {
        MoonshineModuleTarget::Dom => "@tschk/moonshine-svelte/mount",
        MoonshineModuleTarget::Ssr => "@tschk/moonshine-svelte/server-mount",
    };
    // A cast export expression would replace the original ESM hoisted binding with a TDZ.
    output.push_str(&format!("\nexport default function {component}(this:unknown,...args:{types}.Parameters<{factory_type}<typeof {implementation}>>):{types}.ReturnType<typeof {implementation}>{{return {implementation}.apply(this,args);}}\n"));
    Ok((format!("import type {{ComponentView as {view_type},ComponentFactory as {factory_type}}} from {runtime:?};\n{output}"), runtime))
}
