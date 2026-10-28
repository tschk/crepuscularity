//! Explicit per-file Solid provenance, never inferred from JSX or imports.
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::Semantic;
use oxc_span::{GetSpan, Span};
use std::collections::HashSet;

/// Only an exact selector opts a template-shaped input into module compilation.
pub(crate) fn has_selector(source: &str, program: &Program<'_>) -> bool {
    program.comments.iter().any(|comment| {
        let span = comment.content_span();
        source[span.start as usize..span.end as usize]
            .lines()
            .any(|line| {
                let line = line.trim();
                line.strip_prefix('*').unwrap_or(line).trim() == "@jsxImportSource solid-js"
            })
    })
}

/// Consume only a complete leading standard selector; retain the surrounding comments.
pub(crate) fn selector(source: &str, program: &Program<'_>) -> Result<Vec<Span>, String> {
    let leading_end = program
        .body
        .first()
        .map_or(u32::MAX, |node| node.span().start)
        .min(
            program
                .directives
                .first()
                .map_or(u32::MAX, |directive| directive.span.start),
        );
    let mut selectors = Vec::new();
    for comment in &program.comments {
        let content = comment.content_span();
        let text = &source[content.start as usize..content.end as usize];
        if !text.contains("@jsx") {
            continue;
        }
        if comment.span.start >= leading_end {
            return Err("JSX provenance directives must precede the first statement".into());
        }
        let mut offset = content.start as usize;
        for line in text.split_inclusive('\n') {
            if line.contains("@jsx") {
                let directive = line.trim().strip_prefix('*').unwrap_or(line.trim()).trim();
                if directive != "@jsxImportSource solid-js" {
                    return Err("only a complete leading @jsxImportSource solid-js directive is supported; factory/runtime conflicts and quoted mentions are unsupported".into());
                }
                if !selectors.is_empty() {
                    return Err("duplicate JSX provenance directives are unsupported".into());
                }
                let start = offset + line.find('@').ok_or("missing JSX directive marker")?;
                selectors.push(Span::new(start as u32, (start + directive.len()) as u32));
            }
            offset += line.len();
        }
    }
    Ok(selectors)
}

/// Admit stable local functions and imports proven by the source graph resolver.
pub(crate) fn check_components(
    semantic: &Semantic<'_>,
    verified_imports: &[String],
    verified_default_export: bool,
) -> Result<(), String> {
    let nodes = semantic.nodes();
    let mut local = HashSet::new();
    for node in nodes.iter() {
        let foreign_component = |source: &str| {
            let source = source.to_ascii_lowercase();
            source.ends_with(".svelte") || source.ends_with(".vue")
        };
        let foreign_runtime = match node.kind() {
            AstKind::ImportDeclaration(import) => import.import_kind != ImportOrExportKind::Type
                && foreign_component(import.source.value.as_str())
                && import.specifiers.as_ref().is_none_or(|specifiers| specifiers.is_empty() || specifiers.iter().any(|specifier| !matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(value) if value.import_kind == ImportOrExportKind::Type))),
            AstKind::ExportNamedDeclaration(export) => export.export_kind != ImportOrExportKind::Type
                && export.source.as_ref().is_some_and(|source| foreign_component(source.value.as_str()))
                && (export.specifiers.is_empty() || export.specifiers.iter().any(|specifier| specifier.export_kind != ImportOrExportKind::Type)),
            AstKind::ExportAllDeclaration(export) => export.export_kind != ImportOrExportKind::Type && foreign_component(export.source.value.as_str()),
            AstKind::ImportExpression(import) => matches!(import.source.get_inner_expression(), Expression::StringLiteral(source) if foreign_component(source.value.as_str())),
            _ => false,
        };
        if foreign_runtime {
            return Err("explicit Solid source imported component ownership is unsupported for runtime .svelte/.vue imports or reexports; use a native-frame bridge".into());
        }
        match node.kind() {
            AstKind::ImportDeclaration(import) => {
                for specifier in import.specifiers.iter().flatten() {
                    let binding = specifier.local();
                    if verified_imports
                        .iter()
                        .any(|name| name == binding.name.as_str())
                    {
                        if let Some(symbol) = binding.symbol_id.get() {
                            local.insert(symbol);
                        }
                    }
                }
            }
            AstKind::Function(function)
                if function.body.is_some() && !function.r#async && !function.generator =>
            {
                if let Some(id) = &function.id {
                    if let Some(symbol) = id.symbol_id.get() {
                        local.insert(symbol);
                    }
                }
            }
            AstKind::VariableDeclarator(declaration) => {
                let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                    continue;
                };
                let Some(value) = &declaration.init else {
                    continue;
                };
                let safe = match value.get_inner_expression() {
                    Expression::ArrowFunctionExpression(function) => !function.r#async,
                    Expression::FunctionExpression(function) => {
                        !function.r#async && !function.generator
                    }
                    _ => false,
                };
                let immutable = nodes
                    .ancestors(node.id())
                    .find_map(|parent| match parent.kind() {
                        AstKind::VariableDeclaration(value) => {
                            Some(value.kind == VariableDeclarationKind::Const)
                        }
                        _ => None,
                    })
                    .unwrap_or(false);
                if safe && immutable {
                    if let Some(symbol) = id.symbol_id.get() {
                        local.insert(symbol);
                    }
                }
            }
            _ => {}
        }
    }
    let mut used = HashSet::new();
    for node in nodes.iter() {
        if let AstKind::ExportAllDeclaration(export) = node.kind() {
            if export.export_kind != ImportOrExportKind::Type
                && export
                    .exported
                    .as_ref()
                    .is_some_and(|name| name.name() == "default")
            {
                return Err("Solid default component export cannot be a namespace object".into());
            }
        }
        let AstKind::ExportNamedDeclaration(export) = node.kind() else {
            continue;
        };
        for specifier in &export.specifiers {
            if export.export_kind == ImportOrExportKind::Type
                || specifier.export_kind == ImportOrExportKind::Type
                || specifier.exported.name() != "default"
            {
                continue;
            }
            let symbol = semantic.scoping().get_binding(
                semantic.scoping().root_scope_id(),
                specifier.local.name().as_str().into(),
            );
            if export.source.is_some() || !symbol.is_some_and(|symbol| local.contains(&symbol)) {
                if verified_default_export {
                    continue;
                }
                return Err(
                    "Solid default component export requires a local stable synchronous function"
                        .into(),
                );
            }
            used.insert(symbol.expect("validated local default component binding"));
        }
    }
    for node in nodes.iter() {
        let AstKind::ExportDefaultDeclaration(export) = node.kind() else {
            continue;
        };
        let valid = match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                let valid = function.body.is_some() && !function.r#async && !function.generator;
                if valid {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        used.insert(symbol);
                    }
                }
                valid
            }
            declaration => match declaration
                .as_expression()
                .map(Expression::get_inner_expression)
            {
                Some(Expression::ArrowFunctionExpression(function)) => !function.r#async,
                Some(Expression::FunctionExpression(function)) => {
                    !function.r#async && !function.generator
                }
                Some(Expression::Identifier(id)) => {
                    let symbol = id
                        .reference_id
                        .get()
                        .and_then(|id| semantic.scoping().get_reference(id).symbol_id());
                    if let Some(symbol) = symbol.filter(|symbol| local.contains(symbol)) {
                        used.insert(symbol);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        };
        if !valid && !verified_default_export {
            return Err("Solid default component export requires a local stable synchronous function; async, generator and dynamic entry identities are unsupported".into());
        }
    }
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        match &element.opening_element.name {
            JSXElementName::Identifier(_) => {}
            JSXElementName::IdentifierReference(id) => {
                let symbol = id
                    .reference_id
                    .get()
                    .and_then(|id| semantic.scoping().get_reference(id).symbol_id());
                if let Some(symbol) = symbol.filter(|symbol| local.contains(symbol)) {
                    used.insert(symbol);
                } else {
                    return Err("Solid component tags require a local stable function; imported, aliased and dynamic component ownership is unsupported".into());
                }
            }
            _ => return Err("Solid member/dynamic component ownership is unsupported".into()),
        }
    }
    for node in nodes.iter() {
        let declaration_write = match node.kind() {
            AstKind::VariableDeclarator(declaration) => {
                let immutable = nodes
                    .ancestors(node.id())
                    .find_map(|parent| match parent.kind() {
                        AstKind::VariableDeclaration(value) => {
                            Some(value.kind == VariableDeclarationKind::Const)
                        }
                        _ => None,
                    })
                    .unwrap_or(false);
                !immutable
                    && declaration.init.is_some()
                    && declaration.id.get_binding_identifiers().iter().any(|id| {
                        id.symbol_id
                            .get()
                            .is_some_and(|symbol| used.contains(&symbol))
                    })
            }
            AstKind::Function(function) if function.r#async || function.generator => function
                .id
                .as_ref()
                .and_then(|id| id.symbol_id.get())
                .is_some_and(|symbol| used.contains(&symbol)),
            _ => false,
        };
        if declaration_write {
            return Err("Solid component function redeclaration writes are unsupported".into());
        }
        if let AstKind::IdentifierReference(id) = node.kind() {
            if let Some(reference) = id
                .reference_id
                .get()
                .map(|id| semantic.scoping().get_reference(id))
            {
                if reference.is_write()
                    && reference
                        .symbol_id()
                        .is_some_and(|symbol| used.contains(&symbol))
                {
                    return Err("Solid component function reassignment is unsupported".into());
                }
            }
        }
    }
    Ok(())
}
