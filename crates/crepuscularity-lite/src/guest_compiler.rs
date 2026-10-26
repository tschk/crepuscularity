//! Guest-source preparation before V8 evaluation.
//!
//! JavaScript is evaluated as-is. TypeScript and TSX are transformed with Oxc so
//! development can run `.ts` / `.tsx` entries without a separate Bun/esbuild
//! process. This is intentionally a transpiler, not a bundler: production builds
//! still use esbuild through the `cl build` path.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{HelperLoaderMode, TransformOptions, Transformer};

pub fn prepare_guest_source(path: &Path, source: &str) -> Result<String, String> {
    if !requires_oxc(path) {
        return Ok(source.to_string());
    }
    transform_with_oxc(path, &strip_top_level_exports(source))
}

fn requires_oxc(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts" | "tsx" | "jsx")
    )
}

fn strip_top_level_exports(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if matches!(
                trimmed.split_whitespace().next(),
                Some("function" | "const" | "let" | "var" | "class")
            ) {
                line.to_string()
            } else if let Some(rest) = trimmed.strip_prefix("export ") {
                let indent_len = line.len() - trimmed.len();
                format!("{}{}", &line[..indent_len], rest)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn transform_with_oxc(path: &Path, source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| format!("unsupported guest source type: {}", path.display()))?;

    let parse = Parser::new(&allocator, source, source_type).parse();
    if !parse.errors.is_empty() {
        return Err(format_oxc_errors("parse", source, parse.errors));
    }

    let mut program = parse.program;
    let semantic = SemanticBuilder::new()
        .with_excess_capacity(2.0)
        .build(&program);
    if !semantic.errors.is_empty() {
        return Err(format_oxc_errors("semantic", source, semantic.errors));
    }

    let mut options = TransformOptions::from_target("es2020")
        .map_err(|e| format!("failed to configure Oxc target: {e}"))?;
    options.helper_loader.mode = HelperLoaderMode::External;

    let transform = Transformer::new(&allocator, path, &options)
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    if !transform.errors.is_empty() {
        return Err(format_oxc_errors("transform", source, transform.errors));
    }

    Ok(Codegen::new().build(&program).code)
}

fn format_oxc_errors<E>(stage: &str, _source: &str, errors: Vec<E>) -> String
where
    E: std::fmt::Display + std::fmt::Debug,
{
    let mut out = format!("Oxc {stage} failed");
    for error in errors {
        out.push_str("\n  ");
        out.push_str(&error.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_javascript_through() {
        let source = "function run() { return 1; }";
        assert_eq!(
            prepare_guest_source(Path::new("guest.js"), source).unwrap(),
            source
        );
    }

    #[test]
    fn strips_export_before_transforming_typescript() {
        let out = prepare_guest_source(
            Path::new("guest.ts"),
            "export function run(value: number): number { return value + 1; }",
        )
        .unwrap();
        assert!(out.contains("function run(value)"));
        assert!(!out.contains("export"));
        assert!(!out.contains(": number"));
    }
}
