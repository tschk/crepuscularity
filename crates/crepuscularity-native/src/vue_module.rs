//! Isolated bounded authored Vue setup prototype; template parsing stays in core.
use crate::{MoonshineModule, MoonshineModuleTarget};
use crepuscularity_core::ast::{Node, TextPart};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// Imported type data is supplied by the caller's existing source resolver; native never reads files.
pub struct VueTypeModule {
    pub path: PathBuf,
    pub source: String,
}
pub type VueTypeResolver<'a> = dyn FnMut(&Path, &str) -> Result<VueTypeModule, String> + 'a;

fn event_name(name: &str) -> Result<String, String> {
    if let Some(model) = name.strip_prefix("update:") {
        let valid = !model.is_empty()
            && model.starts_with(|ch: char| ch.is_ascii_alphabetic() || matches!(ch, '_' | '$'))
            && model.split('-').enumerate().all(|(index, part)| {
                !part.is_empty()
                    && (index == 0 || !part.starts_with('$'))
                    && part
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
            });
        if !valid {
            return Err("native Vue update events require simple ASCII model names".into());
        }
        return Ok(format!("onUpdate:{}", camelized_ascii_prop(model).unwrap()));
    }
    let mut words = name.split('-');
    let first = words.next().unwrap_or_default();
    if first.is_empty()
        || !first.starts_with(|ch: char| ch.is_ascii_alphabetic())
        || !first.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err("native Vue event names require simple ASCII camel/kebab names".into());
    }
    let mut key = String::from("on");
    for word in std::iter::once(first).chain(words) {
        if word.is_empty() || !word.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return Err("native Vue event names require simple ASCII camel/kebab names".into());
        }
        let mut chars = word.chars();
        let first = chars.next().ok_or("native Vue event requires a name")?;
        key.push(first.to_ascii_uppercase());
        key.extend(chars);
    }
    Ok(key)
}

fn is_model_binding(name: &str) -> bool {
    name == "bind" || name.starts_with("bind.") || name.starts_with("bind:")
}
fn component_model_name(binding: &str) -> Result<&str, String> {
    let head = binding.split('.').next().unwrap_or_default();
    if head == "bind" {
        return Ok("modelValue");
    }
    head.strip_prefix("bind:")
        .ok_or_else(|| "native Vue component model binding is malformed".into())
}

fn component_model_modifiers(binding: &str) -> Result<Option<(String, Vec<&str>)>, String> {
    let modifiers: Vec<&str> = binding.split('.').skip(1).collect();
    if modifiers.is_empty() {
        return Ok(None);
    }
    let key = if binding.starts_with("bind:") {
        format!("{}Modifiers", component_model_name(binding)?)
    } else {
        "modelModifiers".to_owned()
    };
    Ok(Some((key, modifiers)))
}

fn event_shape(ty: &TSType<'_>) -> Result<HashSet<String>, String> {
    let TSType::TSTypeLiteral(body) = ty else {
        return Err("native Vue defineEmits requires an inline event tuple map".into());
    };
    let mut keys = HashSet::new();
    let mut callback_keys = HashSet::new();
    for member in &body.members {
        let TSSignature::TSPropertySignature(property) = member else {
            return Err("native Vue defineEmits requires event tuple properties".into());
        };
        if property.computed || property.optional {
            return Err("native Vue defineEmits requires required static event names".into());
        }
        let name = match &property.key {
            PropertyKey::StaticIdentifier(key) => key.name.as_str(),
            PropertyKey::StringLiteral(key) => key.value.as_str(),
            _ => return Err("native Vue defineEmits requires static string event names".into()),
        };
        let callback = event_name(name)?;
        if !callback_keys.insert(callback.clone())
            || !callback_keys.insert(format!("{callback}Once"))
        {
            return Err("native Vue event listener keys collide after normalization".into());
        }
        if !keys.insert(name) {
            return Err("native Vue defineEmits duplicate event name".into());
        }
        if !property
            .type_annotation
            .as_ref()
            .is_some_and(|annotation| matches!(annotation.type_annotation, TSType::TSTupleType(_)))
        {
            return Err("native Vue defineEmits requires tuple payloads".into());
        }
    }
    struct PublicType {
        query: bool,
    }
    impl<'a> Visit<'a> for PublicType {
        fn visit_ts_type_query(&mut self, _: &TSTypeQuery<'a>) {
            self.query = true;
        }
    }
    let mut check = PublicType { query: false };
    check.visit_ts_type(ty);
    if check.query {
        return Err("native Vue public emits type queries are unsupported".into());
    }
    Ok(keys.into_iter().map(str::to_owned).collect())
}

#[derive(Debug, PartialEq, Eq)]
struct PropInfo {
    name: String,
    boolean_cast: Option<bool>,
    sole_function: bool,
    duplicate: bool,
}

fn merge_prop(keys: &mut Vec<PropInfo>, prop: PropInfo) -> Result<(), String> {
    if let Some(previous) = keys.iter_mut().find(|previous| previous.name == prop.name) {
        if previous.boolean_cast.is_some() || prop.boolean_cast.is_some() {
            return Err("native Vue duplicate Boolean prop declarations are unsupported".into());
        }
        previous.duplicate = true;
    } else {
        keys.push(prop);
    }
    Ok(())
}

fn prop_value_types(
    ty: &TSType<'_>,
    types: &mut Vec<bool>,
    program: &Program<'_>,
) -> Result<(), String> {
    match ty {
        TSType::TSBooleanKeyword(_) => types.push(true),
        TSType::TSStringKeyword(_) => types.push(false),
        TSType::TSLiteralType(literal) => match literal.literal {
            TSLiteral::BooleanLiteral(_) => types.push(true),
            TSLiteral::StringLiteral(_) => types.push(false),
            _ => {}
        },
        TSType::TSUnionType(union) => {
            for item in &union.types { prop_value_types(item, types, program)?; }
        }
        TSType::TSIntersectionType(intersection) => {
            let mut members = Vec::new();
            for item in &intersection.types { prop_value_types(item, &mut members, program)?; }
            if members.contains(&true) {
                return Err("native Vue intersected Boolean prop values are unsupported".into());
            }
            types.extend(members);
        }
        TSType::TSParenthesizedType(inner) => prop_value_types(&inner.type_annotation, types, program)?,
        TSType::TSTypeReference(reference) if matches!(&reference.type_name, TSTypeName::IdentifierReference(name) if name.name == "Set") && reference.type_arguments.as_ref().is_some_and(|args| args.params.len() == 1) => {
            struct SetBinding(bool);
            impl<'a> Visit<'a> for SetBinding {
                fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
                    self.0 |= id.name == "Set";
                }
            }
            let mut binding = SetBinding(false);
            binding.visit_program(program);
            if binding.0 { return Err("native Vue Set prop types with local Set bindings are unsupported".into()); }
        }
        TSType::TSTypeReference(_) | TSType::TSConditionalType(_) | TSType::TSIndexedAccessType(_) | TSType::TSTypeQuery(_) | TSType::TSTypeOperatorType(_) | TSType::TSMappedType(_) | TSType::TSImportType(_) => return Err("native Vue indirect prop value types are unsupported until casting metadata is resolved".into()),
        _ => {}
    }
    Ok(())
}

// Vue removes unknown constructors when a Function constructor is present.
// Keep that finite runtime classification separate from TypeScript assignability.
fn sole_function_prop(ty: &TSType<'_>) -> bool {
    fn constructors(ty: &TSType<'_>) -> (bool, bool) {
        match ty {
            TSType::TSFunctionType(_) => (true, false),
            TSType::TSParenthesizedType(inner) => constructors(&inner.type_annotation),
            TSType::TSTypeLiteral(literal) => {
                let function = literal.members.iter().any(|member| {
                    matches!(
                        member,
                        TSSignature::TSCallSignatureDeclaration(_)
                            | TSSignature::TSConstructSignatureDeclaration(_)
                    )
                });
                let object = literal.members.is_empty()
                    || literal.members.iter().any(|member| {
                        !matches!(
                            member,
                            TSSignature::TSCallSignatureDeclaration(_)
                                | TSSignature::TSConstructSignatureDeclaration(_)
                        )
                    });
                (function, object)
            }
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .map(constructors)
                .fold((false, false), |a, b| (a.0 || b.0, a.1 || b.1)),
            TSType::TSIntersectionType(intersection) => intersection
                .types
                .iter()
                .map(constructors)
                .fold((false, false), |a, b| (a.0 || b.0, a.1 || b.1)),
            // These infer Unknown, not a runtime constructor, in pinned Vue 3.5.13.
            // A constructor signature literal above differs from `new () => T`.
            TSType::TSUndefinedKeyword(_)
            | TSType::TSNeverKeyword(_)
            | TSType::TSAnyKeyword(_)
            | TSType::TSUnknownKeyword(_)
            | TSType::TSVoidKeyword(_)
            | TSType::TSConstructorType(_) => (false, false),
            _ => (false, true),
        }
    }
    let (function, other) = constructors(ty);
    function && !other
}

fn props_macro_call<'s, 'a>(
    call: &'s CallExpression<'a>,
    semantic: &oxc_semantic::Semantic<'a>,
) -> Result<(&'s CallExpression<'a>, Option<&'s ObjectExpression<'a>>), String> {
    let Some(id) = call.callee.get_identifier_reference() else {
        return Ok((call, None));
    };
    if id.name != "withDefaults"
        || id.reference_id.get().is_some_and(|reference| {
            semantic
                .scoping()
                .get_reference(reference)
                .symbol_id()
                .is_some()
        })
    {
        return Ok((call, None));
    }
    if call.optional || call.type_arguments.is_some() || call.arguments.len() != 2 {
        return Err(
            "native Vue withDefaults requires typed defineProps and one finite defaults object"
                .into(),
        );
    }
    let Some(Expression::CallExpression(props)) = call.arguments[0]
        .as_expression()
        .map(Expression::get_inner_expression)
    else {
        return Err("native Vue withDefaults requires typed defineProps as first argument".into());
    };
    if !props.callee.is_specific_id("defineProps") || props.optional {
        return Err("native Vue withDefaults requires typed defineProps as first argument".into());
    }
    let Some(Expression::ObjectExpression(defaults)) = call.arguments[1]
        .as_expression()
        .map(Expression::get_inner_expression)
    else {
        return Err("native Vue withDefaults requires a finite literal defaults object".into());
    };
    Ok((props, Some(defaults)))
}

fn default_keys(defaults: &ObjectExpression<'_>) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for property in &defaults.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Err("native Vue withDefaults spread defaults are unsupported".into());
        };
        if property.computed || property.kind != PropertyKind::Init {
            return Err("native Vue withDefaults requires static data defaults".into());
        }
        let name = match &property.key {
            PropertyKey::StaticIdentifier(key) => key.name.to_string(),
            PropertyKey::StringLiteral(key) => key.value.to_string(),
            _ => return Err("native Vue withDefaults requires static string default keys".into()),
        };
        if name == "__proto__" {
            return Err("native Vue withDefaults prototype default keys are unsupported".into());
        }
        if names.contains(&name) {
            return Err("native Vue withDefaults duplicate default keys are unsupported".into());
        }
        names.push(name);
    }
    Ok(names)
}

fn prop_members(
    members: &[TSSignature<'_>],
    program: &Program<'_>,
) -> Result<Vec<PropInfo>, String> {
    let mut keys = Vec::new();
    for member in members {
        let TSSignature::TSPropertySignature(property) = member else {
            return Err("native Vue props require finite property signatures".into());
        };
        if property.computed {
            return Err("native Vue computed prop keys are unsupported".into());
        }
        let name = match &property.key {
            PropertyKey::StaticIdentifier(key) => key.name.to_string(),
            PropertyKey::StringLiteral(key) => key.value.to_string(),
            _ => return Err("native Vue props require static string names".into()),
        };
        let mut types = Vec::new();
        if let Some(annotation) = &property.type_annotation {
            prop_value_types(&annotation.type_annotation, &mut types, program)?;
        }
        if matches!(name.as_str(), "id" | "class" | "style") && types.contains(&true) {
            return Err("native Vue Boolean id/class/style props are unsupported".into());
        }
        if name.contains('-') && types.contains(&true) {
            return Err("native Vue hyphenated Boolean prop declarations are unsupported".into());
        }
        let boolean_cast = types.iter().position(|kind| *kind).map(|boolean| {
            types
                .iter()
                .position(|kind| !*kind)
                .is_none_or(|string| boolean < string)
        });
        let sole_function = property
            .type_annotation
            .as_ref()
            .is_some_and(|annotation| sole_function_prop(&annotation.type_annotation));
        merge_prop(
            &mut keys,
            PropInfo {
                name,
                boolean_cast,
                sole_function,
                duplicate: false,
            },
        )?;
    }
    Ok(keys)
}

fn prop_shape(
    ty: &TSType<'_>,
    program: &Program<'_>,
    path: &Path,
    resolver: &mut VueTypeResolver<'_>,
    stack: &mut HashSet<(PathBuf, String)>,
) -> Result<Vec<PropInfo>, String> {
    let keys = match ty {
        TSType::TSTypeLiteral(body) => prop_members(&body.members, program),
        TSType::TSParenthesizedType(inner) => {
            prop_shape(&inner.type_annotation, program, path, resolver, stack)
        }
        TSType::TSIntersectionType(intersection) => {
            let mut keys = Vec::new();
            for item in &intersection.types {
                for key in prop_shape(item, program, path, resolver, stack)? {
                    merge_prop(&mut keys, key)?;
                }
            }
            Ok(keys)
        }
        TSType::TSTypeReference(reference) if reference.type_arguments.is_none() => {
            let TSTypeName::IdentifierReference(name) = &reference.type_name else {
                return Err("native Vue qualified prop types are unsupported".into());
            };
            named_prop_shape(name.name.as_str(), program, path, resolver, stack)
        }
        _ => Err("native Vue props require a finite object type".into()),
    }?;
    if keys.iter().any(|prop| prop.boolean_cast.is_some())
        && keys.iter().any(|prop| prop.name.contains('-'))
    {
        return Err(
            "native Vue hyphenated prop declarations with Boolean casting are unsupported".into(),
        );
    }
    Ok(keys)
}

const MAX_VUE_PROP_TYPE_DEPTH: usize = 64;

fn named_prop_shape(
    name: &str,
    program: &Program<'_>,
    path: &Path,
    resolver: &mut VueTypeResolver<'_>,
    stack: &mut HashSet<(PathBuf, String)>,
) -> Result<Vec<PropInfo>, String> {
    if stack.len() >= MAX_VUE_PROP_TYPE_DEPTH {
        return Err(format!(
            "native Vue props type resolution exceeds {MAX_VUE_PROP_TYPE_DEPTH} levels"
        ));
    }
    let token = (path.to_owned(), name.to_owned());
    if !stack.insert(token.clone()) {
        return Err(format!("native Vue cyclic props type {name}"));
    }
    let result = (|| {
        let declarations = program
            .body
            .iter()
            .filter(|statement| {
                let declaration = match statement {
                    Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                    _ => None,
                };
                match (statement, declaration) {
                    (Statement::TSInterfaceDeclaration(item), _)
                    | (_, Some(Declaration::TSInterfaceDeclaration(item))) => item.id.name == name,
                    (Statement::TSTypeAliasDeclaration(item), _)
                    | (_, Some(Declaration::TSTypeAliasDeclaration(item))) => item.id.name == name,
                    _ => false,
                }
            })
            .count();
        if declarations > 1 {
            return Err("native Vue merged props declarations are unsupported".into());
        }
        for statement in &program.body {
            let declaration = match statement {
                Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
                _ => None,
            };
            let alias = match (statement, declaration) {
                (Statement::TSTypeAliasDeclaration(alias), _)
                | (_, Some(Declaration::TSTypeAliasDeclaration(alias)))
                    if alias.id.name == name =>
                {
                    Some(alias)
                }
                _ => None,
            };
            if let Some(alias) = alias {
                if alias.type_parameters.is_some() {
                    return Err("native Vue generic props types are unsupported".into());
                }
                return prop_shape(&alias.type_annotation, program, path, resolver, stack);
            }
            let interface = match (statement, declaration) {
                (Statement::TSInterfaceDeclaration(interface), _)
                | (_, Some(Declaration::TSInterfaceDeclaration(interface)))
                    if interface.id.name == name =>
                {
                    Some(interface)
                }
                _ => None,
            };
            if let Some(interface) = interface {
                if interface.type_parameters.is_some() {
                    return Err("native Vue generic props interfaces are unsupported".into());
                }
                let mut keys = prop_members(&interface.body.body, program)?;
                for base in &interface.extends {
                    let Expression::Identifier(id) = &base.expression else {
                        return Err("native Vue qualified props inheritance is unsupported".into());
                    };
                    if base.type_arguments.is_some() {
                        return Err("native Vue generic props inheritance is unsupported".into());
                    }
                    for key in named_prop_shape(id.name.as_str(), program, path, resolver, stack)? {
                        merge_prop(&mut keys, key)?;
                    }
                }
                return Ok(keys);
            }
            if let Statement::ImportDeclaration(import) = statement {
                for specifier in import.specifiers.iter().flatten() {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                        continue;
                    };
                    if specifier.local.name != name {
                        continue;
                    }
                    let module = resolver(path, import.source.value.as_str())?;
                    let allocator = Allocator::default();
                    let parsed = Parser::new(&allocator, &module.source, SourceType::tsx()).parse();
                    if !parsed.errors.is_empty() {
                        return Err(format!(
                            "{}: native Vue imported props parse failed: {:?}",
                            module.path.display(),
                            parsed.errors
                        ));
                    }
                    return named_prop_shape(
                        specifier.imported.name().as_str(),
                        &parsed.program,
                        &module.path,
                        resolver,
                        stack,
                    );
                }
            }
            if let Statement::ExportNamedDeclaration(export) = statement {
                if let Some(source) = &export.source {
                    for specifier in &export.specifiers {
                        if specifier.exported.name() != name {
                            continue;
                        }
                        let module = resolver(path, source.value.as_str())?;
                        let allocator = Allocator::default();
                        let parsed =
                            Parser::new(&allocator, &module.source, SourceType::tsx()).parse();
                        if !parsed.errors.is_empty() {
                            return Err(format!(
                                "{}: native Vue imported props parse failed: {:?}",
                                module.path.display(),
                                parsed.errors
                            ));
                        }
                        return named_prop_shape(
                            specifier.local.name().as_str(),
                            &parsed.program,
                            &module.path,
                            resolver,
                            stack,
                        );
                    }
                }
            }
        }
        Err(format!("native Vue unresolved props type {name}"))
    })();
    stack.remove(&token);
    result
}

pub fn compile_moonshine_vue(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<MoonshineModule, String> {
    compile_moonshine_vue_with_resolver(source, path, target, &mut |_, _| {
        Err("native Vue imported props types require an explicit source resolver".into())
    })
}

pub fn compile_moonshine_vue_with_resolver(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    resolver: &mut VueTypeResolver<'_>,
) -> Result<MoonshineModule, String> {
    let (sfc, metadata) = crepuscularity_core::parser::parse_vue_sfc_with_metadata(source)
        .map_err(|e| e.to_string())?;
    let mut styles = Vec::new();
    for (style, attributes) in sfc.styles.iter().zip(&metadata.style_attributes) {
        if attributes
            .iter()
            .any(|name| !matches!(name.as_str(), "scoped" | "module" | "lang"))
        {
            return Err("unsupported Vue style attribute".into());
        }
        if style.scoped
            || style.module
            || !matches!(style.lang.as_deref(), None | Some("") | Some("css"))
        {
            return Err("native Vue styles are unsupported unless plain unscoped CSS without modules or preprocessors".into());
        }
        // shortcut: external references, escapes and CSS bindings need an owned asset/parser contract.
        let lower = style.content.to_ascii_lowercase();
        if lower.contains("url(")
            || lower.contains("image-set(")
            || lower.contains("@import")
            || lower.contains("v-bind")
            || style.content.contains('\\')
        {
            return Err(
                "native Vue plain CSS does not yet support URLs, imports, escapes or v-bind".into(),
            );
        }
        if !style.content.trim().is_empty() {
            styles.push(style.content.clone());
        }
    }
    if metadata.script_generics.iter().any(Option::is_some) {
        return Err("native Vue generic script components are unsupported".into());
    }
    let scriptless = sfc.scripts.is_empty();
    if !scriptless
        && (sfc.scripts.len() != 1
            || !sfc.scripts[0].setup
            || !matches!(
                sfc.scripts[0].lang.as_deref(),
                None | Some("") | Some("js") | Some("ts")
            ))
    {
        return Err("native Vue requires one JS/TS script setup block".into());
    }
    let script = sfc
        .scripts
        .first()
        .map_or("", |script| script.content.as_str());
    sfc.template
        .as_deref()
        .ok_or("native Vue requires a template")?;
    let mut nodes =
        crepuscularity_core::parser::parse_vue_native_file(source).map_err(|e| e.to_string())?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, script, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("Vue setup parse failed: {:?}", parsed.errors));
    }
    let setup_semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut occupied_names = VueNames::default();
    occupied_names.visit_program(&parsed.program);
    for node in &nodes {
        occupied_names.template(node);
    }
    let mut name_buf = String::new();
    let mut fresh = |prefix: &str| {
        use std::fmt::Write;
        for i in 0usize.. {
            name_buf.clear();
            write!(&mut name_buf, "{prefix}{i}").unwrap();
            if !source.contains(&name_buf)
                && !occupied_names
                    .0
                    .iter()
                    .any(|occupied| occupied.starts_with(name_buf.as_str()))
            {
                return name_buf.clone();
            }
        }
        unreachable!("finite source cannot occupy every generated identifier")
    };
    let props = fresh("__crepusVueProps");
    let emitter = fresh("__crepusVueEmitter");
    let listeners = fresh("__crepusVueListeners");
    let unref = fresh("__crepusVueUnref");
    let name = fresh("__crepusVueComponent");
    let text = fresh("__crepusVueText");
    let args = fresh("__crepusVueEventArgs");
    let handler = fresh("__crepusVueEventHandler");
    let slot_props = fresh("__crepusVueSlotProps");
    let slot_outlet = fresh("__crepusVueSlotOutlet");
    let text_helper = if matches!(target, MoonshineModuleTarget::Ssr) {
        "toDisplayStringSSR"
    } else {
        "toDisplayString"
    };
    let mut imports = format!(
        "import {{{text_helper} as {text}}} from '@tschk/moonshine-vue/text';\nimport {{ unref as {unref} }} from '@tschk/moonshine-vue/reactive';\n"
    );
    let slot_helper = if matches!(target, MoonshineModuleTarget::Ssr) {
        "createServerSlotOutlet"
    } else {
        "createDOMSlotOutlet"
    };
    let mut slots = SlotEmit {
        source: props.clone(),
        transport: slot_props,
        outlet: slot_outlet,
        prefix: fresh("__crepusVueOutlet"),
        declarations: vec![],
        normalize_class: fresh("__crepusVueNormalizeClass"),
        normalize_style: fresh("__crepusVueNormalizeStyle"),
        normalize_static_style: fresh("__crepusVueNormalizeStaticStyle"),
        normalization: false,
        constant_styles: HashSet::new(),
        normalize_vnode_style: fresh("__crepusVueVNodeStyle"),
        stable_handlers: HashSet::new(),
        server: matches!(target, MoonshineModuleTarget::Ssr),
        model: fresh("__crepusVueTextModel"),
        checked_model: fresh("__crepusVueCheckedModel"),
        checked_models: false,
        text_models: false,
        select_model: fresh("__crepusVueSelectModel"),
        select_scope: fresh("__crepusVueSelectScope"),
        capture_select_scope: fresh("__crepusVueCaptureSelectScope"),
        option_model: fresh("__crepusVueOptionModel"),
        select_models: false,
        selections: Vec::new(),
        models: Vec::new(),
        model_node: fresh("__crepusVueModelElement"),
        model_bindings: model_bindings(&parsed.program, &setup_semantic),
    };
    let module_type_scopes: Vec<_> = parsed
        .program
        .body
        .iter()
        .filter(|statement| {
            matches!(
                statement,
                Statement::ImportDeclaration(_)
                    | Statement::TSTypeAliasDeclaration(_)
                    | Statement::TSInterfaceDeclaration(_)
            )
        })
        .map(GetSpan::span)
        .collect();
    let in_module_type_scope = |span: oxc_span::Span| {
        module_type_scopes
            .iter()
            .any(|scope| scope.start <= span.start && scope.end >= span.end)
    };
    // A merged type/value symbol's representative declaration can hide its setup value.
    let setup_value_symbols: HashSet<_> = setup_semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::BindingIdentifier(id) = node.kind() else {
                return None;
            };
            (!in_module_type_scope(id.span))
                .then(|| id.symbol_id.get())
                .flatten()
        })
        .collect();
    let mut edits = vec![];
    let mut ref_helpers = HashSet::new();
    let mut lifecycle_helpers = HashSet::new();
    let mut refs = HashSet::new();
    let mut components = HashSet::new();
    let mut component_sources = HashMap::new();
    let mut props_calls = 0;
    let mut emits_type: Option<String> = None;
    let mut emit_events = HashSet::new();
    let mut emits_spans = Vec::new();
    let mut props_spans = Vec::new();
    let mut props_type = scriptless.then(|| "{}".to_owned());
    let props_view = fresh("__crepusVuePropsView");
    let mut boolean_props_view = false;
    let mut default_props_view = false;
    let mut default_hoists = String::new();
    let defaults_name = fresh("__crepusVueDefaults");
    let defaults_type = fresh("__crepusVueDefaultValues");
    let defaults_key = fresh("__crepusVueDefaultKey");
    let defaults_scheduler = fresh("__crepusVueDefaultsScheduler");
    let props_helper = if matches!(target, MoonshineModuleTarget::Ssr) {
        "createServerProps"
    } else {
        "createProps"
    };
    let props_import_offset = imports.len();
    let mut stable_handlers = HashSet::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                if import.source.value == "vue" {
                    if import.with_clause.is_some() || import.phase.is_some() {
                        return Err("native Vue imports require plain named helpers".into());
                    }
                    let mut groups = std::collections::BTreeMap::<&str, Vec<String>>::new();
                    for spec in import.specifiers.iter().flatten() {
                        let ImportDeclarationSpecifier::ImportSpecifier(spec) = spec else {
                            return Err("native Vue requires named imports".into());
                        };
                        let helper = spec.imported.name();
                        let lifecycle = matches!(
                            helper.as_str(),
                            "onMounted" | "onBeforeUnmount" | "onUnmounted"
                        );
                        let context = matches!(helper.as_str(), "provide" | "inject");
                        if lifecycle {
                            if let Some(symbol) = spec.local.symbol_id.get() {
                                lifecycle_helpers.insert(symbol);
                            }
                        }
                        if !lifecycle
                            && !context
                            && !matches!(
                                helper.as_str(),
                                "ref"
                                    | "computed"
                                    | "unref"
                                    | "toRef"
                                    | "toRefs"
                                    | "shallowRef"
                                    | "reactive"
                                    | "watch"
                            )
                        {
                            return Err(format!("unsupported native Vue helper {helper}"));
                        }
                        slots.constant_styles.insert(spec.local.name.to_string());
                        if matches!(helper.as_str(), "ref" | "computed" | "shallowRef" | "toRef") {
                            ref_helpers.insert(spec.local.name.to_string());
                        }
                        groups
                            .entry(if lifecycle {
                                "@tschk/moonshine-vue/lifecycle"
                            } else if context {
                                "@tschk/moonshine-vue/context"
                            } else {
                                "@tschk/moonshine-vue/reactive"
                            })
                            .or_default()
                            .push(
                                script[spec.span.start as usize..spec.span.end as usize].to_owned(),
                            );
                    }
                    if groups.is_empty() {
                        let span = import.source.span;
                        let mut declaration =
                            script[import.span.start as usize..import.span.end as usize].to_owned();
                        declaration.replace_range(
                            (span.start - import.span.start) as usize
                                ..(span.end - import.span.start) as usize,
                            "'@tschk/moonshine-vue/reactive'",
                        );
                        imports.push_str(&declaration);
                    } else {
                        let prefix = if import.import_kind == ImportOrExportKind::Type {
                            "import type"
                        } else {
                            "import"
                        };
                        for (runtime, names) in groups {
                            imports.push_str(&format!(
                                "{prefix} {{ {} }} from {runtime:?};\n",
                                names.join(", ")
                            ));
                        }
                    }
                } else if import.source.value.starts_with("@vue/")
                    || import.source.value.starts_with("vue/")
                {
                    return Err("native Vue host subpaths unsupported".into());
                } else {
                    if import.import_kind != ImportOrExportKind::Type {
                        for specifier in import.specifiers.iter().flatten() {
                            let local = match specifier {
                                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                                    Some(&specifier.local)
                                }
                                ImportDeclarationSpecifier::ImportSpecifier(specifier)
                                    if specifier.import_kind != ImportOrExportKind::Type =>
                                {
                                    Some(&specifier.local)
                                }
                                _ => None,
                            };
                            if let Some(local) = local {
                                if matches!(
                                    specifier,
                                    ImportDeclarationSpecifier::ImportDefaultSpecifier(_)
                                ) || matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(spec) if spec.imported.name()=="default")
                                {
                                    component_sources.insert(
                                        local.name.to_string(),
                                        import.source.value.to_string(),
                                    );
                                    if import.source.value.ends_with(".vue") {
                                        slots.constant_styles.insert(local.name.to_string());
                                    }
                                }
                                components.insert(local.name.to_string());
                                refs.insert(local.name.to_string());
                                stable_handlers.insert(local.name.to_string());
                            }
                        }
                    }
                    imports.push_str(&script[import.span.start as usize..import.span.end as usize]);
                }
                imports.push('\n');
                edits.push((
                    import.span.start as usize,
                    import.span.end as usize,
                    String::new(),
                ));
            }
            Statement::VariableDeclaration(variables) => {
                for declaration in &variables.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &declaration.id {
                        refs.insert(id.name.to_string());
                        if variables.kind == VariableDeclarationKind::Const
                            && declaration.init.as_ref().is_some_and(style_setup_const)
                        {
                            slots.constant_styles.insert(id.name.to_string());
                        }
                        if variables.kind == VariableDeclarationKind::Const
                            && declaration.init.as_ref().is_some_and(|value| {
                                matches!(
                                    value.get_inner_expression(),
                                    Expression::ArrowFunctionExpression(_)
                                        | Expression::FunctionExpression(_)
                                )
                            })
                            && id.symbol_id.get().is_some_and(|symbol| {
                                !setup_semantic
                                    .symbol_references(symbol)
                                    .any(|reference| reference.is_write())
                            })
                        {
                            stable_handlers.insert(id.name.to_string());
                        }
                    } else {
                        return Err("native Vue setup destructuring unsupported".into());
                    }
                    if let Some(Expression::CallExpression(call)) = declaration
                        .init
                        .as_ref()
                        .map(Expression::get_inner_expression)
                    {
                        let outer_call = call;
                        let (call, defaults) = props_macro_call(call, &setup_semantic)?;
                        if let Some(callee) = call.callee.get_identifier_reference() {
                            if callee.name == "defineEmits"
                                && !callee.reference_id.get().is_some_and(|id| {
                                    setup_semantic
                                        .scoping()
                                        .get_reference(id)
                                        .symbol_id()
                                        .is_some()
                                })
                            {
                                if emits_type.is_some()
                                    || variables.kind != VariableDeclarationKind::Const
                                    || !call.arguments.is_empty()
                                    || call.optional
                                {
                                    return Err("native Vue defineEmits requires one typed const declaration".into());
                                }
                                let arguments = call.type_arguments.as_ref().ok_or(
                                    "native Vue defineEmits requires an inline event tuple map",
                                )?;
                                if arguments.params.len() != 1 {
                                    return Err(
                                        "native Vue defineEmits requires one event type".into()
                                    );
                                }
                                emit_events = event_shape(&arguments.params[0])?;
                                let span = arguments.params[0].span();
                                let ty = script[span.start as usize..span.end as usize].to_owned();
                                emits_type = Some(ty.clone());
                                emits_spans.push(call.span);
                                edits.push((
                                    call.span.start as usize,
                                    call.span.end as usize,
                                    format!("{emitter}<{ty}>({props})"),
                                ));
                            } else if callee.name == "defineProps" {
                                if callee.reference_id.get().is_some_and(|id| {
                                    setup_semantic
                                        .scoping()
                                        .get_reference(id)
                                        .symbol_id()
                                        .is_some()
                                }) {
                                    return Err("native Vue defineProps must be an unshadowed compiler macro".into());
                                }
                                props_calls += 1;
                                if props_calls > 1
                                    || !call.arguments.is_empty()
                                    || call.type_arguments.is_none()
                                    || !matches!(
                                        declaration.id,
                                        BindingPattern::BindingIdentifier(_)
                                    )
                                {
                                    return Err("native Vue defineProps requires one typed identifier declaration".into());
                                }
                                let args = call
                                    .type_arguments
                                    .as_ref()
                                    .ok_or("native Vue defineProps requires a props type")?;
                                if args.params.len() != 1 {
                                    return Err(
                                        "native Vue defineProps requires one props type".into()
                                    );
                                }
                                struct PropsType {
                                    query: bool,
                                }
                                impl<'a> Visit<'a> for PropsType {
                                    fn visit_ts_type_query(&mut self, _: &TSTypeQuery<'a>) {
                                        self.query = true;
                                    }
                                }
                                let mut props_check = PropsType { query: false };
                                props_check.visit_ts_type(&args.params[0]);
                                if props_check.query {
                                    return Err(
                                        "native Vue public props type queries are unsupported"
                                            .into(),
                                    );
                                }
                                let span = args.params[0].span();
                                props_type =
                                    Some(script[span.start as usize..span.end as usize].to_owned());
                                // Whitelist only validated macro callees, never defaults factory bodies.
                                props_spans.push(call.callee.span());
                                if defaults.is_some() {
                                    props_spans.push(outer_call.callee.span());
                                }
                                let shape = prop_shape(
                                    &args.params[0],
                                    &parsed.program,
                                    path,
                                    resolver,
                                    &mut HashSet::new(),
                                )?;
                                let names: Vec<_> = shape.iter().map(|prop| &prop.name).collect();
                                let boolean_props: Vec<_> = shape
                                    .iter()
                                    .filter_map(|prop| {
                                        prop.boolean_cast.map(|cast| (&prop.name, cast))
                                    })
                                    .collect();
                                boolean_props_view = !boolean_props.is_empty();
                                let metadata = if boolean_props.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        ", {}",
                                        serde_json::to_string(&boolean_props)
                                            .map_err(|e| e.to_string())?
                                    )
                                };
                                let replacement = if let Some(defaults) = defaults {
                                    if variables.kind != VariableDeclarationKind::Const {
                                        return Err("native Vue withDefaults requires a const props declaration".into());
                                    }
                                    let defaults_span = outer_call.arguments[1].span();
                                    let keys = default_keys(defaults)?;
                                    for key in &keys {
                                        let prop = shape.iter().find(|prop| &prop.name == key).ok_or_else(|| format!("native Vue withDefaults unknown default key {key}"))?;
                                        if prop.duplicate {
                                            return Err("native Vue withDefaults duplicated prop declarations are unsupported".into());
                                        }
                                    }
                                    for node in setup_semantic.nodes().iter() {
                                        let AstKind::IdentifierReference(id) = node.kind() else {
                                            continue;
                                        };
                                        if id.span.start < defaults_span.start
                                            || id.span.end > defaults_span.end
                                        {
                                            continue;
                                        }
                                        let Some(symbol) =
                                            id.reference_id.get().and_then(|reference| {
                                                setup_semantic
                                                    .scoping()
                                                    .get_reference(reference)
                                                    .symbol_id()
                                            })
                                        else {
                                            continue;
                                        };
                                        let declaration =
                                            setup_semantic.symbol_declaration(symbol).span();
                                        if setup_value_symbols.contains(&symbol)
                                            && !(defaults_span.start <= declaration.start
                                                && declaration.end <= defaults_span.end)
                                        {
                                            return Err("native Vue withDefaults cannot capture setup values; use imports or factory parameters".into());
                                        }
                                    }
                                    let ty = props_type
                                        .as_ref()
                                        .ok_or("native Vue defaults require props type")?;
                                    default_hoists.push_str(&format!("const {defaults_name} = ({}) satisfies {defaults_type}<{ty}>;\n", &script[defaults_span.start as usize..defaults_span.end as usize]));
                                    let entries = keys.iter().map(|key| {
                                        let name = serde_json::to_string(key).map_err(|error| error.to_string())?;
                                        let sole = shape.iter().find(|prop| &prop.name == key).is_some_and(|prop| prop.sole_function);
                                        Ok(format!("[{name}, {defaults_name}[{name}], {}]", if sole { "false".to_owned() } else { format!("typeof {defaults_name}[{name}] === 'function'") }))
                                    }).collect::<Result<Vec<_>, String>>()?.join(",");
                                    let casts = serde_json::to_string(&boolean_props)
                                        .map_err(|error| error.to_string())?;
                                    default_props_view = true;
                                    let defaults_shape = format!("{{[{defaults_key} in keyof ({ty}) & keyof typeof {defaults_name}]: (typeof {defaults_name})[{defaults_key}]}}");
                                    format!("{props_view}<{ty}, {defaults_shape}>({props}, {}, {casts}, [{entries}]){}", serde_json::to_string(&names).map_err(|error| error.to_string())?, if matches!(target, MoonshineModuleTarget::Ssr) { "" } else { ".props" })
                                } else {
                                    format!(
                                        "{props_view}({props}, {}{metadata})",
                                        serde_json::to_string(&names)
                                            .map_err(|error| error.to_string())?
                                    )
                                };
                                edits.push((
                                    outer_call.span.start as usize,
                                    outer_call.span.end as usize,
                                    replacement,
                                ));
                            } else if ref_helpers.contains(callee.name.as_str()) {
                                let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                                    return Err(
                                        "native Vue ref declarations require identifiers".into()
                                    );
                                };
                                refs.insert(id.name.to_string());
                            }
                        }
                    }
                }
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    slots.constant_styles.insert(id.name.to_string());
                    if id.symbol_id.get().is_some_and(|symbol| {
                        !setup_semantic
                            .symbol_references(symbol)
                            .any(|reference| reference.is_write())
                    }) {
                        stable_handlers.insert(id.name.to_string());
                    }
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    slots.constant_styles.insert(id.name.to_string());
                }
            }
            Statement::TSTypeAliasDeclaration(_) | Statement::TSInterfaceDeclaration(_) => {
                let span = statement.span();
                for node in setup_semantic.nodes().iter() {
                    let AstKind::IdentifierReference(id) = node.kind() else {
                        continue;
                    };
                    if id.span.start < span.start || id.span.end > span.end {
                        continue;
                    }
                    let Some(symbol) = id.reference_id.get().and_then(|reference| {
                        setup_semantic
                            .scoping()
                            .get_reference(reference)
                            .symbol_id()
                    }) else {
                        continue;
                    };
                    let declaration = setup_semantic.symbol_declaration(symbol).span();
                    if setup_value_symbols.contains(&symbol) || !in_module_type_scope(declaration) {
                        return Err(
                            "native Vue local types cannot reference setup value declarations"
                                .into(),
                        );
                    }
                }
                imports.push_str(&script[span.start as usize..span.end as usize]);
                imports.push('\n');
                edits.push((span.start as usize, span.end as usize, String::new()));
            }
            Statement::ExportNamedDeclaration(_)
            | Statement::ExportDefaultDeclaration(_)
            | Statement::ExportAllDeclaration(_) => {
                return Err("native Vue setup exports unsupported".into());
            }
            _ => {}
        }
    }
    for node in setup_semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Some(id) = call.callee.get_identifier_reference() else {
            continue;
        };
        let lifecycle = id
            .reference_id
            .get()
            .and_then(|reference| {
                setup_semantic
                    .scoping()
                    .get_reference(reference)
                    .symbol_id()
            })
            .is_some_and(|symbol| lifecycle_helpers.contains(&symbol));
        if lifecycle
            && (call.arguments.len() != 1
                || matches!(call.arguments.first(), Some(Argument::SpreadElement(_))))
        {
            return Err("native Vue lifecycle hooks require one callback without an explicit target or spread".into());
        }
    }
    // Unresolved macros must be erased; lexical bindings remain ordinary values.
    struct Check<'s, 'a> {
        semantic: &'s oxc_semantic::Semantic<'a>,
        props_spans: &'s [oxc_span::Span],
        emits_spans: &'s [oxc_span::Span],
        function_depth: usize,
        error: Option<String>,
    }
    impl<'a> Visit<'a> for Check<'_, 'a> {
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            if id.reference_id.get().is_some_and(|reference| {
                self.semantic
                    .scoping()
                    .get_reference(reference)
                    .symbol_id()
                    .is_some()
            }) {
                return;
            }
            if id.name == "defineProps" {
                if !self
                    .props_spans
                    .iter()
                    .any(|span| span.start <= id.span.start && span.end >= id.span.end)
                {
                    self.error = Some(
                        "native Vue defineProps macro must be top-level typed declaration".into(),
                    );
                }
            } else if id.name == "defineEmits" {
                if !self
                    .emits_spans
                    .iter()
                    .any(|span| span.start <= id.span.start && span.end >= id.span.end)
                {
                    self.error = Some(
                        "unsupported native Vue defineEmits macro usage; requires a top-level typed const declaration"
                            .into(),
                    );
                }
            } else if id.name == "withDefaults"
                && self
                    .props_spans
                    .iter()
                    .any(|span| span.start <= id.span.start && span.end >= id.span.end)
            {
                // Validated finite wrapper is erased with its nested defineProps.
            } else if matches!(
                id.name.as_str(),
                "defineExpose" | "defineModel" | "defineOptions" | "defineSlots" | "withDefaults"
            ) {
                self.error = Some(format!("unsupported native Vue macro {}", id.name));
            }
        }
        fn visit_function(&mut self, node: &Function<'a>, flags: ScopeFlags) {
            self.function_depth += 1;
            walk::walk_function(self, node, flags);
            self.function_depth -= 1;
        }
        fn visit_arrow_function_expression(&mut self, node: &ArrowFunctionExpression<'a>) {
            self.function_depth += 1;
            walk::walk_arrow_function_expression(self, node);
            self.function_depth -= 1;
        }
        fn visit_for_of_statement(&mut self, node: &ForOfStatement<'a>) {
            if node.r#await && self.function_depth == 0 {
                self.error = Some("native Vue async setup unsupported".into());
            }
            walk::walk_for_of_statement(self, node);
        }
        fn visit_await_expression(&mut self, node: &AwaitExpression<'a>) {
            if self.function_depth == 0 {
                self.error = Some("native Vue async setup unsupported".into());
            }
            walk::walk_await_expression(self, node);
        }
    }
    let mut check = Check {
        semantic: &setup_semantic,
        props_spans: &props_spans,
        emits_spans: &emits_spans,
        function_depth: 0,
        error: None,
    };
    check.visit_program(&parsed.program);
    if let Some(error) = check.error {
        return Err(error);
    }
    let mut props_type = props_type.unwrap_or_else(|| "{}".to_owned());
    if let Some(ty) = emits_type {
        imports.push_str(&format!("import {{createEmitter as {emitter}, type EventListeners as {listeners}}} from '@tschk/moonshine-vue/events';\n"));
        let mut modifier_keys = std::collections::BTreeSet::new();
        for event in &emit_events {
            let Some(model) = event.strip_prefix("update:") else {
                continue;
            };
            let model = camelized_ascii_prop(model)
                .ok_or("native Vue update events require simple ASCII model names")?;
            if model == "modelValue" {
                modifier_keys.insert("modelModifiers".to_owned());
            }
            modifier_keys.insert(format!("{model}Modifiers"));
        }
        let modifiers = modifier_keys
            .iter()
            .map(|key| {
                serde_json::to_string(key)
                    .map(|key| format!("{key}?: Record<string, boolean | undefined>"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
            .join(";");
        props_type = if modifiers.is_empty() {
            format!("({props_type}) & {listeners}<{ty}>")
        } else {
            format!("({props_type}) & {{{modifiers}}} & {listeners}<{ty}>")
        };
    }
    let props_helper = if default_props_view {
        if matches!(target, MoonshineModuleTarget::Ssr) {
            "createServerDefaultProps"
        } else {
            "createDefaultProps"
        }
    } else if boolean_props_view {
        if matches!(target, MoonshineModuleTarget::Ssr) {
            "createServerBooleanProps"
        } else {
            "createBooleanProps"
        }
    } else {
        props_helper
    };
    imports.insert_str(
        props_import_offset,
        &format!(
            "import {{ {props_helper} as {props_view} }} from '@tschk/moonshine-vue/props';\n"
        ),
    );
    if default_props_view {
        imports.push_str(&format!("import {{ withVueScheduler as {defaults_scheduler}, type DefaultValues as {defaults_type} }} from '@tschk/moonshine-vue/props';\n"));
        imports.push_str(&default_hoists);
    }
    edits.sort_by_key(|edit| edit.0);
    let mut instance = script.to_owned();
    for (start, end, value) in edits.into_iter().rev() {
        instance.replace_range(start..end, &value)
    }
    let mut component_metadata = HashMap::new();
    for node in &mut nodes {
        resolve_component_events(
            node,
            &component_sources,
            &mut component_metadata,
            path,
            resolver,
        )?;
        validate_components(node, &components, &component_metadata)?;
    }
    slots.stable_handlers = stable_handlers;
    let jsx = nodes
        .iter()
        .map(|node| {
            emit(
                node,
                &refs,
                &unref,
                &text,
                &args,
                &handler,
                &mut slots,
                nodes.len() != 1,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("");
    if slots.normalization {
        let class_helper = if slots.server {
            "normalizeClassSSR"
        } else {
            "normalizeClass"
        };
        imports.push_str(&format!("import {{{class_helper} as {}, normalizeStyleDOM as {}, normalizeStaticStyle as {}, normalizeStyleSSR as {}}} from '@tschk/moonshine-vue/normalize';\n", slots.normalize_class, slots.normalize_style, slots.normalize_static_style, slots.normalize_vnode_style));
    }
    if !slots.declarations.is_empty() {
        imports.push_str(&format!("import {{withDefaultSlot as {}, {slot_helper} as {}}} from '@tschk/moonshine-vue/slots';\n", slots.transport, slots.outlet));
    }
    if slots.text_models {
        let helper = if slots.server {
            "createServerTextModel"
        } else {
            "createTextModel"
        };
        let module = if slots.server {
            "@tschk/moonshine-vue/model/ssr"
        } else {
            "@tschk/moonshine-vue/model"
        };
        imports.push_str(&format!(
            "import {{{helper} as {}}} from '{module}';\n",
            slots.model
        ));
    }
    if slots.checked_models {
        let (helper, module) = if slots.server {
            ("createServerCheckedModel", "@tschk/moonshine-vue/model/ssr")
        } else {
            ("createCheckedModel", "@tschk/moonshine-vue/model")
        };
        imports.push_str(&format!(
            "import {{{helper} as {}}} from '{module}';\n",
            slots.checked_model
        ));
    }
    if slots.select_models {
        if slots.server {
            imports.push_str(&format!("import {{createServerModelOption as {}}} from '@tschk/moonshine-vue/select/ssr';\n", slots.option_model));
        } else {
            imports.push_str(&format!("import {{createSelectModel as {}, createModelOption as {}, captureSelectScope as {}}} from '@tschk/moonshine-vue/select';\n", slots.select_model, slots.option_model, slots.capture_select_scope));
        }
    }
    let select_scheduler = slots.select_models && !slots.server;
    if select_scheduler && !default_props_view {
        imports.push_str(&format!("import {{withVueScheduler as {defaults_scheduler}}} from '@tschk/moonshine-vue/props';\n"));
    }
    let select_scope_declaration = if select_scheduler {
        format!(
            "const {} = {}();",
            slots.select_scope, slots.capture_select_scope
        )
    } else {
        String::new()
    };
    let model_declarations = slots.models.join("\n");
    let slot_declarations = slots.declarations.join("\n");
    let body = format!("{instance}\n{select_scope_declaration}\n{model_declarations}\n{slot_declarations}\nreturn <>{jsx}</>;");
    let body = if default_props_view || select_scheduler {
        format!("return {defaults_scheduler}(() => {{\n{body}\n}});")
    } else {
        body
    };
    let synthetic =
        format!("{imports}\nexport default function {name}({props}: {props_type}){{\n{body}\n}}");
    let mut compiled =
        crate::compile_moonshine_module(&synthetic, &path.with_extension("vue.tsx"), target)?;
    compiled.styles = styles;
    Ok(compiled)
}
#[derive(Default)]
struct VueNames(HashSet<String>);

impl<'a> Visit<'a> for VueNames {
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        self.0.insert(id.name.to_string());
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.0.insert(id.name.to_string());
    }
}

impl VueNames {
    fn expression(&mut self, source: &str) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
        // Emission owns validation; this pass only reserves names conservatively.
        self.visit_program(&parsed.program);
    }

    fn template(&mut self, node: &Node) {
        match node {
            Node::Text(parts) => {
                for part in parts {
                    if let TextPart::Expr(value) = part {
                        self.expression(value);
                    }
                }
            }
            Node::Element(element) => {
                for binding in &element.bindings {
                    self.expression(&binding.value);
                }
                for event in &element.event_handlers {
                    self.expression(&event.handler);
                }
                for child in &element.children {
                    self.template(child);
                }
            }
            _ => {}
        }
    }
}

fn expression(source: &str, refs: &HashSet<String>, unref: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("invalid Vue expression {source:?}"));
    }
    struct Rewrite<'s, 'a> {
        refs: &'s HashSet<String>,
        unref: &'s str,
        semantic: &'s oxc_semantic::Semantic<'a>,
        edits: Vec<(usize, usize, String)>,
    }
    impl<'a> Visit<'a> for Rewrite<'_, 'a> {
        fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
            if property.shorthand {
                let before = self.edits.len();
                self.visit_expression(&property.value);
                for (_, _, replacement) in &mut self.edits[before..] {
                    if let Expression::Identifier(id) = &property.value {
                        // A non-computed __proto__ key would set the object's prototype.
                        let key = if id.name == "__proto__" {
                            "[\"__proto__\"]"
                        } else {
                            id.name.as_str()
                        };
                        *replacement = format!("{key}: {replacement}");
                    }
                }
                return;
            }
            walk::walk_object_property(self, property);
        }
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            if self.refs.contains(id.name.as_str())
                && id.reference_id.get().is_some_and(|reference| {
                    self.semantic
                        .scoping()
                        .get_reference(reference)
                        .symbol_id()
                        .is_none()
                })
            {
                self.edits.push((
                    id.span.start as usize,
                    id.span.end as usize,
                    format!("{}({})", self.unref, id.name),
                ))
            }
        }
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut visitor = Rewrite {
        refs,
        unref,
        semantic: &semantic,
        edits: vec![],
    };
    visitor.visit_program(&parsed.program);
    let mut result = source.to_owned();
    visitor.edits.sort_by_key(|e| e.0);
    for (start, end, value) in visitor.edits.into_iter().rev() {
        result.replace_range(start..end, &value)
    }
    Ok(result)
}
#[derive(Default)]
struct ComponentEvents {
    names: HashSet<String>,
    props: HashSet<String>,
    boolean_casts: HashMap<String, bool>,
}

fn component_events(
    module: &VueTypeModule,
    resolver: &mut VueTypeResolver<'_>,
) -> Result<ComponentEvents, String> {
    if !module
        .path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vue"))
    {
        return Err("native Vue component listeners require a resolved Vue SFC child".into());
    }
    let sfc = crepuscularity_core::parser::parse_vue_sfc(&module.source)
        .map_err(|error| error.to_string())?;
    let mut result = ComponentEvents::default();
    for script in &sfc.scripts {
        if !script.setup {
            continue;
        }
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &script.content, SourceType::tsx()).parse();
        if !parsed.errors.is_empty() {
            return Err("native Vue child setup metadata parse failed".into());
        }
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let mut found = false;
        for statement in &parsed.program.body {
            let Statement::VariableDeclaration(declarations) = statement else {
                continue;
            };
            for declaration in &declarations.declarations {
                let Some(Expression::CallExpression(call)) = declaration
                    .init
                    .as_ref()
                    .map(Expression::get_inner_expression)
                else {
                    continue;
                };
                let (call, defaults) = props_macro_call(call, &semantic)?;
                if let Some(defaults) = defaults {
                    default_keys(defaults)?;
                }
                let Some(callee) = call.callee.get_identifier_reference() else {
                    continue;
                };
                if callee
                    .reference_id
                    .get()
                    .is_some_and(|id| semantic.scoping().get_reference(id).symbol_id().is_some())
                {
                    continue;
                }
                if !matches!(callee.name.as_str(), "defineProps" | "defineEmits") {
                    continue;
                }
                let args = call
                    .type_arguments
                    .as_ref()
                    .ok_or("native Vue child metadata requires typed compiler macros")?;
                if args.params.len() != 1 || !call.arguments.is_empty() {
                    return Err(
                        "native Vue child metadata requires one typed compiler macro argument"
                            .into(),
                    );
                }
                if callee.name == "defineEmits" {
                    if found || declarations.kind != VariableDeclarationKind::Const {
                        return Err(
                            "native Vue child emits metadata requires one typed const declaration"
                                .into(),
                        );
                    }
                    found = true;
                    result.names = event_shape(&args.params[0])?;
                } else {
                    let shape = prop_shape(
                        &args.params[0],
                        &parsed.program,
                        &module.path,
                        resolver,
                        &mut HashSet::new(),
                    )?;
                    for prop in shape {
                        if let Some(cast) = prop.boolean_cast {
                            result.boolean_casts.insert(prop.name.clone(), cast);
                        }
                        result.props.insert(prop.name);
                    }
                }
            }
        }
    }
    Ok(result)
}

fn hyphenated_ascii_prop(name: &str) -> Option<String> {
    if !name.is_ascii() {
        return None;
    }
    let mut result = String::new();
    let mut previous_word = false;
    for ch in name.chars() {
        if previous_word && ch.is_ascii_uppercase() {
            result.push('-');
        }
        result.push(ch.to_ascii_lowercase());
        previous_word = ch.is_ascii_alphanumeric() || ch == '_';
    }
    Some(result)
}

fn camelized_ascii_prop(name: &str) -> Option<String> {
    if !name.is_ascii() {
        return None;
    }
    let mut result = String::new();
    let mut chars = name.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '-'
            && chars
                .peek()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        {
            if let Some(next) = chars.next() {
                result.push(next.to_ascii_uppercase());
            }
        } else {
            result.push(ch);
        }
    }
    Some(result)
}

fn possible_boolean_literal(binding: &crepuscularity_core::ast::Binding) -> bool {
    let allocator = Allocator::default();
    let Ok(value) = Parser::new(&allocator, &binding.value, SourceType::tsx()).parse_expression()
    else {
        return false;
    };
    let Expression::StringLiteral(value) = value.get_inner_expression() else {
        return false;
    };
    if value.value.is_empty() {
        return true;
    }
    hyphenated_ascii_prop(&binding.prop).is_none_or(|name| value.value == name)
}

fn resolve_component_events(
    node: &Node,
    sources: &HashMap<String, String>,
    metadata: &mut HashMap<String, ComponentEvents>,
    path: &Path,
    resolver: &mut VueTypeResolver<'_>,
) -> Result<(), String> {
    if let Node::Element(element) = node {
        let component = element.tag.chars().next().is_some_and(char::is_uppercase);
        if component
            && (!element.event_handlers.is_empty()
                || element.bindings.iter().any(|binding| {
                    is_model_binding(&binding.prop)
                        || binding.prop.contains('-')
                        || (sources.contains_key(&element.tag) && possible_boolean_literal(binding))
                        || matches!(binding.prop.as_str(), "id" | "class" | "style")
                })
                || element.id.is_some())
        {
            if !metadata.contains_key(&element.tag) {
                let source = sources.get(&element.tag).ok_or(
                    "native Vue component event/id metadata requires a default Vue SFC import",
                )?;
                let module = resolver(path, source).map_err(|error| {
                    format!("unsupported native Vue component event/id metadata: {error}")
                })?;
                metadata.insert(element.tag.clone(), component_events(&module, resolver)?);
            }
            let events = metadata
                .get(&element.tag)
                .ok_or("native Vue child metadata is missing")?;
            for binding in element
                .bindings
                .iter()
                .filter(|binding| is_model_binding(&binding.prop))
            {
                let name = component_model_name(&binding.prop)?;
                let listener = event_name(&format!("update:{name}"))?;
                if !events.props.contains(name)
                    || !events
                        .names
                        .iter()
                        .any(|event| event_name(event).as_ref() == Ok(&listener))
                {
                    return Err(format!("native Vue component models require declared {name} props and update:{name} emits"));
                }
            }
            for event in &element.event_handlers {
                if !events.names.contains(&event.event)
                    && !(event.event.starts_with("update:")
                        && events
                            .names
                            .iter()
                            .any(|declared| event_name(declared) == event_name(&event.event)))
                {
                    return Err(format!("native Vue component event {} must be declared by child defineEmits; DOM listener fallthrough is unsupported", event.event));
                }
            }
        }
        for child in &element.children {
            resolve_component_events(child, sources, metadata, path, resolver)?;
        }
    }
    Ok(())
}

fn validate_components(
    node: &mut Node,
    components: &HashSet<String>,
    metadata: &HashMap<String, ComponentEvents>,
) -> Result<(), String> {
    if let Node::Element(element) = node {
        let pascal_case = element.tag.chars().next().is_some_and(char::is_uppercase);
        if element.tag.contains('.') || (components.contains(&element.tag) && !pascal_case) {
            return Err(
                "unsupported native Vue component tag: use an explicit PascalCase value import"
                    .into(),
            );
        }
        if pascal_case {
            if !components.contains(&element.tag) {
                return Err("unsupported native Vue component/form/directive: component tags require an explicit value import".into());
            }
            let declared = |name: &str| {
                metadata
                    .get(&element.tag)
                    .is_some_and(|facts| facts.props.contains(name))
            };
            let aliases: Vec<_> = element
                .bindings
                .iter()
                .filter_map(|binding| {
                    if is_model_binding(&binding.prop) || !binding.prop.contains('-') {
                        return None;
                    }
                    let normalized = camelized_ascii_prop(&binding.prop)?;
                    let canonical = metadata
                        .get(&element.tag)?
                        .props
                        .iter()
                        .find(|name| **name == normalized)?;
                    Some((binding.prop.clone(), canonical.clone()))
                })
                .collect();
            for (alias, canonical) in &aliases {
                if element
                    .bindings
                    .iter()
                    .any(|binding| binding.prop == *canonical)
                {
                    return Err("native Vue duplicate canonical and hyphenated component props are unsupported".into());
                }
                if aliases.iter().filter(|(_, name)| name == canonical).count() > 1 {
                    return Err(
                        "native Vue colliding hyphenated component props are unsupported".into(),
                    );
                }
                for binding in &mut element.bindings {
                    if binding.prop == *alias {
                        binding.prop.clone_from(canonical);
                    }
                }
            }
            if let Some(facts) = metadata.get(&element.tag) {
                for binding in &mut element.bindings {
                    if !facts
                        .boolean_casts
                        .get(&binding.prop)
                        .copied()
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let allocator = Allocator::default();
                    let value = Parser::new(&allocator, &binding.value, SourceType::tsx())
                        .parse_expression()
                        .map_err(|_| "native Vue literal prop value parse failed")?;
                    if let Expression::StringLiteral(value) = value.get_inner_expression() {
                        let matches = if value.value.is_empty() {
                            true
                        } else {
                            let name = hyphenated_ascii_prop(&binding.prop).ok_or(
                                "native Vue non-ASCII Boolean prop name literals are unsupported",
                            )?;
                            value.value == name
                        };
                        if matches {
                            binding.value = "true".into();
                        }
                    }
                }
            }
            if (element.id.is_some() && !declared("id"))
                || !element.classes.is_empty()
                || element.bindings.iter().any(|binding| {
                    (!is_model_binding(&binding.prop) && binding.prop.contains('-'))
                        || (matches!(binding.prop.as_str(), "id" | "class" | "style")
                            && !declared(&binding.prop))
                        || matches!(
                            binding.prop.as_str(),
                            "children" | "ref" | "key" | "is" | "innerHTML" | "textContent"
                        )
                })
            {
                return Err(
                    "unsupported native Vue component slots/events/fallthrough or reserved props"
                        .into(),
                );
            }
        }
        for child in &mut element.children {
            validate_components(child, components, metadata)?;
        }
    }
    Ok(())
}

// Match Vue's setup-const vs dynamic style normalization using parsed Oxc expressions.
fn style_static(node: &Expression<'_>) -> bool {
    match node.get_inner_expression() {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(node) => style_static(&node.argument),
        Expression::BinaryExpression(node) => style_static(&node.left) && style_static(&node.right),
        Expression::LogicalExpression(node) => {
            style_static(&node.left) && style_static(&node.right)
        }
        Expression::ConditionalExpression(node) => {
            style_static(&node.test)
                && style_static(&node.consequent)
                && style_static(&node.alternate)
        }
        Expression::SequenceExpression(node) => node.expressions.iter().all(style_static),
        Expression::TemplateLiteral(node) => node.expressions.iter().all(style_static),
        _ => false,
    }
}
fn style_setup_const(node: &Expression<'_>) -> bool {
    match node.get_inner_expression() {
        Expression::UnaryExpression(_)
        | Expression::BinaryExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::UpdateExpression(_)
        | Expression::ClassExpression(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::SequenceExpression(node) => {
            node.expressions.last().is_some_and(style_setup_const)
        }
        node => style_static(node),
    }
}
fn style_expression_const(source: &str, bindings: &HashSet<String>) -> Result<bool, String> {
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, source, SourceType::tsx())
        .parse_expression()
        .map_err(|errors| format!("Vue style expression parse failed: {errors:?}"))?;
    Ok(match expression.get_inner_expression() {
        Expression::Identifier(id) => bindings.contains(id.name.as_str()),
        expression => style_static(expression),
    })
}

fn literal_string_binding(binding: &crepuscularity_core::ast::Binding) -> Result<String, String> {
    let allocator = Allocator::default();
    let value = Parser::new(&allocator, &binding.value, SourceType::tsx())
        .parse_expression()
        .map_err(|_| "invalid native Vue input type")?;
    if let Expression::StringLiteral(value) = value.get_inner_expression() {
        Ok(value.value.to_string())
    } else {
        Err("native Vue text models require a static input type".into())
    }
}

fn model_bindings(
    program: &Program<'_>,
    semantic: &oxc_semantic::Semantic<'_>,
) -> HashMap<String, bool> {
    let mut helpers = HashMap::new();
    for statement in &program.body {
        if let Statement::ImportDeclaration(import) = statement {
            if import.source.value != "vue" || import.import_kind == ImportOrExportKind::Type {
                continue;
            }
            for spec in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(spec) = spec {
                    if spec.import_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let helper = spec.imported.name();
                    if matches!(helper.as_str(), "ref" | "shallowRef" | "reactive") {
                        if let Some(symbol) = spec.local.symbol_id.get() {
                            helpers.insert(symbol, helper != "reactive");
                        }
                    }
                }
            }
        }
    }
    let mut bindings = HashMap::new();
    for statement in &program.body {
        let Statement::VariableDeclaration(declarations) = statement else {
            continue;
        };
        if declarations.kind != VariableDeclarationKind::Const {
            continue;
        }
        for declaration in &declarations.declarations {
            let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                continue;
            };
            let Some(Expression::CallExpression(call)) = declaration
                .init
                .as_ref()
                .map(Expression::get_inner_expression)
            else {
                continue;
            };
            let Some(callee) = call.callee.get_identifier_reference() else {
                continue;
            };
            if let Some(kind) = callee
                .reference_id
                .get()
                .and_then(|id| semantic.scoping().get_reference(id).symbol_id())
                .and_then(|symbol| helpers.get(&symbol))
            {
                bindings.insert(id.name.to_string(), *kind);
            }
        }
    }
    bindings
}

fn model_target(
    source: &str,
    bindings: &HashMap<String, bool>,
    refs: &HashSet<String>,
    unref: &str,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx())
        .parse_expression()
        .map_err(|_| "native Vue model requires a writable setup target")?;
    fn root<'a>(value: &'a Expression<'a>) -> Option<&'a str> {
        match value.get_inner_expression() {
            Expression::Identifier(id) => Some(id.name.as_str()),
            Expression::StaticMemberExpression(member) if !member.optional => root(&member.object),
            Expression::ComputedMemberExpression(member) if !member.optional => {
                root(&member.object)
            }
            _ => None,
        }
    }
    let name = root(&parsed).ok_or("native Vue model requires a writable setup target")?;
    if matches!(parsed.get_inner_expression(), Expression::Identifier(_)) {
        if bindings.get(name) != Some(&true) {
            return Err("native Vue model requires a writable setup ref".into());
        }
        return Ok(format!("{name}.value"));
    }
    if !bindings.contains_key(name) {
        return Err("native Vue model requires a writable ref or reactive object".into());
    }
    expression(source, refs, unref)
}

struct SlotEmit {
    source: String,
    transport: String,
    outlet: String,
    prefix: String,
    declarations: Vec<String>,
    normalize_class: String,
    normalize_style: String,
    normalize_static_style: String,
    normalization: bool,
    constant_styles: HashSet<String>,
    normalize_vnode_style: String,
    stable_handlers: HashSet<String>,
    server: bool,
    model: String,
    checked_model: String,
    checked_models: bool,
    text_models: bool,
    select_model: String,
    select_scope: String,
    capture_select_scope: String,
    option_model: String,
    select_models: bool,
    selections: Vec<String>,
    models: Vec<String>,
    model_node: String,
    model_bindings: HashMap<String, bool>,
}

fn literal_text(value: &str) -> Result<String, String> {
    let decoded = crate::svelte_entities::decode_static_dom(value);
    // shortcut: reject condensed/edge whitespace until Vue whitespace ownership is implemented.
    if decoded.contains("  ")
        || decoded.contains(['\n', '\r', '\t', '\u{c}'])
        || (decoded != value
            && (decoded.starts_with(|ch: char| ch.is_ascii_whitespace())
                || decoded.ends_with(|ch: char| ch.is_ascii_whitespace())))
    {
        return Err(
            "native Vue condensed or entity-boundary text whitespace is unsupported".into(),
        );
    }
    Ok(decoded)
}

#[allow(clippy::too_many_arguments)]
fn emit(
    node: &Node,
    refs: &HashSet<String>,
    unref: &str,
    text: &str,
    args: &str,
    handler: &str,
    slots: &mut SlotEmit,
    nested: bool,
) -> Result<String, String> {
    match node {
        Node::Text(parts) => parts
            .iter()
            .map(|part| {
                Ok(match part {
                    TextPart::Literal(value) => format!(
                        "{{{}}}",
                        serde_json::to_string(&literal_text(value)?).map_err(|e| e.to_string())?
                    ),
                    TextPart::Expr(value) => {
                        format!("{{{text}({})}}", expression(value, refs, unref)?)
                    }
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|parts| parts.join("")),
        Node::Element(element) => {
            let boundary_space = |node: Option<&Node>, first: bool| matches!(node, Some(Node::Text(parts)) if matches!(if first {parts.first()} else {parts.last()}, Some(TextPart::Literal(value)) if if first {value.starts_with(' ')} else {value.ends_with(' ')}));
            if boundary_space(element.children.first(), true)
                || boundary_space(element.children.last(), false)
            {
                return Err("native Vue boundary text whitespace is unsupported".into());
            }
            if element
                .bindings
                .iter()
                .any(|binding| binding.value.contains('&'))
                || element.id.as_ref().is_some_and(|id| id.contains('&'))
                || element.classes.iter().any(|class| class.contains('&'))
            {
                return Err("native Vue attribute entities are unsupported".into());
            }
            if matches!(element.tag.as_str(), "component" | "template")
                || !element.conditional_classes.is_empty()
                || !element.animations.is_empty()
            {
                return Err("unsupported native Vue component/form/directive".into());
            }
            if !slots.selections.is_empty()
                && !matches!(element.tag.as_str(), "option" | "optgroup")
            {
                return Err(
                    "native Vue select models require inline option/optgroup children".into(),
                );
            }
            let models: Vec<_> = element
                .bindings
                .iter()
                .filter(|binding| is_model_binding(&binding.prop))
                .collect();
            let mut model_tag = element.tag.clone();
            let mut component_models = Vec::new();
            if matches!(element.tag.as_str(), "input" | "textarea") {
                if models
                    .iter()
                    .any(|binding| binding.prop.starts_with("bind:"))
                {
                    return Err("named models require a component".into());
                }

                let input_type = element
                    .bindings
                    .iter()
                    .find(|binding| binding.prop == "type")
                    .map(literal_string_binding)
                    .transpose()?;
                let checked = element.tag == "input"
                    && matches!(input_type.as_deref(), Some("checkbox" | "radio"));
                if models.len() != 1
                    || !element.children.is_empty()
                    || element.bindings.iter().any(|binding| {
                        matches!(binding.prop.as_str(), "defaultValue" | "ref")
                            || (!checked && binding.prop == "value")
                    })
                {
                    return Err("native Vue text models require exactly one model without value/ref/children conflicts".into());
                }
                if let Some(kind) = &input_type {
                    if element.tag != "input"
                        || !matches!(
                            kind.as_str(),
                            "text"
                                | "search"
                                | "tel"
                                | "url"
                                | "email"
                                | "password"
                                | "number"
                                | "range"
                                | "checkbox"
                                | "radio"
                        )
                    {
                        return Err(
                            "native Vue text models require a supported static input type".into(),
                        );
                    }
                }
                if checked && models[0].prop != "bind" {
                    return Err("native Vue checked model modifiers unsupported".into());
                }
                let binding = models[0];
                let write = model_target(&binding.value, &slots.model_bindings, refs, unref)?;
                let read = expression(&binding.value, refs, unref)?;
                model_tag = format!("{}{}", slots.model_node, slots.models.len());
                let tag = serde_json::to_string(if checked {
                    input_type.as_ref().unwrap()
                } else {
                    &element.tag
                })
                .map_err(|error| error.to_string())?;
                let modifiers = binding
                    .prop
                    .split('.')
                    .skip(1)
                    .map(|modifier| format!("{modifier}:true"))
                    .collect::<Vec<_>>()
                    .join(",");
                let arguments = if slots.server && checked {
                    format!("{tag}, {handler}, () => {read}, {nested}")
                } else if slots.server {
                    format!("{tag}, {handler}, () => {read}")
                } else if checked {
                    format!(
                        "{tag}, {handler}, () => {read}, ({args}: any) => {{ {write} = {args}; }}"
                    )
                } else {
                    format!("{tag}, {handler}, () => {read}, ({args}: any) => {{ {write} = {args}; }}, {{{modifiers}}}")
                };
                let helper = if checked {
                    slots.checked_models = true;
                    &slots.checked_model
                } else {
                    slots.text_models = true;
                    &slots.model
                };
                slots.models.push(format!(
                    "const {model_tag} = ({handler}: Record<string, unknown>) => {}({arguments});",
                    helper
                ));
            } else if element.tag == "select" {
                if !slots.selections.is_empty()
                    || models.len() != 1
                    || !matches!(models[0].prop.as_str(), "bind" | "bind.number")
                    || element.bindings.iter().any(|binding| {
                        matches!(binding.prop.as_str(), "value" | "defaultValue" | "ref")
                    })
                {
                    return Err("native Vue select models require one writable model and optional number modifier without value/ref conflicts".into());
                }
                let binding = models[0];
                let write = model_target(&binding.value, &slots.model_bindings, refs, unref)?;
                let read = expression(&binding.value, refs, unref)?;
                slots.selections.push(read.clone());
                slots.select_models = true;
                if !slots.server {
                    model_tag = format!("{}{}", slots.model_node, slots.models.len());
                    let number = binding.prop == "bind.number";
                    slots.models.push(format!("const {model_tag} = ({handler}: any) => {}(() => {}({handler}, () => {read}, ({args}: any) => {{ {write} = {args}; }}, {handler}.children, {number}));", slots.select_scope, slots.select_model));
                }
            } else if !models.is_empty()
                && element.tag.chars().next().is_some_and(char::is_uppercase)
            {
                let mut names = HashSet::new();
                let mut modifier_keys = HashSet::new();
                for binding in &models {
                    let name = component_model_name(&binding.prop)?;
                    let listener = event_name(&format!("update:{name}"))?;
                    let modifiers = component_model_modifiers(&binding.prop)?;
                    if !names.insert(name)
                        || modifier_keys.contains(name)
                        || matches!(
                            name,
                            "children"
                                | "ref"
                                | "key"
                                | "is"
                                | "innerHTML"
                                | "textContent"
                                | "__proto__"
                        )
                        || element
                            .bindings
                            .iter()
                            .any(|binding| binding.prop == name || binding.prop == listener)
                        || modifiers.as_ref().is_some_and(|(key, _)| {
                            names.contains(key.as_str())
                                || !modifier_keys.insert(key.clone())
                                || element.bindings.iter().any(|binding| binding.prop == *key)
                        })
                    {
                        return Err("native Vue component models require distinct model names without explicit model/listener/modifier prop conflicts".into());
                    }
                    let write = model_target(&binding.value, &slots.model_bindings, refs, unref)?;
                    let read = expression(&binding.value, refs, unref)?;
                    let modifiers = match modifiers {
                        Some((key, flags)) => {
                            let mut object = String::from("{");
                            for (index, flag) in flags.iter().enumerate() {
                                if index != 0 {
                                    object.push(',');
                                }
                                object.push_str(
                                    &serde_json::to_string(flag).map_err(|e| e.to_string())?,
                                );
                                object.push_str(":true");
                            }
                            object.push('}');
                            let constant = format!("{}{}", slots.model_node, slots.models.len());
                            slots.models.push(format!("const {constant} = {object};"));
                            Some((key, constant))
                        }
                        None => None,
                    };
                    component_models.push((name.to_owned(), read, write, modifiers));
                }
            } else if !models.is_empty() {
                return Err("native Vue component/non-text models unsupported".into());
            }
            if element.tag == "option" && !slots.selections.is_empty() {
                model_tag = format!("{}{}", slots.model_node, slots.models.len());
                let read = slots.selections.last().unwrap();
                let arguments = if slots.server {
                    format!("{handler}, {handler}.children, () => {read}")
                } else {
                    format!("{handler}, {handler}.children")
                };
                let call = format!("{}({arguments})", slots.option_model);
                let call = if slots.server {
                    call
                } else {
                    format!("{}(() => {call})", slots.select_scope)
                };
                slots
                    .models
                    .push(format!("const {model_tag} = ({handler}: any) => {call};"));
            }
            if element.tag == "slot" {
                if !element.bindings.is_empty()
                    || !element.event_handlers.is_empty()
                    || element.id.is_some()
                    || !element.classes.is_empty()
                {
                    return Err(
                        "native Vue only supports implicit default slot outlets without props"
                            .into(),
                    );
                }
                let children = element
                    .children
                    .iter()
                    .map(|child| emit(child, refs, unref, text, args, handler, slots, true))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("");
                let name = format!("{}{}", slots.prefix, slots.declarations.len());
                slots.declarations.push(format!(
                    "const {name} = {}({});",
                    slots.outlet, slots.source
                ));
                return Ok(format!("{{{name}(() => <>{children}</>)}}"));
            }
            let mut attrs = String::new();
            if let Some(id) = &element.id {
                attrs.push_str(&format!(
                    " id={{{}}}",
                    serde_json::to_string(id).map_err(|e| e.to_string())?
                ))
            }
            if !element.classes.is_empty() {
                attrs.push_str(&format!(
                    " class={{{}}}",
                    serde_json::to_string(&element.classes.join(" ")).map_err(|e| e.to_string())?
                ))
            }
            for binding in &element.bindings {
                if is_model_binding(&binding.prop) {
                    continue;
                }
                if matches!(
                    binding.prop.as_str(),
                    "ref" | "key" | "is" | "innerHTML" | "textContent"
                ) {
                    return Err("native Vue model/ref/key/content bindings unsupported".into());
                }
                if matches!(binding.prop.as_str(), "class" | "style") {
                    let component = element.tag.chars().next().is_some_and(char::is_uppercase);
                    if !component {
                        if binding.prop == "class"
                            && matches!(
                                binding.origin,
                                crepuscularity_core::ast::BindingOrigin::Literal
                            )
                        {
                            attrs.push_str(&format!(" class={{{}}}", binding.value));
                            continue;
                        }
                        return Err(
                            "native Vue intrinsic dynamic class/style bindings are unsupported"
                                .into(),
                        );
                    }
                    if binding.prop == "class"
                        && matches!(
                            binding.origin,
                            crepuscularity_core::ast::BindingOrigin::Literal
                        )
                    {
                        attrs.push_str(&format!(" class={{{}}}", binding.value));
                        continue;
                    }
                    slots.normalization = true;
                    let helper = if binding.prop == "class" {
                        &slots.normalize_class
                    } else if matches!(
                        binding.origin,
                        crepuscularity_core::ast::BindingOrigin::Literal
                    ) {
                        &slots.normalize_static_style
                    } else if slots.server
                        || style_expression_const(&binding.value, &slots.constant_styles)?
                    {
                        &slots.normalize_vnode_style
                    } else {
                        &slots.normalize_style
                    };
                    attrs.push_str(&format!(
                        " {}={{{helper}({})}}",
                        binding.prop,
                        expression(&binding.value, refs, unref)?
                    ));
                } else {
                    attrs.push_str(&format!(
                        " {}={{{}}}",
                        binding.prop,
                        expression(&binding.value, refs, unref)?
                    ));
                }
            }
            let component = element.tag.chars().next().is_some_and(char::is_uppercase);
            let mut component_events: Vec<(String, Vec<String>)> = Vec::new();
            let mut model_listeners = component_models
                .into_iter()
                .map(|(name, read, write, modifiers)| {
                    let read = if matches!(name.as_str(), "class" | "style") {
                        slots.normalization = true;
                        let helper = if name == "class" {
                            &slots.normalize_class
                        } else if slots.server {
                            &slots.normalize_vnode_style
                        } else {
                            &slots.normalize_style
                        };
                        format!("{helper}({read})")
                    } else {
                        read
                    };
                    attrs.push_str(&format!(" {name}={{{read}}}"));
                    if let Some((key, constant)) = modifiers {
                        attrs.push_str(&format!(" {key}={{{constant}}}"));
                    }
                    (
                        format!("onUpdate:{name}"),
                        format!("({args}: any) => {{ {write} = {args}; }}"),
                    )
                })
                .collect::<Vec<_>>();
            for event in &element.event_handlers {
                if !slots.stable_handlers.contains(event.handler.trim()) {
                    return Err("native Vue events require stable setup function declarations, const functions, or value imports".into());
                }
                let allocator = Allocator::default();
                let parsed_handler =
                    Parser::new(&allocator, &event.handler, SourceType::tsx()).parse();
                if !parsed_handler.errors.is_empty()
                    || !matches!(&parsed_handler.program.body[..],[Statement::ExpressionStatement(statement)] if matches!(statement.expression,Expression::Identifier(_)))
                {
                    return Err("native Vue events currently require handler identifiers".into());
                }
                if component {
                    if event.modifiers.iter().any(|modifier| modifier != "once")
                        || event.modifiers.len() > 1
                    {
                        return Err(
                            "native Vue component events support only the once modifier".into()
                        );
                    }
                    let mut name = event_name(&event.event)?;
                    if !slots.server && !event.modifiers.is_empty() {
                        name.push_str("Once");
                    }
                    if element.bindings.iter().any(|binding| {
                        binding.prop == name
                            || (is_model_binding(&binding.prop)
                                && (component_model_name(&binding.prop)
                                    .is_ok_and(|prop| prop == name)
                                    || component_model_modifiers(&binding.prop)
                                        .ok()
                                        .flatten()
                                        .is_some_and(|(key, _)| key == name)))
                    }) {
                        return Err(
                            "native Vue component event listener collides with a bound prop".into(),
                        );
                    }
                    if event.model_before_listener != Some(false) {
                        if let Some(index) =
                            model_listeners.iter().position(|(key, _)| key == &name)
                        {
                            let (_, listener) = model_listeners.remove(index);
                            if let Some((_, callbacks)) =
                                component_events.iter_mut().find(|(key, _)| key == &name)
                            {
                                callbacks.push(listener);
                            } else {
                                component_events.push((name.clone(), vec![listener]));
                            }
                        }
                    }
                    let value = event.handler.trim().to_owned();
                    if let Some((_, callbacks)) =
                        component_events.iter_mut().find(|(key, _)| key == &name)
                    {
                        callbacks.push(value);
                    } else {
                        component_events.push((name, vec![value]));
                    }
                    continue;
                }
                if !event.modifiers.is_empty() {
                    return Err("native Vue event modifiers unsupported".into());
                }
                let mut letters = event.event.chars();
                let Some(first) = letters.next() else {
                    return Err("native Vue event requires a name".into());
                };
                if !event.event.chars().all(|ch| ch.is_ascii_lowercase()) {
                    return Err("native Vue custom event names unsupported".into());
                }
                attrs.push_str(&format!(
                    " on{}{}={{(...{args}: any[]) => {{ const {handler}: any = {}; return {handler}(...{args}); }}}}",
                    first.to_ascii_uppercase(),
                    letters.as_str(),
                    expression(&event.handler, refs, unref)?
                ));
            }
            for (name, listener) in model_listeners {
                if let Some((_, callbacks)) =
                    component_events.iter_mut().find(|(key, _)| key == &name)
                {
                    callbacks.push(listener);
                } else {
                    component_events.push((name, vec![listener]));
                }
            }
            for (key, callbacks) in component_events {
                let value = if callbacks.len() == 1 {
                    callbacks[0].clone()
                } else {
                    format!("[{}]", callbacks.join(","))
                };
                attrs.push_str(&format!(" {key}={{{value}}}"));
            }
            let children = element
                .children
                .iter()
                .map(|child| emit(child, refs, unref, text, args, handler, slots, true))
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            if element.tag == "select" {
                slots.selections.pop();
            }
            if element.tag.chars().next().is_some_and(char::is_uppercase)
                && !element.children.is_empty()
            {
                let wrapper = format!("{}{}", slots.prefix, slots.declarations.len());
                slots.declarations.push(format!(
                    "const {wrapper} = {}({}, () => <>{children}</>);",
                    slots.transport, element.tag
                ));
                return Ok(format!("<{wrapper}{attrs}></{wrapper}>"));
            }
            Ok(format!("<{}{attrs}>{children}</{}>", model_tag, model_tag))
        }
        _ => Err("unsupported native Vue template control/content".into()),
    }
}
