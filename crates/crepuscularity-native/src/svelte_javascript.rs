//! Source-language policy and final JavaScript delivery for pure-JS SFCs.
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_transformer::{TransformOptions, Transformer};
use std::path::Path;

/// Typed compiler IR may contain annotations even when its authored blocks are JavaScript.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum IrPolicy {
    #[default]
    Typed,
    JavaScript,
}
impl IrPolicy {
    pub(crate) fn configure(self, program: &mut Program<'_>) {
        if self == Self::JavaScript {
            // Original blocks were validated independently. Retain JS class semantics,
            // without disabling lexical/duplicate/private-name validation of the IR.
            program.source_type = program.source_type.with_javascript(true);
        }
    }
}

pub(crate) fn validate(source: &str, path: &Path) -> Result<(), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: invalid authored JavaScript Svelte script",
            path.display()
        ));
    }
    let checked = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !checked.errors.is_empty() {
        return Err(format!(
            "{}: invalid authored JavaScript Svelte scope",
            path.display()
        ));
    }
    validate_docs(source, &parsed.program)?;
    Ok(())
}

pub(crate) fn validate_docs(source: &str, program: &Program<'_>) -> Result<(), String> {
    for comment in &program.comments {
        if comment.is_jsdoc() {
            let text =
                &source[comment.content_span().start as usize..comment.content_span().end as usize];
            // This is a conservative import-expression guard, not a JSDoc grammar.
            // Opaque prose containing the word "import" has no type dependency.
            if text.match_indices("import").any(|(index, _)| {
                let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
                let before = text[..index].chars().next_back();
                let after = &text[index + "import".len()..];
                if before.is_some_and(word) || after.chars().next().is_some_and(word) {
                    return false;
                }
                let next = after.chars().find(|c| !c.is_whitespace() && *c != '*');
                next == Some('(') || before == Some('@')
            }) {
                return Err(
                    "unsupported pure-JS Svelte JSDoc import types; use an inline typedef".into(),
                );
            }
        }
    }
    Ok(())
}

/// Emit type-only documentation after the last compiler code-generation pass.
pub(crate) fn finish(
    source: &str,
    path: &Path,
    component: &str,
    props: &str,
    exports: &str,
    runtime: &str,
    fresh: impl Fn(&str) -> String,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid pure-JS Svelte delivery IR".into());
    }
    IrPolicy::JavaScript.configure(&mut parsed.program);
    struct Scope<'s> {
        props: &'s str,
        returns: Vec<Span>,
    }
    impl<'a> Visit<'a> for Scope<'_> {
        fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
            for statement in &body.statements {
                if let Statement::TSTypeAliasDeclaration(alias) = statement {
                    if alias.id.name == self.props {
                        // The type expression is compiler-owned or an opaque authored annotation.
                        for statement in &body.statements {
                            if let Statement::ReturnStatement(ret) = statement {
                                if ret.argument.is_some() {
                                    self.returns.push(ret.span);
                                }
                            }
                        }
                    }
                }
            }
            walk::walk_function_body(self, body);
        }
    }
    let mut scope = Scope {
        props,
        returns: vec![],
    };
    scope.visit_program(&parsed.program);
    if scope.returns.len() != 1 {
        return Err("pure-JS Svelte metadata requires one lexical return".into());
    }
    let mut props_type = None;
    struct Alias<'s> {
        name: &'s str,
        span: Option<Span>,
    }
    impl<'a> Visit<'a> for Alias<'_> {
        fn visit_ts_type_alias_declaration(&mut self, node: &TSTypeAliasDeclaration<'a>) {
            if node.id.name == self.name {
                self.span = Some(node.type_annotation.span());
            }
            walk::walk_ts_type_alias_declaration(self, node);
        }
    }
    let mut alias = Alias {
        name: props,
        span: None,
    };
    alias.visit_program(&parsed.program);
    if let Some(span) = alias.span {
        props_type = Some(source[span.start as usize..span.end as usize].to_owned());
    }
    let props_type = props_type.ok_or("missing pure-JS Svelte public props type")?;
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
        .ok_or("pure-JS Svelte delivery requires the named default component")?;
    let implementation = fresh("__crepusSvelteImplementation");
    let view = fresh("__crepusSvelteView");
    let ret = scope.returns[0];
    let expression = source[ret.start as usize + "return".len()..ret.end as usize]
        .trim_end()
        .trim_end_matches(';');
    let mut edits = vec![
        (
            ret.start as usize,
            ret.end as usize,
            format!("const {view}={expression};return {view};"),
        ),
        (
            function.0.span.start as usize,
            function.1.span.start as usize,
            String::new(),
        ),
        (
            function.2.span.start as usize,
            function.2.span.end as usize,
            implementation.clone(),
        ),
    ];
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut typed = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        typed.replace_range(start..end, &text);
    }
    let mut parsed = Parser::new(&allocator, &typed, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid renamed pure-JS Svelte IR".into());
    }
    IrPolicy::JavaScript.configure(&mut parsed.program);
    let checked = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !checked.errors.is_empty() {
        return Err("invalid pure-JS Svelte IR scope".into());
    }
    let scoping = checked.semantic.into_scoping();
    parsed.program.source_type = SourceType::tsx();
    let mut options = TransformOptions::default();
    // Authored JS value imports retain their module side effects even when only documented.
    options.typescript.only_remove_type_imports = true;
    let transformed = Transformer::new(&allocator, path, &options)
        .build_with_scoping(scoping, &mut parsed.program);
    if !transformed.errors.is_empty() {
        return Err("pure-JS Svelte type erasure failed".into());
    }
    let erased = Codegen::new().build(&parsed.program).code;
    let parsed = Parser::new(&allocator, &erased, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid erased Svelte JavaScript".into());
    }
    let checked = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !checked.errors.is_empty() {
        return Err("invalid erased Svelte JavaScript scope".into());
    }
    // The view identifier was hygienically allocated, so a return of that exact
    // identifier is the compiler-owned return, including a scheduler callback.
    struct Returns<'s> {
        name: &'s str,
        spans: Vec<Span>,
    }
    impl<'a> Visit<'a> for Returns<'_> {
        fn visit_return_statement(&mut self, node: &ReturnStatement<'a>) {
            if matches!(&node.argument,Some(Expression::Identifier(id))if id.name==self.name) {
                self.spans.push(node.span);
            }
            walk::walk_return_statement(self, node);
        }
    }
    let mut returns = Returns {
        name: &view,
        spans: vec![],
    };
    returns.visit_program(&parsed.program);
    if returns.spans.len() != 1 {
        return Err("erased Svelte JavaScript lost its view return".into());
    }
    let ret = returns.spans[0];
    let mut output = erased;
    output.replace_range(ret.start as usize..ret.end as usize,&format!("/** @typedef {{{props_type}}} {props} */\nreturn /** @type {{import({runtime:?}).ComponentView<typeof {view},{exports},{props}>}} */({view});"));
    output.push_str(&format!("\n/** @this {{unknown}}\n * @param {{import(\"@tschk/moonshine-dom/platform\").GlobalTypes.Parameters<import({runtime:?}).ComponentFactory<typeof {implementation}>>}} args\n * @returns {{import(\"@tschk/moonshine-dom/platform\").GlobalTypes.ReturnType<typeof {implementation}>}}\n */\nexport default function {component}(...args){{return {implementation}.apply(this,args);}}\n"));
    let parsed = Parser::new(&allocator, &output, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid documented Svelte JavaScript".into());
    }
    let checked = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !checked.errors.is_empty() {
        return Err("invalid documented Svelte JavaScript scope".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_javascript_keeps_public_overrides_and_rejects_private_duplicates() {
        assert!(validate(
            "class C {get x(){return 1}set x(v){}['x'](){return 9}}",
            Path::new("App.svelte")
        )
        .is_ok());
        assert!(validate("class C {#x;#x}", Path::new("App.svelte")).is_err());
        assert!(validate("const value:number=1", Path::new("App.svelte")).is_err());
    }
    #[test]
    fn jsdoc_imports_fail_closed_without_erasing_the_comment() {
        assert!(validate(
            "/** @typedef {import('svelte').Snippet} S */const n=1;",
            Path::new("App.svelte")
        )
        .unwrap_err()
        .contains("unsupported pure-JS Svelte JSDoc import types"));
        assert!(validate(
            "/** @typedef {{label:string}} Props */const n=1;",
            Path::new("App.svelte")
        )
        .is_ok());
    }
    #[test]
    fn ordinary_jsdoc_import_prose_is_not_a_type_dependency() {
        assert!(validate(
            "/** we import values; import choices are documented here */const n=1",
            Path::new("App.svelte")
        )
        .is_ok());
        assert!(validate(
            "/** @import {Snippet} from 'svelte' */const n=1",
            Path::new("App.svelte")
        )
        .is_err());
    }
}
