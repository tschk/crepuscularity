//! Install the Svelte scheduler only after source-aware transforms finish.
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;

pub(crate) fn wrap(source: &str, component_name: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid compiled Svelte scheduler syntax".into());
    }
    let components: Vec<_> = parsed
        .program
        .body
        .iter()
        .filter_map(|statement| {
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
                .then_some(function)
        })
        .collect();
    if components.len() != 1 {
        return Err("Svelte scheduler requires one named default component function".into());
    }
    let component = components[0];
    if component.r#async || component.generator {
        return Err("Svelte scheduler requires a synchronous component function".into());
    }
    let body = component
        .body
        .as_ref()
        .ok_or_else(|| "Svelte scheduler component body is missing".to_owned())?;
    let mut alias = "__crepusWithRuneScheduler".to_owned();
    while source.contains(&alias) {
        alias.push('_');
    }
    let mut output = source.to_owned();
    // Arrow capture preserves the component call's this, arguments, and props.
    output.insert_str(body.span.end as usize - 1, "\n});\n");
    output.insert_str(
        body.span.start as usize + 1,
        &format!("\nreturn {alias}(()=>{{\n"),
    );
    Ok(format!(
        "import {{withRuneScheduler as {alias}}} from \"@tschk/moonshine-svelte/pre\";\n{output}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(source: &str) -> String {
        let output = wrap(source, "Component").unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        output
    }
    #[test]
    fn whole_component_setup_and_return_share_scheduler_context() {
        let output=lower("import {h} from 'runtime';export default function Component(props:any){const local=setup();return h(Child,{value:local});}");
        assert!(output.contains("return __crepusWithRuneScheduler(()=>{\nconst local=setup();return h(Child,{value:local});\n});"));
        assert!(output.contains("from \"@tschk/moonshine-svelte/pre\""));
        assert!(output.contains("export default function Component(props:any)"));
    }
    #[test]
    fn arrow_preserves_lexical_context_and_comments_hygienically() {
        let output=lower("const __crepusWithRuneScheduler=0;export default function Component(props:any){// keep setup\nconst value=[this,arguments[0],props];return value;}");
        assert!(output.contains("return __crepusWithRuneScheduler_(()=>{"));
        assert!(
            output.contains("// keep setup\nconst value=[this,arguments[0],props];return value;")
        );
        assert!(!output.contains("function(){"));
    }
    #[test]
    fn only_exact_named_default_function_is_wrapped() {
        let output=lower("function Other(){return 1}export default function Component(){function nested(){return 2}return nested()}");
        assert!(output.contains("function Other(){return 1}"));
        assert!(output.contains("function nested(){return 2}"));
        for source in [
            "export default ()=>1",
            "function Component(){return 1}",
            "export default function Other(){return 1}",
            "export default async function Component(){return 1}",
            "export default function* Component(){yield 1}",
            "const broken=;",
        ] {
            assert!(wrap(source, "Component").is_err(), "{source}");
        }
    }
}
