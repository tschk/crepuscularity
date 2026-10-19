//! Cross-file proof that imported Solid JSX component tags are stable local functions.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{Semantic, SemanticBuilder, SymbolFlags, SymbolId};
use oxc_span::SourceType;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A resolved relative module: its identity for cycle detection and its source text.
pub struct SolidComponentModule {
    pub path: PathBuf,
    pub source: String,
}

type Resolver<'r> = dyn FnMut(&Path, &str) -> Result<SolidComponentModule, String> + 'r;

struct ImportRef {
    source: String,
    name: Option<String>,
    type_only: bool,
}

enum Origin {
    Valid,
    Follow { source: String, name: String },
}

struct Context<'r, 'f> {
    resolver: &'r mut Resolver<'f>,
    done: HashSet<(PathBuf, String)>,
}

/// Local names of JSX component tags imported from relative modules, once every
/// export chain target is proven a stable synchronous local function in a file
/// with the leading `@jsxImportSource solid-js` pragma. Input without the pragma
/// yields no names; local-only tags are left to `solid_source::check_components`.
pub fn verified_imports(
    source: &str,
    path: &Path,
    resolver: &mut dyn FnMut(&Path, &str) -> Result<SolidComponentModule, String>,
) -> Result<Vec<String>, String> {
    let used = with_module(source, path, |program, semantic| {
        if crate::solid_source::selector(source, program)?.is_empty() {
            return Ok(Vec::new());
        }
        let imports = import_table(program);
        let scoping = semantic.scoping();
        let mut seen = HashSet::new();
        let mut used = Vec::new();
        for node in semantic.nodes().iter() {
            let AstKind::JSXElement(element) = node.kind() else {
                continue;
            };
            match &element.opening_element.name {
                JSXElementName::Identifier(_) => {}
                JSXElementName::IdentifierReference(id) => {
                    let Some(symbol) = id
                        .reference_id
                        .get()
                        .and_then(|id| scoping.get_reference(id).symbol_id())
                        .or_else(|| scoping.get_binding(scoping.root_scope_id(), id.name))
                    else {
                        continue;
                    };
                    let Some(import) = imports.get(&symbol) else {
                        continue;
                    };
                    let name = scoping.symbol_name(symbol).to_string();
                    if import.type_only {
                        return Err(format!(
                            "Solid component `{name}` is a type-only import and cannot be used as a value"
                        ));
                    }
                    let Some(export) = &import.name else {
                        return Err(format!(
                            "Solid component `{name}` comes from a namespace import; import the component by name or default"
                        ));
                    };
                    if import.source == "solid-js" && matches!(export.as_str(), "For" | "Index") {
                        continue;
                    }
                    if seen.insert(symbol) {
                        used.push((name, import.source.clone(), export.clone()));
                    }
                }
                _ => return Err("Solid member/dynamic component ownership is unsupported".into()),
            }
        }
        Ok(used)
    })?;
    let mut context = Context {
        resolver,
        done: HashSet::new(),
    };
    let mut names = Vec::with_capacity(used.len());
    for (local, specifier, export) in used {
        let target = resolve_relative(&mut *context.resolver, path, &specifier)
            .map_err(|e| format!("{}: component `{local}`: {e}", path.display()))?;
        verify(&mut context, &target, &export)?;
        names.push(local);
    }
    Ok(names)
}

/// Verify a default entry or barrel with the same export-chain proof as a tag.
pub(crate) fn verified_default_export(
    source: &str,
    path: &Path,
    resolver: &mut Resolver<'_>,
) -> Result<bool, String> {
    let has_default = with_module(source, path, |program, _| {
        if crate::solid_source::selector(source, program)?.is_empty() {
            return Ok(false);
        }
        Ok(program.body.iter().any(|statement| match statement {
            Statement::ExportDefaultDeclaration(_) => true,
            Statement::ExportNamedDeclaration(export) => {
                export.export_kind != ImportOrExportKind::Type
                    && export.specifiers.iter().any(|specifier| {
                        specifier.export_kind != ImportOrExportKind::Type
                            && specifier.exported.name() == "default"
                    })
            }
            _ => false,
        }))
    })?;
    if has_default {
        let mut context = Context {
            resolver,
            done: HashSet::new(),
        };
        verify(
            &mut context,
            &SolidComponentModule {
                path: path.to_path_buf(),
                source: source.to_owned(),
            },
            "default",
        )
        .map_err(|error| format!("Solid default component export: {error}"))?;
    }
    Ok(has_default)
}

fn resolve_relative(
    resolver: &mut Resolver<'_>,
    importer: &Path,
    specifier: &str,
) -> Result<SolidComponentModule, String> {
    let lower = specifier.to_ascii_lowercase();
    if lower.ends_with(".svelte") || lower.ends_with(".vue") {
        return Err(format!(
            "`{specifier}` is a foreign component; use a native-frame bridge"
        ));
    }
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return Err(format!(
            "`{specifier}` is not a relative import; external Solid components are unsupported"
        ));
    }
    resolver(importer, specifier)
}

fn with_module<R>(
    source: &str,
    path: &Path,
    f: impl for<'b, 'a> FnOnce(&'b Program<'a>, &'b Semantic<'a>) -> Result<R, String>,
) -> Result<R, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(
        path.with_extension(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
    )
    .map_err(|e| format!("{}: {e}", path.display()))?
    .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: module parse failed: {:?}",
            path.display(),
            parsed.errors
        ));
    }
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !semantic.errors.is_empty() {
        return Err(format!(
            "{}: module semantic errors: {:?}",
            path.display(),
            semantic.errors
        ));
    }
    f(&parsed.program, &semantic.semantic)
}

fn import_table(program: &Program<'_>) -> HashMap<SymbolId, ImportRef> {
    let mut table = HashMap::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            let (local, name, type_only) = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(value) => (
                    &value.local,
                    Some(value.imported.name().to_string()),
                    value.import_kind == ImportOrExportKind::Type,
                ),
                ImportDeclarationSpecifier::ImportDefaultSpecifier(value) => {
                    (&value.local, Some("default".to_string()), false)
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(value) => {
                    (&value.local, None, false)
                }
            };
            if let Some(symbol) = local.symbol_id.get() {
                table.insert(
                    symbol,
                    ImportRef {
                        source: import.source.value.to_string(),
                        name,
                        type_only: type_only || import.import_kind == ImportOrExportKind::Type,
                    },
                );
            }
        }
    }
    table
}

fn verify(
    context: &mut Context<'_, '_>,
    module: &SolidComponentModule,
    export: &str,
) -> Result<(), String> {
    let mut module = SolidComponentModule {
        path: module.path.clone(),
        source: module.source.clone(),
    };
    let mut export = export.to_string();
    let mut chain = HashSet::new();
    loop {
        let key = (module.path.clone(), export.clone());
        if context.done.contains(&key) {
            break;
        }
        if !chain.insert(key) {
            return Err(format!(
                "{}: export `{export}` is part of an unresolved reexport cycle",
                module.path.display()
            ));
        }
        let origin = with_module(&module.source, &module.path, |program, semantic| {
            if crate::solid_source::selector(&module.source, program)?.is_empty() {
                return Err(
                    "Solid component module requires a leading `@jsxImportSource solid-js` pragma"
                        .into(),
                );
            }
            locate(program, semantic, &export)
        })
        .map_err(|e| {
            if e.starts_with(&module.path.display().to_string()) {
                e
            } else {
                format!("{}: export `{export}`: {e}", module.path.display())
            }
        })?;
        match origin {
            Origin::Valid => break,
            Origin::Follow { source, name } => {
                module = resolve_relative(&mut *context.resolver, &module.path, &source)
                    .map_err(|e| format!("{}: export `{export}`: {e}", module.path.display()))?;
                export = name;
            }
        }
    }
    context.done.extend(chain);
    Ok(())
}

fn locate(program: &Program<'_>, semantic: &Semantic<'_>, export: &str) -> Result<Origin, String> {
    let scoping = semantic.scoping();
    let imports = import_table(program);
    let from_symbol = |symbol: SymbolId| -> Result<Origin, String> {
        if let Some(import) = imports.get(&symbol) {
            let name = scoping.symbol_name(symbol);
            if import.type_only {
                return Err(format!("`{name}` is a type-only import"));
            }
            let Some(imported) = &import.name else {
                return Err(format!("`{name}` is a namespace import, not a component"));
            };
            return Ok(Origin::Follow {
                source: import.source.clone(),
                name: imported.clone(),
            });
        }
        stable_function(semantic, symbol)?;
        Ok(Origin::Valid)
    };
    let mut star = false;
    for statement in &program.body {
        match statement {
            Statement::ExportNamedDeclaration(declaration) => {
                if declaration.export_kind == ImportOrExportKind::Type {
                    continue;
                }
                match &declaration.declaration {
                    Some(Declaration::FunctionDeclaration(function)) => {
                        if let Some(id) = function.id.as_ref().filter(|id| id.name == export) {
                            return from_symbol(id.symbol_id.get().ok_or("unbound export")?);
                        }
                    }
                    Some(Declaration::VariableDeclaration(variables)) => {
                        for declarator in &variables.declarations {
                            for id in declarator.id.get_binding_identifiers() {
                                if id.name == export {
                                    return from_symbol(
                                        id.symbol_id.get().ok_or("unbound export")?,
                                    );
                                }
                            }
                        }
                    }
                    Some(_) => {}
                    None => {}
                }
                for specifier in &declaration.specifiers {
                    if specifier.export_kind == ImportOrExportKind::Type
                        || specifier.exported.name() != export
                    {
                        continue;
                    }
                    if let Some(source) = &declaration.source {
                        return Ok(Origin::Follow {
                            source: source.value.to_string(),
                            name: specifier.local.name().to_string(),
                        });
                    }
                    let symbol = scoping
                        .get_binding(
                            scoping.root_scope_id(),
                            specifier.local.name().as_str().into(),
                        )
                        .ok_or_else(|| {
                            format!(
                                "exported binding `{}` is not declared",
                                specifier.local.name()
                            )
                        })?;
                    return from_symbol(symbol);
                }
            }
            Statement::ExportDefaultDeclaration(declaration) if export == "default" => {
                let invalid = || -> Result<Origin, String> {
                    Err("default export requires a local stable synchronous function; async, generator and dynamic identities are unsupported".into())
                };
                return match &declaration.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                        match function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                            Some(symbol) => from_symbol(symbol),
                            None if function.body.is_some()
                                && !function.r#async
                                && !function.generator =>
                            {
                                Ok(Origin::Valid)
                            }
                            None => invalid(),
                        }
                    }
                    other => match other.as_expression().map(Expression::get_inner_expression) {
                        Some(Expression::ArrowFunctionExpression(function))
                            if !function.r#async =>
                        {
                            Ok(Origin::Valid)
                        }
                        Some(Expression::FunctionExpression(function))
                            if !function.r#async && !function.generator =>
                        {
                            Ok(Origin::Valid)
                        }
                        Some(Expression::Identifier(id)) => {
                            match id
                                .reference_id
                                .get()
                                .and_then(|id| scoping.get_reference(id).symbol_id())
                            {
                                Some(symbol) => from_symbol(symbol),
                                None => invalid(),
                            }
                        }
                        _ => invalid(),
                    },
                };
            }
            Statement::ExportAllDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                match &declaration.exported {
                    Some(name) if name.name() == export => {
                        return Err("export is a namespace object, not a component".into());
                    }
                    Some(_) => {}
                    None => star = true,
                }
            }
            _ => {}
        }
    }
    if star && export != "default" {
        return Err("not declared locally and `export * from` lookup is unsupported; reexport the component by name".into());
    }
    Err("module has no such component export".into())
}

fn stable_function(semantic: &Semantic<'_>, symbol: SymbolId) -> Result<(), String> {
    let scoping = semantic.scoping();
    let name = scoping.symbol_name(symbol);
    let declaration = semantic
        .nodes()
        .get_node(scoping.symbol_declaration(symbol));
    let function = match declaration.kind() {
        AstKind::Function(function) => {
            function.body.is_some() && !function.r#async && !function.generator
        }
        AstKind::VariableDeclarator(declarator) => {
            matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                && scoping
                    .symbol_flags(symbol)
                    .contains(SymbolFlags::ConstVariable)
                && declarator
                    .init
                    .as_ref()
                    .is_some_and(|init| match init.get_inner_expression() {
                        Expression::ArrowFunctionExpression(function) => !function.r#async,
                        Expression::FunctionExpression(function) => {
                            !function.r#async && !function.generator
                        }
                        _ => false,
                    })
        }
        _ => false,
    };
    if !function {
        return Err(format!(
            "`{name}` must be a local synchronous function declaration or const function; async, generator, mutable and dynamic identities are unsupported"
        ));
    }
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return Err(format!(
            "`{name}` is redeclared; component function identity must be unique"
        ));
    }
    if scoping
        .get_resolved_references(symbol)
        .any(|reference| reference.is_write())
    {
        return Err(format!(
            "`{name}` is reassigned; component function reassignment is unsupported"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "/** @jsxImportSource solid-js */\n";

    fn run(entry: &str, files: &[(&str, &str, String)]) -> Result<Vec<String>, String> {
        let mut resolver = |_: &Path, specifier: &str| {
            files
                .iter()
                .find(|(spec, _, _)| *spec == specifier)
                .map(|(_, path, source)| SolidComponentModule {
                    path: PathBuf::from(path),
                    source: source.clone(),
                })
                .ok_or_else(|| format!("unresolved {specifier}"))
        };
        verified_imports(entry, Path::new("entry.tsx"), &mut resolver)
    }

    fn solid(body: &str) -> String {
        format!("{P}{body}")
    }

    fn entry(imports: &str, tags: &str) -> String {
        solid(&format!(
            "{imports}\nexport const App = () => <div>{tags}</div>;"
        ))
    }

    #[test]
    fn default_named_alias_and_barrel_in_both_languages() {
        let files = [
            ("./card", "card.tsx", solid("export default function Card() { return <b/>; }")),
            ("./items", "items.jsx", solid("const Item = () => <i/>;\nexport { Item };")),
            (
                "./barrel",
                "barrel.ts",
                solid("export { Leaf as Twig } from './leaf';\nexport { default as Deck } from './card';"),
            ),
            ("./leaf", "leaf.jsx", solid("export const Leaf = () => <s/>;")),
        ];
        let source = entry(
            "import Card from './card';\nimport { Item as Row } from './items';\nimport { Twig, Deck } from './barrel';",
            "<Card/><Row/><Twig/><Deck/><Row/>",
        );
        assert_eq!(
            run(&source, &files).unwrap(),
            ["Card", "Row", "Twig", "Deck"]
        );
    }

    #[test]
    fn import_then_export_and_default_identifier_follow() {
        let files = [
            (
                "./a",
                "a.tsx",
                solid("import { B } from './b';\nexport default B;"),
            ),
            (
                "./b",
                "b.tsx",
                solid("export function B() { return <p/>; }"),
            ),
        ];
        let source = entry("import A from './a';", "<A/>");
        assert_eq!(run(&source, &files).unwrap(), ["A"]);
    }

    #[test]
    fn missing_pragma_is_inert_on_input_and_fatal_on_targets() {
        let source = "import X from 'ui';\nexport const A = () => <X/>;";
        assert!(run(source, &[]).unwrap().is_empty());
        let files = [("./a", "a.tsx", "export const A = () => <i/>;".to_string())];
        let source = entry("import { A } from './a';", "<A/>");
        assert!(run(&source, &files).unwrap_err().contains("pragma"));
        let files = [
            ("./b", "b.tsx", solid("export { A } from './a';")),
            ("./a", "a.tsx", "export const A = () => <i/>;".to_string()),
        ];
        let source = entry("import { A } from './b';", "<A/>");
        assert!(run(&source, &files).unwrap_err().contains("a.tsx"));
    }

    #[test]
    fn external_namespace_member_and_type_only_are_rejected() {
        let source = entry("import { X } from 'ui';", "<X/>");
        assert!(run(&source, &[]).unwrap_err().contains("not a relative"));
        let files = [("./a", "a.tsx", solid("export const A = () => <i/>;"))];
        let source = entry("import * as NS from './a';", "<NS.A/>");
        assert!(run(&source, &files).unwrap_err().contains("member"));
        let source = entry("import * as NS from './a';", "<NS/>");
        assert!(run(&source, &files).unwrap_err().contains("namespace"));
        let source = entry("import type { A } from './a';", "<A/>");
        assert!(run(&source, &files).unwrap_err().contains("type-only"));
        let source = entry("import V from './v.svelte';", "<V/>");
        assert!(run(&source, &files).unwrap_err().contains("foreign"));
    }

    #[test]
    fn unstable_missing_and_unsupported_exports_are_rejected() {
        let cases = [
            ("export let A = () => <i/>;", "synchronous"),
            ("export const A = async () => <i/>;", "synchronous"),
            ("export async function A() { return <i/>; }", "synchronous"),
            ("export function* A() { yield <i/>; }", "synchronous"),
            (
                "export function A() { return <i/>; }\nA = () => <b/>;",
                "reassigned",
            ),
            ("export const B = () => <i/>;", "no such"),
            ("export const A = 1;", "synchronous"),
            ("export * from './other';", "export *"),
            ("export * as A from './other';", "namespace"),
        ];
        for (body, expected) in cases {
            let files = [("./a", "a.tsx", solid(body))];
            let source = entry("import { A } from './a';", "<A/>");
            let error = run(&source, &files).unwrap_err();
            assert!(error.contains(expected), "{body}: {error}");
        }
    }

    #[test]
    fn function_properties_and_unrelated_cycles_are_harmless() {
        let files = [
            (
                "./a",
                "a.tsx",
                solid("import { unrelated } from './b';\nexport function A() { return <i/>; }\nA.displayName = 'A';\nexport const x = unrelated;"),
            ),
            (
                "./b",
                "b.tsx",
                solid("import { A } from './a';\nexport const unrelated = A;"),
            ),
        ];
        let source = entry("import { A } from './a';", "<A/>");
        assert_eq!(run(&source, &files).unwrap(), ["A"]);
    }

    #[test]
    fn reexport_cycle_and_nonrelative_reexport_are_rejected() {
        let files = [
            ("./a", "a.tsx", solid("export { A } from './b';")),
            ("./b", "b.tsx", solid("export { A } from './a';")),
        ];
        let source = entry("import { A } from './a';", "<A/>");
        assert!(run(&source, &files).unwrap_err().contains("cycle"));
        let files = [("./a", "a.tsx", solid("export { A } from 'ui';"))];
        assert!(run(&source, &files).unwrap_err().contains("not a relative"));
    }

    #[test]
    fn long_reexport_chains_do_not_consume_the_thread_stack() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let source = entry("import { A } from './m0';", "<A/>");
                let mut resolver = |_: &Path, specifier: &str| {
                    let index: usize = specifier.strip_prefix("./m").unwrap().parse().unwrap();
                    Ok(SolidComponentModule {
                        path: PathBuf::from(format!("m{index}.tsx")),
                        source: solid(&if index == 2048 {
                            "export const A = () => <i/>;".to_string()
                        } else {
                            format!("export {{ A }} from './m{}';", index + 1)
                        }),
                    })
                };
                assert_eq!(
                    verified_imports(&source, Path::new("entry.tsx"), &mut resolver).unwrap(),
                    ["A"]
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn local_only_tags_are_not_returned() {
        let source = solid("const L = () => <i/>;\nexport const App = () => <L/>;");
        assert!(run(&source, &[]).unwrap().is_empty());
    }
}
