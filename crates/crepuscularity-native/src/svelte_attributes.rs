//! Lower generated mixed-attribute conversions before component prop analysis.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::Expression, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

pub(crate) fn transform(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte attribute syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let mut edits = Vec::new();
    for node in built.semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !matches!(&call.callee, Expression::Identifier(name) if name.name == marker) {
            continue;
        }
        let [argument] = call.arguments.as_slice() else {
            return Err("invalid generated Svelte attribute conversion".into());
        };
        let Some(argument) = argument.as_expression() else {
            return Err("invalid generated Svelte attribute conversion".into());
        };
        let span = argument.span();
        let value = &source[span.start as usize..span.end as usize];
        // Indexed reads avoid TypeScript's redundant-?? diagnostic without
        // duplicating evaluation or introducing a prop factory call.
        let value = format!("([({value}\n)][0] ?? \"\")");
        let replacement = match target {
            MoonshineModuleTarget::Dom => format!("`${{{value}}}`"),
            MoonshineModuleTarget::Ssr => value,
        };
        edits.push((
            call.span.start as usize,
            call.span.end as usize,
            replacement,
        ));
    }
    let mut output = source.to_owned();
    edits.sort_by_key(|edit| edit.0);
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok(output)
}

pub(crate) fn class_directives(source: &str, marker: &str, helper: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte class directive syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::Identifier(name) = &call.callee else {
            continue;
        };
        // Escaped authored references retain their original outer binding.
        if name.name != marker
            || &source[name.span.start as usize..name.span.end as usize] != marker
        {
            continue;
        }
        if call.arguments.len() != 3 {
            return Err("invalid generated Svelte class directive arguments".into());
        }
        edits.push(name.span);
    }
    if edits.is_empty() {
        return Err("missing generated Svelte class directive call".into());
    }
    let mut output = source.to_owned();
    edits.sort_by_key(|span| span.start);
    for span in edits.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, helper);
    }
    output.push_str(&format!(
        "\nimport{{classDirectives as {helper}}}from\"@tschk/moonshine-svelte/class\";\n"
    ));
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_directive_import_only_renames_generated_plain_calls() {
        let source = r"const authored=\u0043lasses(base,null,{}); const v=<div className={Classes(base,null,{active:flag})}/>;";
        let output = class_directives(source, "Classes", "Fresh").unwrap();
        assert!(output.contains(r"\u0043lasses(base,null,{})"));
        assert!(output.contains("Fresh(base,null,{active:flag})"));
        assert!(output
            .ends_with("import{classDirectives as Fresh}from\"@tschk/moonshine-svelte/class\";\n"));
        assert_eq!(output.matches("import{").count(), 1);
    }

    #[test]
    fn only_generated_conversions_get_target_specific_coercion() {
        let source = r#"const value = "" + Text(read()) + "" + object;"#;
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = transform(source, "Text", target).unwrap();
            assert_eq!(output.matches("read()").count(), 1);
            assert!(output.ends_with("+ \"\" + object;"));
            assert!(!output.contains("Text("));
            assert_eq!(
                output.contains("`${"),
                matches!(target, MoonshineModuleTarget::Dom)
            );
            let allocator = Allocator::default();
            assert!(Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty());
        }
    }
}
