//! Instance exports become descriptors on the native component reference.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::path::Path;

pub(crate) struct InstanceExport {
    local: String,
    public: String,
    cell: bool,
    mutable: bool,
    class: bool,
    prop: bool,
    readonly: bool,
}

impl InstanceExport {
    pub(crate) fn writable_prop(&self) -> Option<&str> {
        self.prop.then_some(self.local.as_str())
    }
    pub(crate) fn type_field(&self) -> String {
        format!(
            "{}[{}]:typeof {}{}",
            if self.readonly { "readonly " } else { "" },
            serde_json::to_string(&self.public).unwrap(),
            self.local,
            if self.cell { ".value" } else { "" }
        )
    }
    pub(crate) fn descriptor(&self) -> String {
        let key = serde_json::to_string(&self.public).unwrap();
        let value = format!("{}{}", self.local, if self.cell { ".value" } else { "" });
        if self.readonly {
            format!("get [{key}](){{return {value};}}")
        } else if self.mutable {
            let mut parameter = "__crepusExportValue".to_owned();
            while parameter == self.local {
                parameter.push('_');
            }
            // Class declarations are writable in JS but TS rejects a bare assignment.
            let target = if self.class {
                format!("({value} as any)")
            } else {
                value.clone()
            };
            format!(
                "get [{key}](){{return {value};}},set [{key}]({parameter}:typeof {value}){{{target}={parameter};}}"
            )
        } else {
            format!("[{key}]:{value}")
        }
    }
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
) -> Result<(String, Vec<InstanceExport>), String> {
    transform_with_source_type(source, path, SourceType::ts())
}

pub(crate) fn transform_with_source_type(
    source: &str,
    path: &Path,
    source_type: SourceType,
) -> Result<(String, Vec<InstanceExport>), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let fail = |message: &str| format!("{}: {message}", path.display());
    if !parsed.errors.is_empty() {
        return Err(fail("invalid Svelte instance export syntax"));
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("invalid Svelte instance export scope"));
    }
    let semantic = &built.semantic;
    let mut edits = Vec::new();
    let mut names = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(_) | Statement::ExportAllDeclaration(_) => {
                return Err(fail(
                    "default and wildcard instance exports are unsupported",
                ));
            }
            Statement::ExportNamedDeclaration(export) => {
                if export.source.is_some() {
                    return Err(fail(
                        "instance reexports are unsupported; use a module script",
                    ));
                }
                if let Some(declaration) = &export.declaration {
                    edits.push((
                        export.span.start as usize,
                        export.span.start as usize + "export".len(),
                        String::new(),
                    ));
                    match declaration {
                        Declaration::VariableDeclaration(declaration) => {
                            if declaration.kind != VariableDeclarationKind::Const {
                                return Err(fail(
                                    "export let/var is unsupported; use $props() or a named export",
                                ));
                            }
                            for declaration in &declaration.declarations {
                                for id in declaration.id.get_binding_identifiers() {
                                    names.push((id.name.to_string(), id.name.to_string(), true));
                                }
                            }
                        }
                        Declaration::FunctionDeclaration(function) => {
                            let id = function
                                .id
                                .as_ref()
                                .ok_or_else(|| fail("anonymous instance export"))?;
                            names.push((id.name.to_string(), id.name.to_string(), true));
                        }
                        Declaration::ClassDeclaration(class) => {
                            let id = class
                                .id
                                .as_ref()
                                .ok_or_else(|| fail("anonymous instance export"))?;
                            names.push((id.name.to_string(), id.name.to_string(), true));
                        }
                        Declaration::TSTypeAliasDeclaration(_)
                        | Declaration::TSInterfaceDeclaration(_) => {}
                        _ => return Err(fail("unsupported instance export declaration")),
                    }
                } else {
                    let mut comments = parsed
                        .program
                        .comments
                        .iter()
                        .filter(|comment| {
                            comment.span.start >= export.span.start
                                && comment.span.end <= export.span.end
                        })
                        .map(|comment| {
                            source[comment.span.start as usize..comment.span.end as usize]
                                .to_owned()
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !comments.is_empty() {
                        comments.push('\n');
                    }
                    edits.push((
                        export.span.start as usize,
                        export.span.end as usize,
                        comments,
                    ));
                    for specifier in &export.specifiers {
                        if export.export_kind == ImportOrExportKind::Type
                            || specifier.export_kind == ImportOrExportKind::Type
                        {
                            continue;
                        }
                        names.push((
                            specifier.local.name().to_string(),
                            specifier.exported.name().to_string(),
                            false,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    // The authored script is TypeScript, not JSX. Disambiguate generic arrows
    // before it is embedded into the later synthetic TSX component.
    for node in semantic.nodes().iter() {
        let AstKind::ArrowFunctionExpression(arrow) = node.kind() else {
            continue;
        };
        let Some(parameters) = &arrow.type_parameters else {
            continue;
        };
        if parameters.params.len() != 1 {
            continue;
        }
        let start = parameters.params[0].span.end as usize;
        let end = parameters.span.end as usize;
        let comma = source.as_bytes()[start..end]
            .iter()
            .enumerate()
            .any(|(offset, byte)| {
                *byte == b','
                    && !parsed.program.comments.iter().any(|comment| {
                        let offset = (start + offset) as u32;
                        comment.span.start <= offset && offset < comment.span.end
                    })
            });
        if !comma {
            edits.push((start, start, ",".to_owned()));
        }
    }
    let mut exports = Vec::new();
    for (local, public, direct) in names {
        let symbol = semantic
            .scoping()
            .get_binding(semantic.scoping().root_scope_id(), local.as_str().into())
            .ok_or_else(|| fail("instance export must name a local binding"))?;
        let flags = semantic.scoping().symbol_flags(symbol);
        if flags.is_import() {
            return Err(fail(
                "imported instance exports are unsupported; use a local binding",
            ));
        }
        let mut cell = false;
        let mut prop = false;
        let mut readonly = false;
        let mut mutable = !flags.is_const_variable() && !flags.is_function();
        if let AstKind::VariableDeclarator(declaration) = semantic.symbol_declaration(symbol).kind()
        {
            if let Some(Expression::CallExpression(call)) = &declaration.init {
                let (root, member) = match call.callee.get_inner_expression() {
                    Expression::Identifier(id) => (Some(id), None),
                    Expression::StaticMemberExpression(member) => {
                        match member.object.get_inner_expression() {
                            Expression::Identifier(id) => {
                                (Some(id), Some(member.property.name.as_str()))
                            }
                            _ => (None, None),
                        }
                    }
                    _ => (None, None),
                };
                if let Some(root) = root.filter(|id| {
                    semantic
                        .scoping()
                        .get_reference(id.reference_id.get().unwrap())
                        .symbol_id()
                        .is_none()
                }) {
                    if root.name == "$derived" {
                        if direct {
                            return Err(fail(
                                "direct derived exports are unsupported; use a named instance alias",
                            ));
                        }
                        cell = true;
                        readonly = true;
                    }
                    if root.name == "$state" && (member.is_none() || member == Some("raw")) {
                        cell = true;
                        mutable = !direct;
                    }
                    if root.name == "$props" && member.is_none() {
                        if let BindingPattern::ObjectPattern(pattern) = &declaration.id {
                            cell = pattern.properties.iter().any(|property| {
                                property
                                    .value
                                    .get_binding_identifiers()
                                    .iter()
                                    .any(|id| id.name == local.as_str())
                            });
                            prop = cell;
                            mutable |= cell;
                        }
                    }
                }
            }
        }
        exports.push(InstanceExport {
            local,
            public,
            cell,
            mutable,
            prop,
            readonly,
            class: matches!(
                semantic.symbol_declaration(symbol).kind(),
                AstKind::Class(_)
            ),
        });
    }
    let mut out = source.to_owned();
    edits.sort_by_key(|edit| (edit.0, edit.1));
    for (start, end, replacement) in edits.into_iter().rev() {
        out.replace_range(start..end, &replacement);
    }
    Ok((out, exports))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_preserve_binding_descriptors_and_strip_only_export_syntax() {
        let (source, exports) = transform("export const kind='counter';export function read(){return count};export class Klass{};let count=$state(0);export{count as current};export type Id=string;", Path::new("child.svelte")).unwrap();
        assert!(!source.contains("export"));
        let descriptors = exports
            .iter()
            .map(InstanceExport::descriptor)
            .collect::<Vec<_>>()
            .join(",");
        assert!(descriptors.contains("[\"kind\"]:kind"));
        assert!(descriptors.contains("[\"read\"]:read"));
        assert!(descriptors.contains("get [\"Klass\"](){return Klass;}"));
        assert!(descriptors.contains("(Klass as any)=__crepusExportValue"));
        assert!(descriptors.contains("get [\"current\"](){return count.value;}"));
        assert!(descriptors.contains("set [\"current\"](__crepusExportValue:typeof count.value)"));
        assert!(descriptors.contains("set [\"Klass\"](__crepusExportValue:typeof Klass)"));
    }
    #[test]
    fn invalid_exports_fail_without_changing_module_exports() {
        for source in [
            "export default 1",
            "export * from './x'",
            "export{a}from './x'",
            "export let name=1",
            "export const count=$derived(1)",
        ] {
            assert!(
                transform(source, Path::new("child.svelte")).is_err(),
                "{source}"
            );
        }
    }
    #[test]
    fn exported_state_cells_and_mutable_aliases_keep_safe_setters() {
        let (_, exports) = transform("export const state=$state({count:1});let value=1;export{value as '__proto__'};let __crepusExportValue=2;export{__crepusExportValue};", Path::new("exports.svelte")).unwrap();
        let descriptors = exports
            .iter()
            .map(InstanceExport::descriptor)
            .collect::<Vec<_>>()
            .join(",");
        assert!(!descriptors.contains("set [\"state\"]"));
        assert!(descriptors.contains("[\"state\"]:state.value"));
        assert!(descriptors.contains(
            "set [\"__proto__\"](__crepusExportValue:typeof value){value=__crepusExportValue;}"
        ));
        assert!(descriptors.contains("__crepusExportValue=__crepusExportValue_;"));
    }
    #[test]
    fn export_lowering_retains_authored_comments() {
        let (source, _) = transform(
            "export /* license */ const kind=1;export {kind /* public alias */ as alias};",
            Path::new("exports.svelte"),
        )
        .unwrap();
        assert!(source.contains("/* license */"));
        assert!(source.contains("/* public alias */"));
    }
    #[test]
    fn exported_destructured_props_require_writable_cells() {
        for declaration in ["let", "const"] {
            let (_, exports) = transform(
                &format!("{declaration}{{label}}=$props();export{{label as title}};"),
                Path::new("exports.svelte"),
            )
            .unwrap();
            assert_eq!(exports[0].writable_prop(), Some("label"));
            assert!(exports[0]
                .descriptor()
                .contains("label.value=__crepusExportValue"));
        }
    }
    #[test]
    fn named_rune_exports_are_live_even_when_state_is_reassigned() {
        let (_, exports) = transform("let value=$state(1);const doubled=$derived(value*2);export{value as alias,doubled};export function increment(){value++};", Path::new("exports.svelte")).unwrap();
        assert!(exports[0]
            .descriptor()
            .contains("value.value=__crepusExportValue"));
        let derived = exports[1].descriptor();
        assert!(derived.contains("return doubled.value"));
        assert!(!derived.contains("set ["));
    }
}

#[cfg(test)]
mod generic_script_tests {
    use super::transform;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::path::Path;

    #[test]
    fn generic_arrows_are_disambiguated_without_rewriting_script_trivia() {
        for (source, expected) in [
            ("const f=<T>(x:T)=>x;", "const f=<T,>(x:T)=>x;"),
            ("const f=<T /*,*/>(x:T)=>x;", "const f=<T, /*,*/>(x:T)=>x;"),
            (
                "const f=<T, /*tail*/>(x:T)=>x;",
                "const f=<T, /*tail*/>(x:T)=>x;",
            ),
            (
                "const f=<T extends string=string>(x:T)=>x;",
                "const f=<T extends string=string,>(x:T)=>x;",
            ),
            (
                "const f=<T>(x:T)=><U>(y:U)=>[x,y];",
                "const f=<T,>(x:T)=><U,>(y:U)=>[x,y];",
            ),
        ] {
            let (output, _) = transform(source, Path::new("Generic.svelte")).expect("typed script");
            assert_eq!(output, expected);
            let allocator = Allocator::default();
            assert!(Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty());
        }
    }

    #[test]
    fn generic_arrow_edits_compose_with_export_removal() {
        let (output, exports) = transform(
            "export const f=<T>(x:T)=>x;export{f as identity};",
            Path::new("Generic.svelte"),
        )
        .expect("typed exported script");
        assert_eq!(output, " const f=<T,>(x:T)=>x;");
        assert_eq!(exports.len(), 2);
    }
}
