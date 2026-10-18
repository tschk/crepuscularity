//! Authored synchronous Astro factories; markup parsing stays in core.
use crate::{compile_moonshine_module, MoonshineModule, MoonshineModuleTarget};
use crepuscularity_core::{
    ast::{Binding, BindingOrigin, Element, Node, TextPart},
    parser::{
        parse_astro_module_template_with_expression_validator, split_astro_source, svelte_entities,
    },
};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, Semantic, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const HOST_RUNTIMES: &[&str] = &[
    "astro",
    "react",
    "react-dom",
    "solid-js",
    "vue",
    "svelte",
    "preact",
];

/// Lower an authored Astro component to an independent synchronous SSR factory.
pub fn compile_moonshine_astro(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<MoonshineModule, String> {
    if !matches!(target, MoonshineModuleTarget::Ssr) {
        return Err(
            "Astro components compile for the server (Ssr target) only; frontmatter is never bundled into client modules"
                .into(),
        );
    }
    let split = split_astro_source(source).map_err(|error| error.to_string())?;
    let nodes =
        parse_astro_module_template_with_expression_validator(source, javascript_expression)
            .map_err(|error| error.to_string())?;
    let front = match &split.frontmatter {
        Some(frontmatter) => analyse_frontmatter(frontmatter.content)?,
        None => Front::default(),
    };
    let mut used = identifier_words(source);
    let props_name = fresh("__crepusAstroProps", &mut used);
    let factory = fresh("__CrepusAstro", &mut used);
    let raw_alias = fresh("__crepusAstroRawHTML", &mut used);
    let class_alias = fresh("__crepusAstroClass", &mut used);
    let child_alias = fresh("__crepusAstroChild", &mut used);
    let child_type = fresh("__crepusAstroChildType", &mut used);
    let h_alias = fresh("__crepusAstroH", &mut used);
    let component_alias = fresh("__crepusAstroComponent", &mut used);
    let slot_alias = fresh("__crepusAstroSlot", &mut used);
    let slots_name = fresh("__crepusAstroSlots", &mut used);
    let slots_type = fresh("__crepusAstroSlotsType", &mut used);
    let has_alias = fresh("__crepusAstroHasSlot", &mut used);
    let element_alias = fresh("__crepusAstroElement", &mut used);
    let spread_alias = fresh("__crepusAstroSpread", &mut used);
    let attribute_alias = fresh("__crepusAstroAttribute", &mut used);
    let mut template = Template {
        h_alias: h_alias.clone(),
        child_alias: child_alias.clone(),
        raw_alias,
        class_alias,
        component_alias,
        slot_alias,
        element_alias,
        spread_alias,
        attribute_alias,
        slots_name,
        h_used: false,
        raw_used: false,
        class_used: false,
        component_used: false,
        slot_used: false,
        element_used: false,
        spread_used: false,
    };
    let mut markup = String::new();
    template.nodes(&nodes, &mut markup)?;
    let root = markup;
    let props_type = if front.props_type {
        "Props"
    } else {
        "Record<string, any>"
    };
    let build = |has_slot: bool| {
        let mut specifiers = Vec::new();
        if template.h_used {
            specifiers.push(format!("h as {h_alias}"));
        }
        specifiers.push(format!("sourceChild as {child_alias}"));
        specifiers.push(format!("type SSRChild as {child_type}"));
        if template.component_used {
            specifiers.push(format!("sourceComponent as {}", template.component_alias));
        }
        if template.slot_used {
            specifiers.push(format!("sourceSlot as {}", template.slot_alias));
        }
        if has_slot {
            specifiers.push(format!("sourceHasSlot as {has_alias}"));
        }
        if template.element_used {
            specifiers.push(format!("sourceElement as {}", template.element_alias));
            specifiers.push(format!("sourceAttribute as {}", template.attribute_alias));
        }
        if template.spread_used {
            specifiers.push(format!("sourceSpread as {}", template.spread_alias));
        }
        let slots_used = template.slot_used || has_slot;
        if slots_used {
            specifiers.push(format!("type SourceSlots as {slots_type}"));
        }
        let mut module = front.hoisted.clone();
        module.push_str(&format!(
            "import {{{}}} from \"@tschk/moonshine-astro/ssr\";\n",
            specifiers.join(",")
        ));
        if template.raw_used {
            module.push_str(&format!(
                "import {{ rawHTML as {} }} from \"@tschk/moonshine-astro/ssr\";\n",
                template.raw_alias
            ));
        }
        let slots_parameter = if slots_used {
            format!(", {}: {slots_type} = {{}}", template.slots_name)
        } else {
            String::new()
        };
        module.push_str(&format!(
            "export default function {factory}({props_name}: {props_type}{slots_parameter}): {child_type} {{\n{}\nreturn {child_alias}(\n{root}\n);\n}}\n",
            front.body
        ));
        if template.class_used {
            module.push_str(&format!(
                "import {{classList as {}}} from \"@tschk/moonshine-astro/ssr\";\n",
                template.class_alias
            ));
        }
        module
    };
    let (mut module, has_slot) =
        bind_astro_props(&build(false), &props_name, &template.slots_name, &has_alias)?;
    if has_slot {
        module = bind_astro_props(&build(true), &props_name, &template.slots_name, &has_alias)?.0;
    }
    compile_moonshine_module(
        &module,
        &path.with_extension("astro.tsx"),
        MoonshineModuleTarget::Ssr,
    )
}

#[derive(Default)]
struct Front {
    hoisted: String,
    body: String,
    props_type: bool,
}

enum Kind {
    Hoist,
    Type,
    Body,
}

fn fresh(base: &str, used: &mut HashSet<String>) -> String {
    let mut name = base.to_owned();
    while used.contains(&name) {
        name.push('_');
    }
    used.insert(name.clone());
    name
}

fn decode_unicode_escapes(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(position) = rest.find("\\u") {
        out.push_str(&rest[..position]);
        let tail = &rest[position + 2..];
        let (code, length) = match tail.strip_prefix('{') {
            Some(braced) => match braced.find('}') {
                Some(end) => (u32::from_str_radix(&braced[..end], 16).ok(), end + 2),
                None => (None, 0),
            },
            None => (
                tail.get(..4)
                    .and_then(|digits| u32::from_str_radix(digits, 16).ok()),
                4,
            ),
        };
        match code.and_then(char::from_u32) {
            Some(character) => {
                out.push(character);
                rest = &tail[length..];
            }
            None => {
                out.push_str("\\u");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn identifier_words(source: &str) -> HashSet<String> {
    decode_unicode_escapes(source)
        .split(|character: char| {
            !(character.is_alphanumeric() || character == '_' || character == '$')
        })
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn json(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

fn reject_host_runtime(specifier: &str) -> Result<(), String> {
    let host = specifier.starts_with("astro:")
        || specifier.starts_with("astro/")
        || specifier.starts_with("@astrojs/")
        || HOST_RUNTIMES.iter().any(|runtime| {
            specifier == *runtime
                || specifier
                    .strip_prefix(runtime)
                    .is_some_and(|rest| rest.starts_with('/'))
        });
    if host {
        return Err(format!(
            "Astro frontmatter cannot import host runtime `{specifier}`"
        ));
    }
    Ok(())
}

fn type_name<'s>(statement: &'s Statement<'_>) -> Option<(&'s str, bool)> {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => Some((
            declaration.id.name.as_str(),
            declaration.type_parameters.is_some(),
        )),
        Statement::TSTypeAliasDeclaration(declaration) => Some((
            declaration.id.name.as_str(),
            declaration.type_parameters.is_some(),
        )),
        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(Declaration::TSInterfaceDeclaration(declaration)) => Some((
                declaration.id.name.as_str(),
                declaration.type_parameters.is_some(),
            )),
            Some(Declaration::TSTypeAliasDeclaration(declaration)) => Some((
                declaration.id.name.as_str(),
                declaration.type_parameters.is_some(),
            )),
            _ => None,
        },
        _ => None,
    }
}

fn classify(statement: &Statement<'_>) -> Result<Kind, String> {
    match statement {
        Statement::ImportDeclaration(import) => {
            reject_host_runtime(import.source.value.as_str())?;
            Ok(Kind::Hoist)
        }
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => {
            Ok(Kind::Type)
        }
        Statement::ExportNamedDeclaration(export) => {
            let type_declaration = matches!(
                &export.declaration,
                Some(Declaration::TSInterfaceDeclaration(_) | Declaration::TSTypeAliasDeclaration(_))
            );
            let type_list = export.declaration.is_none()
                && (export.export_kind == ImportOrExportKind::Type
                    || (!export.specifiers.is_empty()
                        && export
                            .specifiers
                            .iter()
                            .all(|specifier| specifier.export_kind == ImportOrExportKind::Type)));
            if type_declaration {
                Ok(Kind::Type)
            } else if type_list {
                match &export.source {
                    Some(source) => {
                        reject_host_runtime(source.value.as_str())?;
                        Ok(Kind::Hoist)
                    }
                    None => Ok(Kind::Type),
                }
            } else {
                Err("Astro frontmatter runtime exports are unsupported; only type-only exports are admitted".into())
            }
        }
        Statement::ExportDefaultDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_) => {
            Err("Astro frontmatter exports and re-exports are unsupported; only type-only exports are admitted".into())
        }
        Statement::TSImportEqualsDeclaration(_) => {
            Err("Astro frontmatter `import x = require(...)` is unsupported; use ESM imports".into())
        }
        _ => Ok(Kind::Body),
    }
}

struct TopLevel {
    depth: usize,
    error: Option<String>,
}

impl<'a> Visit<'a> for TopLevel {
    fn visit_import_expression(&mut self, _: &ImportExpression<'a>) {
        self.error.get_or_insert_with(|| {
            "Astro dynamic imports are unsupported; use static ESM imports".into()
        });
    }
    fn visit_function(&mut self, node: &Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, node, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, node: &ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, node);
        self.depth -= 1;
    }
    fn visit_for_of_statement(&mut self, node: &ForOfStatement<'a>) {
        if node.r#await && self.depth == 0 {
            self.error.get_or_insert_with(|| {
                "Astro frontmatter top-level for await is unsupported".into()
            });
        }
        walk::walk_for_of_statement(self, node);
    }
    fn visit_await_expression(&mut self, node: &AwaitExpression<'a>) {
        if self.depth == 0 {
            self.error
                .get_or_insert_with(|| "Astro frontmatter top-level await is unsupported".into());
        }
        walk::walk_await_expression(self, node);
    }
}

struct TypeScope<'s, 'a> {
    semantic: &'s Semantic<'a>,
    body: &'s [Span],
    error: Option<String>,
}

impl<'a> Visit<'a> for TypeScope<'_, 'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(symbol) = id
            .reference_id
            .get()
            .and_then(|reference| self.semantic.scoping().get_reference(reference).symbol_id())
        else {
            return;
        };
        let span = self.semantic.scoping().symbol_span(symbol);
        if self
            .body
            .iter()
            .any(|body| body.start <= span.start && span.end <= body.end)
        {
            self.error.get_or_insert_with(|| {
                format!(
                    "hoisted type declaration references per-render value `{}`",
                    id.name
                )
            });
        }
    }
}

fn analyse_frontmatter(content: &str) -> Result<Front, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, content, SourceType::ts().with_module(true)).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "Astro frontmatter parse failed: {:?}",
            parsed.errors
        ));
    }
    if !parsed.program.directives.is_empty() || parsed.program.hashbang.is_some() {
        return Err("Astro frontmatter directives and hashbangs are unsupported".into());
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!(
            "Astro frontmatter semantic errors: {:?}",
            built.errors
        ));
    }
    let semantic = built.semantic;
    let mut top = TopLevel {
        depth: 0,
        error: None,
    };
    top.visit_program(&parsed.program);
    if let Some(error) = top.error {
        return Err(error);
    }
    let mut front = Front::default();
    let mut body_spans = Vec::new();
    let mut type_statements = Vec::new();
    let mut cursor = 0usize;
    for statement in &parsed.program.body {
        let span = statement.span();
        let text = &content[cursor..span.end as usize];
        cursor = span.end as usize;
        match classify(statement)? {
            Kind::Hoist => {
                front.hoisted.push_str(text);
                front.hoisted.push('\n');
            }
            Kind::Type => {
                if let Some((name, generic)) = type_name(statement) {
                    if name == "Props" {
                        if generic {
                            return Err("generic Astro Props declarations are unsupported".into());
                        }
                        front.props_type = true;
                    }
                }
                front.hoisted.push_str(text);
                front.hoisted.push('\n');
                type_statements.push(statement);
            }
            Kind::Body => {
                front.body.push_str(text);
                front.body.push('\n');
                body_spans.push(span);
            }
        }
    }
    front.body.push_str(&content[cursor..]);
    let mut scope = TypeScope {
        semantic: &semantic,
        body: &body_spans,
        error: None,
    };
    for statement in type_statements {
        scope.visit_statement(statement);
    }
    match scope.error {
        Some(error) => Err(error),
        None => Ok(front),
    }
}

struct PropsRewrite<'s, 'a> {
    semantic: &'s Semantic<'a>,
    props_name: &'s str,
    slots_name: &'s str,
    has_alias: &'s str,
    edits: Vec<(usize, usize, String)>,
    has_used: bool,
    function_depth: usize,
    error: Option<String>,
}

impl<'a> PropsRewrite<'_, 'a> {
    fn unresolved(&self, id: &IdentifierReference<'a>) -> bool {
        id.reference_id.get().is_some_and(|reference| {
            self.semantic
                .scoping()
                .get_reference(reference)
                .symbol_id()
                .is_none()
        })
    }
}

impl<'a> Visit<'a> for PropsRewrite<'_, 'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if !call.optional && call.type_arguments.is_none() {
            if let Expression::StaticMemberExpression(has) = &call.callee {
                if let Expression::StaticMemberExpression(slots) = &has.object {
                    if let Expression::Identifier(id) = &slots.object {
                        if id.name == "Astro"
                            && self.unresolved(id)
                            && has.property.name == "has"
                            && slots.property.name == "slots"
                            && !has.optional
                            && !slots.optional
                        {
                            self.has_used = true;
                            self.edits.push((
                                has.span.start as usize,
                                has.span.end as usize,
                                self.has_alias.to_owned(),
                            ));
                            match call.arguments.first() {
                                Some(first) => {
                                    let at = first.span().start as usize;
                                    self.edits.push((at, at, format!("{}, ", self.slots_name)));
                                }
                                None => {
                                    let at = call.span.end as usize - 1;
                                    self.edits.push((at, at, self.slots_name.to_owned()));
                                }
                            }
                            for argument in &call.arguments {
                                self.visit_argument(argument);
                            }
                            return;
                        }
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if let Expression::StaticMemberExpression(member) = expression {
            if let Expression::Identifier(id) = &member.object {
                if id.name == "Astro" && self.unresolved(id) {
                    if member.property.name == "props" && !member.optional {
                        self.edits.push((
                            member.span.start as usize,
                            member.span.end as usize,
                            self.props_name.to_owned(),
                        ));
                    } else {
                        let property = member.property.name.to_string();
                        self.error.get_or_insert_with(|| {
                            format!("unsupported Astro global `Astro.{property}`; only Astro.props and Astro.slots.has calls are supported")
                        });
                    }
                    return;
                }
            }
        }
        walk::walk_expression(self, expression);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "Astro" && self.unresolved(id) {
            self.error.get_or_insert_with(|| {
                "unsupported Astro global use; only Astro.props reads and Astro.slots.has calls are supported".into()
            });
        } else if id.name == "arguments" && self.function_depth <= 1 {
            self.error.get_or_insert_with(|| {
                "`arguments` is unsupported in Astro frontmatter and template expressions".into()
            });
        }
    }
    fn visit_function(&mut self, node: &Function<'a>, flags: ScopeFlags) {
        self.function_depth += 1;
        walk::walk_function(self, node, flags);
        self.function_depth -= 1;
    }
}

fn bind_astro_props(
    module: &str,
    props_name: &str,
    slots_name: &str,
    has_alias: &str,
) -> Result<(String, bool), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, module, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "Astro component is not valid server TSX (frontmatter and template expressions must be synchronous and avoid angle-bracket type assertions): {:?}",
            parsed.errors
        ));
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut rewrite = PropsRewrite {
        semantic: &semantic,
        props_name,
        slots_name,
        has_alias,
        edits: Vec::new(),
        has_used: false,
        function_depth: 0,
        error: None,
    };
    rewrite.visit_program(&parsed.program);
    if let Some(error) = rewrite.error {
        return Err(error);
    }
    let has_used = rewrite.has_used;
    let mut edits = rewrite.edits;
    edits.sort_by_key(|edit| std::cmp::Reverse((edit.0, edit.1)));
    let mut output = module.to_owned();
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    Ok((output, has_used))
}

struct Template {
    h_alias: String,
    child_alias: String,
    raw_alias: String,
    class_alias: String,
    component_alias: String,
    slot_alias: String,
    element_alias: String,
    spread_alias: String,
    attribute_alias: String,
    slots_name: String,
    h_used: bool,
    raw_used: bool,
    class_used: bool,
    component_used: bool,
    slot_used: bool,
    element_used: bool,
    spread_used: bool,
}

fn expression(source: &str) -> String {
    format!("({source}\n)")
}

fn valid_tag(tag: &str) -> Result<bool, String> {
    let mut characters = tag.chars();
    let first = characters.next().ok_or("empty Astro tag name")?;
    if !first.is_ascii_alphabetic()
        || !characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(format!("unsupported Astro tag name `<{tag}>`"));
    }
    Ok(first.is_ascii_uppercase() || tag.contains('.'))
}

fn valid_attribute(tag: &str, name: &str) -> Result<(), String> {
    if let Some(directive) = name.strip_prefix("client:") {
        return Err(format!(
            "client:{directive} hydration directives are unsupported on <{tag}>; Astro components compile for the server only"
        ));
    }
    if name.contains(':') {
        return Err(format!(
            "directive or namespaced attribute `{name}` is unsupported on <{tag}>"
        ));
    }
    if name == "__proto__" {
        return Err("Astro __proto__ attributes are unsupported".into());
    }
    if name == "slot" {
        return Err(format!(
            "`slot` attribute on <{tag}> is only supported as a literal on an immediate child of a component"
        ));
    }
    if !valid_tag(tag)?
        && name.len() > 2
        && name
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
    {
        return Err(format!(
            "event attribute `{name}` is unsupported on <{tag}>; server components have no events"
        ));
    }
    let mut characters = name.chars();
    let first = characters.next().ok_or("empty attribute name")?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(format!("unsupported attribute name `{name}` on <{tag}>"));
    }
    Ok(())
}

enum Piece<'n> {
    Node(&'n Node),
    Marked(&'n Element),
    Nodes(&'n [Node]),
}

type SlotGroups<'n> = Vec<(String, Vec<Piece<'n>>)>;

fn literal_slot_name(owner: &str, attribute: &str, binding: &Binding) -> Result<String, String> {
    if !matches!(binding.origin, BindingOrigin::Literal) {
        return Err(format!(
            "dynamic `{attribute}` value on <{owner}> is unsupported; slot names must be string literals"
        ));
    }
    let raw: String = serde_json::from_str(&binding.value).map_err(|error| error.to_string())?;
    let name = svelte_entities::decode_static_attribute(&raw);
    if name.is_empty() {
        return Err(format!(
            "empty `{attribute}` value on <{owner}> is unsupported; slot names must be nonempty string literals"
        ));
    }
    Ok(name)
}

fn slot_marker(element: &Element) -> Result<Option<String>, String> {
    let mut markers = element
        .bindings
        .iter()
        .filter(|binding| binding.prop == "slot");
    let Some(marker) = markers.next() else {
        return Ok(None);
    };
    if markers.next().is_some() {
        return Err(format!("duplicate `slot` attribute on <{}>", element.tag));
    }
    literal_slot_name(&element.tag, "slot", marker).map(Some)
}

fn whitespace_node(node: &Node) -> bool {
    matches!(
        node,
        Node::Text(parts) if parts.iter().all(|part| matches!(
            part,
            TextPart::Literal(text) if text.chars().all(|character| matches!(character, ' ' | '\t' | '\n' | '\r' | '\x0c'))
        ))
    )
}

fn whitespace_piece(piece: &Piece<'_>) -> bool {
    matches!(piece, Piece::Node(node) if whitespace_node(node))
}

fn slot_groups(children: &[Node]) -> Result<SlotGroups<'_>, String> {
    let mut groups: SlotGroups<'_> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for child in children {
        let (name, piece) = match child {
            Node::Element(element) => match slot_marker(element)? {
                Some(name) if element.tag == "Fragment" => {
                    if element.bindings.len() != 1
                        || element.id.is_some()
                        || !element.classes.is_empty()
                        || !element.conditional_classes.is_empty()
                        || !element.event_handlers.is_empty()
                        || !element.animations.is_empty()
                    {
                        return Err(
                            "<Fragment slot> accepts no attributes other than `slot`".into()
                        );
                    }
                    let children = if element.children.iter().all(whitespace_node) {
                        &[][..]
                    } else {
                        element.children.as_slice()
                    };
                    (Some(name), Piece::Nodes(children))
                }
                Some(name) => (Some(name), Piece::Marked(element)),
                None => (None, Piece::Node(child)),
            },
            _ => (None, Piece::Node(child)),
        };
        let key = name.as_deref().unwrap_or("default");
        let position = match positions.get(key) {
            Some(position) => *position,
            None => {
                positions.insert(key.to_owned(), groups.len());
                groups.push((key.to_owned(), Vec::new()));
                groups.len() - 1
            }
        };
        groups[position].1.push(piece);
    }
    groups.retain(|(name, pieces)| name != "default" || !pieces.iter().all(whitespace_piece));
    Ok(groups)
}

impl Template {
    fn nodes(&mut self, nodes: &[Node], out: &mut String) -> Result<(), String> {
        out.push('[');
        for node in nodes {
            self.node(node, out)?;
            out.push(',');
        }
        out.push(']');
        Ok(())
    }

    fn node(&mut self, node: &Node, out: &mut String) -> Result<(), String> {
        match node {
            Node::Element(element) => self.element(element, false, out),
            Node::Text(parts) => {
                out.push('[');
                for part in parts {
                    match part {
                        TextPart::Literal(text) => {
                            out.push_str(&json(&svelte_entities::decode_static_dom(text)));
                        }
                        TextPart::Expr(source) => {
                            out.push_str(&format!("{}{}", self.child_alias, expression(source)));
                        }
                    }
                    out.push(',');
                }
                out.push(']');
                Ok(())
            }
            Node::If(block) => {
                // Reconstruct the prefix without changing JavaScript operator precedence.
                out.push('(');
                out.push_str(&block.condition);
                out.push_str(if block.else_children.is_some() {
                    " ? "
                } else {
                    " && "
                });
                self.nodes(&block.then_children, out)?;
                if let Some(children) = &block.else_children {
                    out.push_str(" : ");
                    self.nodes(children, out)?;
                }
                out.push(')');
                Ok(())
            }
            Node::For(block) => {
                out.push_str(&format!("({}.map(({}) => ", block.iterator, block.pattern));
                self.nodes(&block.body, out)?;
                out.push_str("))");
                Ok(())
            }
            Node::RawHtml(source) => {
                self.raw_used = true;
                out.push_str(&format!("{}{}", self.raw_alias, expression(source)));
                Ok(())
            }
            Node::Match(_) => Err("Astro match blocks are unsupported".into()),
            Node::LetDecl(_) => Err("Astro let declarations are unsupported".into()),
            Node::Include(_) => Err("Astro include nodes are unsupported".into()),
            Node::Embed(_) => Err("Astro embed nodes are unsupported".into()),
            Node::RawText(_) => Err("Astro raw text nodes are unsupported".into()),
        }
    }

    fn pieces(&mut self, pieces: &[Piece<'_>], out: &mut String) -> Result<(), String> {
        out.push('[');
        for piece in pieces {
            match piece {
                Piece::Node(node) => {
                    self.node(node, out)?;
                    out.push(',');
                }
                Piece::Marked(element) => {
                    self.element(element, true, out)?;
                    out.push(',');
                }
                Piece::Nodes(nodes) => {
                    for node in *nodes {
                        self.node(node, out)?;
                        out.push(',');
                    }
                }
            }
        }
        out.push(']');
        Ok(())
    }

    fn element(&mut self, element: &Element, marked: bool, out: &mut String) -> Result<(), String> {
        let tag = element.tag.as_str();
        if tag == "Fragment" {
            return Err(
                "<Fragment slot> wrappers are only supported as immediate children of a component"
                    .into(),
            );
        }
        let component = valid_tag(tag)?;
        if !element.event_handlers.is_empty() {
            return Err(format!(
                "event handlers are unsupported on <{tag}>; server components have no events"
            ));
        }
        if !element.animations.is_empty() {
            return Err(format!("animation directives are unsupported on <{tag}>"));
        }
        let void = VOID_TAGS.contains(&tag);
        if void && !element.children.is_empty() {
            return Err(format!("void element <{tag}> cannot have children"));
        }
        let mut attributes: Vec<(String, String, bool)> = Vec::new();
        if element.id.is_some()
            || !element.classes.is_empty()
            || !element.conditional_classes.is_empty()
        {
            return Err(
                "authored Astro attributes require the source-preserving core parser".into(),
            );
        }
        if tag == "slot" {
            return self.outlet(element, marked, out);
        }
        let spreads = element.bindings.iter().any(|binding| binding.prop == "...");
        let normalize_class_list = !spreads || !component;
        let class_list = normalize_class_list
            && element
                .bindings
                .iter()
                .any(|binding| binding.prop == "class:list");
        for binding in &element.bindings {
            if marked && !component && binding.prop == "slot" {
                continue;
            }
            let name = binding.prop.clone();
            let literal = matches!(binding.origin, BindingOrigin::Literal);
            let value = match binding.origin {
                BindingOrigin::Literal => {
                    let raw: String =
                        serde_json::from_str(&binding.value).map_err(|error| error.to_string())?;
                    json(&svelte_entities::decode_static_attribute(&raw))
                }
                BindingOrigin::Expression => format!("(void 0, {})", expression(&binding.value)),
                _ => return Err(format!("unsupported Astro binding `{name}`")),
            };
            if name != "..." && name != "class:list" && !(marked && component && name == "slot") {
                valid_attribute(tag, &name)?;
            }
            attributes.push((name, value, literal));
        }
        if class_list {
            let classes = attributes
                .iter()
                .filter(|(name, _, _)| name == "class")
                .count();
            if classes > 1 {
                return Err(format!("duplicate attribute `class` on <{tag}>"));
            }
            if let Some(index) = attributes.iter().position(|(name, _, _)| name == "class") {
                let (_, class, _) = attributes.remove(index);
                let (_, list, _) = attributes
                    .iter_mut()
                    .find(|(name, _, _)| name == "class:list")
                    .unwrap();
                // Astro combines class first at the class:list evaluation position.
                *list = format!("[{class}, {list}]");
            }
        }
        for (name, value, literal) in &mut attributes {
            if name == "class:list" && normalize_class_list {
                self.class_used = true;
                *literal = false;
                let normalized = format!("{}({value})", self.class_alias);
                *value = if component {
                    normalized
                } else {
                    format!("{normalized} || undefined")
                };
                *name = "class".into();
            }
        }
        let mut names = HashSet::new();
        for (name, _, _) in &attributes {
            if name == "..." {
                continue;
            }
            let key = if component {
                name.clone()
            } else if name == "className" {
                "class".to_owned()
            } else {
                name.to_ascii_lowercase()
            };
            if !names.insert(key) {
                return Err(format!("duplicate attribute `{name}` on <{tag}>"));
            }
        }
        let groups = if component {
            slot_groups(&element.children)?
        } else {
            Vec::new()
        };
        if component {
            self.component_used = true;
            out.push_str(&self.component_alias);
            out.push('(');
            out.push_str(tag);
        } else if spreads {
            self.element_used = true;
            out.push_str(&self.element_alias);
            out.push('(');
            out.push_str(&json(tag));
        } else {
            self.h_used = true;
            out.push_str(&self.h_alias);
            out.push('(');
            out.push_str(&json(tag));
        }
        if !component && spreads {
            out.push_str(", [");
            for (name, value, literal) in &attributes {
                if name == "..." {
                    self.spread_used = true;
                    out.push_str(&format!(
                        "...{}({}, {value}),",
                        self.spread_alias,
                        json(tag)
                    ));
                    continue;
                }
                out.push_str(&format!(
                    "{}({}, {}, {value}, {}),",
                    self.attribute_alias,
                    json(tag),
                    json(name),
                    !*literal
                ));
            }
            out.push(']');
        } else {
            out.push_str(", {");
            for (name, value, literal) in &attributes {
                if name == "..." {
                    out.push_str(&format!("...({value} as any),"));
                    continue;
                }
                if component && class_list && name == "class" {
                    out.push_str(&format!(
                        "...((value: string) => value ? {{class: value}} : {{}})({value}),"
                    ));
                    continue;
                }
                out.push_str(&json(name));
                out.push(':');
                if component || *literal {
                    out.push_str(value);
                } else {
                    // Capture before evaluating children while retaining the dynamic-value codec.
                    out.push_str(&format!("((value: unknown) => () => value)({value})"));
                }
                out.push(',');
            }
            out.push('}');
        }
        if component {
            if !groups.is_empty() {
                out.push_str(", {");
                for (name, pieces) in &groups {
                    out.push('[');
                    out.push_str(&json(name));
                    out.push_str(&format!("]: () => {}(", self.child_alias));
                    self.pieces(pieces, out)?;
                    out.push_str("),");
                }
                out.push('}');
            }
        } else {
            out.push_str(&format!(", {}(", self.child_alias));
            self.nodes(&element.children, out)?;
            out.push(')');
        }
        out.push(')');
        Ok(())
    }

    fn outlet(&mut self, element: &Element, marked: bool, out: &mut String) -> Result<(), String> {
        let mut name = None;
        for binding in &element.bindings {
            match binding.prop.as_str() {
                "slot" if marked => {}
                "slot" => {
                    return Err(
                        "`slot` attribute on <slot> is only supported as a literal on an immediate child of a component"
                            .into(),
                    );
                }
                "name" => {
                    if name.is_some() {
                        return Err("duplicate `name` attribute on <slot>".into());
                    }
                    name = Some(literal_slot_name("slot", "name", binding)?);
                }
                other => {
                    return Err(format!(
                        "unsupported attribute `{other}` on <slot>; only a literal `name` is supported"
                    ));
                }
            }
        }
        self.slot_used = true;
        out.push_str(&format!(
            "{}({}, {}, {}(",
            self.slot_alias,
            self.slots_name,
            json(name.as_deref().unwrap_or("default")),
            self.child_alias
        ));
        self.nodes(&element.children, out)?;
        out.push_str("))");
        Ok(())
    }
}

fn javascript_expression(source: &str) -> bool {
    let wrapped = format!("({source}\n)");
    let allocator = Allocator::default();
    Parser::new(&allocator, &wrapped, SourceType::tsx())
        .parse_expression()
        .is_ok_and(|expression| {
            matches!(&expression, Expression::ParenthesizedExpression(_))
                && expression.span().start == 0
                && expression.span().end as usize == wrapped.len()
        })
}

#[cfg(test)]
mod expression_validator_tests {
    use super::javascript_expression;

    #[test]
    fn fragments_cannot_close_the_validation_wrapper() {
        for source in ["value) + (other", "value), (other", "value) ? 1 : (other"] {
            assert!(!javascript_expression(source), "{source}");
        }
        for source in ["value + other", "value, other", "8 / /}/.source.length"] {
            assert!(javascript_expression(source), "{source}");
        }
    }
}
