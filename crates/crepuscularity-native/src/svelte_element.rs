use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::collections::HashSet;

const TAG_PATTERN: &str = "/^[a-zA-Z][a-zA-Z0-9:_-]*$/";
const VOID_PATTERN: &str =
    "/^(?:area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i";

struct Found {
    start: usize,
    end: usize,
    markup: bool,
    selector: (usize, usize),
    attribute: (usize, usize),
    namespace: Option<((usize, usize), (usize, usize))>,
    name_end: usize,
    tail_from: usize,
    open_end: usize,
    close_start: Option<usize>,
}

fn is_marker(name: &JSXElementName<'_>, marker: &str) -> bool {
    match name {
        JSXElementName::IdentifierReference(id) => id.name.as_str() == marker,
        JSXElementName::Identifier(id) => id.name.as_str() == marker,
        _ => false,
    }
}

fn markup_parent(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::JSXElement(_) | AstKind::JSXFragment(_) | AstKind::JSXAttribute(_)
    )
}

fn unique(
    base: &str,
    fresh: &dyn Fn(&str) -> String,
    source: &str,
    reserved: &HashSet<String>,
    taken: &[&str],
) -> String {
    let mut name = fresh(base);
    while source.contains(&name) || reserved.contains(&name) || taken.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

fn reserved_names(semantic: &oxc_semantic::Semantic<'_>) -> HashSet<String> {
    semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
            AstKind::IdentifierReference(id) => Some(id.name.to_string()),
            _ => None,
        })
        .collect()
}

struct Lower<'a> {
    source: &'a str,
    found: &'a [Found],
    dynamic_tag_attribute: &'a str,
    tag: &'a str,
    map: &'a str,
    namespace_param: Option<&'a str>,
    namespace_output: &'a str,
    target: MoonshineModuleTarget,
    realm: &'a str,
}

impl Lower<'_> {
    fn range(&self, start: usize, end: usize) -> String {
        let mut out = String::new();
        let mut cursor = start;
        for marker in self.found {
            if marker.start < cursor || marker.end > end {
                continue;
            }
            out.push_str(&self.source[cursor..marker.start]);
            out.push_str(&self.marker(marker));
            cursor = marker.end;
        }
        out.push_str(&self.source[cursor..end]);
        out
    }

    fn check(&self, empty: &str) -> String {
        let tag = self.tag;
        let realm = self.realm;
        format!(
            "if(!{tag})return {empty};if(typeof {tag}!==\"string\"||!{TAG_PATTERN}.test({tag}))throw new {realm}.TypeError(\"Invalid dynamic element name\");"
        )
    }

    fn marker(&self, marker: &Found) -> String {
        let tag = self.tag;
        let mut spans = vec![marker.attribute];
        if let Some((attribute, _)) = marker.namespace {
            spans.push(attribute);
        }
        spans.sort_unstable();
        let mut leading = String::new();
        let mut cursor = marker.name_end;
        for (start, end) in spans {
            let mut piece = self.range(cursor, start);
            piece.truncate(piece.trim_end().len());
            leading.push_str(&piece);
            cursor = end;
        }
        let trailing = self.range(cursor, marker.tail_from);
        let mut open = format!(
            "<div{leading}{trailing} {}={{{tag}}}",
            self.dynamic_tag_attribute
        );
        if let (MoonshineModuleTarget::Dom, Some(ns), Some(_)) =
            (self.target, self.namespace_param, marker.namespace)
        {
            open.push_str(&format!(" {}={{{ns}}}", self.namespace_output));
        }
        let tail = &self.source[marker.tail_from..marker.open_end];
        let selector = self.range(marker.selector.0, marker.selector.1);
        let ordinary = match marker.close_start {
            None => format!("{open}{tail}"),
            Some(close) => format!("{open}{tail}{}</div>", self.range(marker.open_end, close)),
        };
        let markup = match self.target {
            MoonshineModuleTarget::Dom => {
                let map = self.map;
                let check = self.check("[]");
                match self.namespace_param {
                    None => format!(
                        "{map}(()=>{{const {tag}=({selector});{check}return [{tag}] as const;}},{tag}=><>{ordinary}</>,undefined,read=>read)"
                    ),
                    Some(ns) => {
                        let reader = marker
                            .namespace
                            .map(|(_, expression)| {
                                format!(",()=>({})", self.range(expression.0, expression.1))
                            })
                            .unwrap_or_default();
                        format!(
                            "{map}(()=>{{const {tag}=({selector});{check}return [{tag}] as const;}},({tag},{ns})=><>{ordinary}</>{reader})"
                        )
                    }
                }
            }
            MoonshineModuleTarget::Ssr => {
                let body = match marker.close_start {
                    None => format!("({ordinary})"),
                    Some(_) => format!("{VOID_PATTERN}.test({tag})?({open}/>):({ordinary})"),
                };
                format!(
                    "(()=>{{const {tag}=({selector});{}return {body};}})()",
                    self.check("null")
                )
            }
        };
        if marker.markup {
            format!("{{{markup}}}")
        } else {
            markup
        }
    }
}

/// Lowers each `element_marker` JSX element into a plain `<div>` placeholder that carries
/// `dynamic_tag_attribute={validatedTag}`. The original `selector_attribute={expr}` is
/// consumed and evaluated once per read; all other attributes and children are preserved
/// verbatim. DOM wraps the placeholder in `mapArray`, imported from
/// `@tschk/moonshine-dom/control`; SSR emits a lazy IIFE that never evaluates children for
/// void tags. Falsy selector values suppress the element; truthy non-strings and names
/// outside `[a-zA-Z][a-zA-Z0-9:_-]*` throw `TypeError`. Generated code carries no type
/// annotations, so it is valid under both `IrPolicy::Typed` and `IrPolicy::JavaScript`.
#[cfg(test)]
pub(crate) fn transform_with_map(
    source: &str,
    element_marker: &str,
    selector_attribute: &str,
    dynamic_tag_attribute: &str,
    target: MoonshineModuleTarget,
    realm: &str,
    fresh: &dyn Fn(&str) -> String,
) -> Result<(String, Option<String>), String> {
    transform_with_namespace(
        source,
        element_marker,
        selector_attribute,
        dynamic_tag_attribute,
        None,
        None,
        target,
        realm,
        fresh,
    )
}

/// Same as `transform_with_map`, plus dynamic namespace metadata. A marker carrying a
/// `namespace_input={expr}` attribute has it consumed (never evaluated in SSR). In DOM, when
/// any marker carries it, every dynamic element goes through one private generic wrapper
/// (returned alias) over `mapArray`, which captures the latest namespace per invocation while
/// keying only on the tag; placeholders with metadata receive `namespace_output={param}`.
/// Ordinary attributes, including `xmlns`, are preserved untouched.
#[expect(
    clippy::too_many_arguments,
    reason = "extends the existing lowering seam with paired private namespace markers"
)]
pub(crate) fn transform_with_namespace(
    source: &str,
    element_marker: &str,
    selector_attribute: &str,
    dynamic_tag_attribute: &str,
    namespace_input: Option<&str>,
    namespace_output: Option<&str>,
    target: MoonshineModuleTarget,
    realm: &str,
    fresh: &dyn Fn(&str) -> String,
) -> Result<(String, Option<String>), String> {
    let namespace_names = match (namespace_input, namespace_output) {
        (Some(input), Some(output)) if !input.is_empty() && !output.is_empty() => {
            Some((input, output))
        }
        (None, _) => None,
        _ => {
            return Err("namespace lowering requires both input and output attribute names".into())
        }
    };
    if element_marker.is_empty() || !source.contains(element_marker) {
        return Ok((source.to_owned(), None));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid dynamic element source syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut found = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let opening = &element.opening_element;
        if !is_marker(&opening.name, element_marker) {
            continue;
        }
        let name_end = opening.name.span().end as usize;
        let mut tail_from = name_end;
        let mut selected = None;
        let mut namespace = None;
        for item in &opening.attributes {
            tail_from = tail_from.max(item.span().end as usize);
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            let JSXAttributeName::Identifier(name) = &attribute.name else {
                continue;
            };
            let name = name.name.as_str();
            if let Some((input, output)) = namespace_names {
                if name == output {
                    return Err(format!(
                        "dynamic element marker cannot carry compiler attribute {output}"
                    ));
                }
                if name == input {
                    if namespace.is_some() {
                        return Err(format!(
                            "duplicate dynamic element namespace attribute {input}"
                        ));
                    }
                    let expression = match &attribute.value {
                        Some(JSXAttributeValue::ExpressionContainer(container)) => {
                            container.expression.as_expression().map(|e| e.span())
                        }
                        _ => None,
                    }
                    .ok_or_else(|| {
                        format!("dynamic element namespace {input} requires an expression")
                    })?;
                    namespace = Some((
                        (attribute.span.start as usize, attribute.span.end as usize),
                        (expression.start as usize, expression.end as usize),
                    ));
                    continue;
                }
            }
            if name == dynamic_tag_attribute {
                return Err(format!(
                    "dynamic element marker cannot carry compiler attribute {dynamic_tag_attribute}"
                ));
            }
            if name != selector_attribute {
                continue;
            }
            if selected.is_some() {
                return Err(format!(
                    "duplicate dynamic element selector attribute {selector_attribute}"
                ));
            }
            let expression = match &attribute.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    container.expression.as_expression().map(|e| e.span())
                }
                _ => None,
            }
            .ok_or_else(|| {
                format!("dynamic element selector {selector_attribute} requires an expression")
            })?;
            selected = Some((
                (attribute.span.start as usize, attribute.span.end as usize),
                (expression.start as usize, expression.end as usize),
            ));
        }
        let (attribute, selector) = selected.ok_or_else(|| {
            format!("dynamic element marker requires selector attribute {selector_attribute}")
        })?;
        found.push(Found {
            start: element.span.start as usize,
            end: element.span.end as usize,
            markup: markup_parent(nodes.parent_kind(node.id())),
            selector,
            attribute,
            namespace,
            name_end,
            tail_from,
            open_end: opening.span.end as usize,
            close_start: element
                .closing_element
                .as_ref()
                .map(|closing| closing.span.start as usize),
        });
    }
    if found.is_empty() {
        return Ok((source.to_owned(), None));
    }
    found.sort_by_key(|marker| (marker.start, std::cmp::Reverse(marker.end)));
    let reserved = reserved_names(&semantic);
    let mut taken: Vec<&str> = vec![element_marker, selector_attribute, dynamic_tag_attribute];
    if let Some((input, output)) = namespace_names {
        taken.push(input);
        taken.push(output);
    }
    let wrapped =
        matches!(target, MoonshineModuleTarget::Dom) && found.iter().any(|m| m.namespace.is_some());
    let map = match target {
        MoonshineModuleTarget::Dom => Some(unique(
            "__crepusSvelteElementMap",
            fresh,
            source,
            &reserved,
            &taken,
        )),
        MoonshineModuleTarget::Ssr => None,
    };
    if let Some(map) = &map {
        taken.push(map);
    }
    let tag = unique("__crepusSvelteElementTag", fresh, source, &reserved, &taken);
    taken.push(&tag);
    let (base, ns) = if wrapped {
        let base = unique(
            "__crepusSvelteElementBase",
            fresh,
            source,
            &reserved,
            &taken,
        );
        taken.push(&base);
        let ns = unique(
            "__crepusSvelteElementNamespace",
            fresh,
            source,
            &reserved,
            &taken,
        );
        (Some(base), Some(ns))
    } else {
        (None, None)
    };
    let lower = Lower {
        source,
        found: &found,
        dynamic_tag_attribute,
        tag: &tag,
        map: map.as_deref().unwrap_or_default(),
        namespace_param: ns.as_deref(),
        namespace_output: namespace_names.map_or("", |names| names.1),
        target,
        realm,
    };
    let body = lower.range(0, source.len());
    let output = match (&map, &base) {
        (Some(map), Some(base)) => format!(
            "{body}\nimport {{mapArray as {base}}} from \"@tschk/moonshine-dom/control\";\nfunction {map}<T,U>(read:()=>readonly T[], render:(tag:T,ns:unknown)=>U, readNS?:()=>unknown) {{ let ns:unknown; return {base}(()=>{{const items=read();ns=readNS?.();return items;}},tag=>render(tag,ns),undefined,read=>read); }}"
        ),
        (Some(map), None) => {
            format!("{body}\nimport {{mapArray as {map}}} from \"@tschk/moonshine-dom/control\";")
        }
        _ => body,
    };
    Ok((output, map))
}

#[cfg(test)]
fn transform(
    source: &str,
    marker: &str,
    selector: &str,
    tag: &str,
    target: MoonshineModuleTarget,
    realm: &str,
    fresh: &dyn Fn(&str) -> String,
) -> Result<String, String> {
    transform_with_map(source, marker, selector, tag, target, realm, fresh).map(|result| result.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "__crepusSvelteElementMap";
    const TAG: &str = "__crepusSvelteElementTag";

    fn lower(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let output = transform(
            source,
            "ElementMarker",
            "__selector",
            "__tag",
            target,
            "globalThis",
            &|base| base.to_owned(),
        )?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}");
        Ok(output)
    }

    const DOM: MoonshineModuleTarget = MoonshineModuleTarget::Dom;
    const SSR: MoonshineModuleTarget = MoonshineModuleTarget::Ssr;

    #[test]
    fn dom_root_uses_map_array_once_per_selector_read_and_keeps_attributes_and_children() {
        let output = lower(
            "export default ()=> <ElementMarker __selector={tag} class=\"a\" {...rest}>hi {x}</ElementMarker>",
            DOM,
        )
        .unwrap();
        let check = format!(
            "if(!{TAG})return [];if(typeof {TAG}!==\"string\"||!{TAG_PATTERN}.test({TAG}))throw new globalThis.TypeError(\"Invalid dynamic element name\");"
        );
        let expected = format!(
            "export default ()=> {MAP}(()=>{{const {TAG}=(tag);{check}return [{TAG}] as const;}},{TAG}=><><div class=\"a\" {{...rest}} __tag={{{TAG}}}>hi {{x}}</div></>,undefined,read=>read)\nimport {{mapArray as {MAP}}} from \"@tschk/moonshine-dom/control\";"
        );
        assert_eq!(output, expected);
        assert!(!output.contains("ElementMarker") && !output.contains("__selector"));
        assert_eq!(output.matches("(tag)").count(), 1);
    }

    #[test]
    fn child_attribute_and_expression_contexts_receive_contextual_braces() {
        for target in [DOM, SSR] {
            let output = lower(
                "export default ()=> <p>{x}<ElementMarker __selector={t}/><i data=<ElementMarker __selector={u}/>/>{ok && <ElementMarker __selector={v}/>}</p>",
                target,
            )
            .unwrap();
            let call = match target {
                MoonshineModuleTarget::Dom => format!("{MAP}("),
                MoonshineModuleTarget::Ssr => "(()=>{".to_owned(),
            };
            assert!(output.contains(&format!("<p>{{x}}{{{call}")), "{output}");
            assert!(output.contains(&format!("data={{{call}")), "{output}");
            assert!(output.contains(&format!("{{ok && {call}")), "{output}");
            assert!(
                output.contains(&format!("<div __tag={{{TAG}}}/>")),
                "{output}"
            );
        }
    }

    #[test]
    fn ssr_void_branch_is_childless_and_children_appear_only_in_the_ordinary_branch() {
        let output = lower(
            "export default ()=> <ElementMarker __selector={tag} id=\"a\">{child()}</ElementMarker>",
            SSR,
        )
        .unwrap();
        let branches = format!(
            "return {VOID_PATTERN}.test({TAG})?(<div id=\"a\" __tag={{{TAG}}}/>):(<div id=\"a\" __tag={{{TAG}}}>{{child()}}</div>);}})()"
        );
        assert!(output.contains(&branches), "{output}");
        assert!(output.contains(&format!("const {TAG}=(tag);if(!{TAG})return null;")));
        assert_eq!(output.matches("child()").count(), 1);
        assert!(!output.contains("mapArray") && !output.contains("import"));

        let self_closing = lower(
            "export default ()=> <ElementMarker __selector={tag} id=\"a\"/>",
            SSR,
        )
        .unwrap();
        assert!(
            self_closing.contains(&format!("return (<div id=\"a\" __tag={{{TAG}}}/>);}})()")),
            "{self_closing}"
        );
        assert!(!self_closing.contains(".test(__crepusSvelteElementTag)?"));
    }

    #[test]
    fn static_and_namespace_attributes_are_preserved_in_order() {
        for target in [DOM, SSR] {
            let output = lower(
                "export default ()=> <ElementMarker xmlns=\"http://www.w3.org/2000/svg\" a:b=\"1\" data-x='q' __selector={t} class={c} {...rest}>text &amp; {y}</ElementMarker>",
                target,
            )
            .unwrap();
            assert!(
                output.contains("<div xmlns=\"http://www.w3.org/2000/svg\" a:b=\"1\" data-x='q' class={c} {...rest} __tag={"),
                "{output}"
            );
            assert!(output.contains(">text &amp; {y}</div>"), "{output}");
        }
    }

    #[test]
    fn nested_markers_in_children_are_lowered_independently() {
        for target in [DOM, SSR] {
            let output = lower(
                "export default ()=> <ElementMarker __selector={a}><ElementMarker __selector={b}>x</ElementMarker></ElementMarker>",
                target,
            )
            .unwrap();
            assert!(!output.contains("ElementMarker"), "{output}");
            assert!(output.contains("(a)") && output.contains("(b)"), "{output}");
        }
    }

    #[test]
    fn aliases_reserve_raw_and_decoded_identifier_names() {
        let output = lower(
            "export default ()=>{const __crepusSvelteElementTag=1;const \\u005f_crepusSvelteElementMap=2;return <ElementMarker __selector={__crepusSvelteElementTag}/>}",
            DOM,
        )
        .unwrap();
        assert!(
            output.contains("mapArray as __crepusSvelteElementMap_}"),
            "{output}"
        );
        assert!(
            output.contains("const __crepusSvelteElementTag_=(__crepusSvelteElementTag);"),
            "{output}"
        );
    }

    #[test]
    fn marker_misuse_is_a_hard_error() {
        for source in [
            "export default ()=> <ElementMarker/>",
            "export default ()=> <ElementMarker __selector={a} __selector={b}/>",
            "export default ()=> <ElementMarker __selector=\"a\"/>",
            "export default ()=> <ElementMarker __selector={}/>",
            "export default ()=> <ElementMarker __selector={a} __tag={b}/>",
            "export default ()=> <ElementMarker __selector={a}",
        ] {
            for target in [DOM, SSR] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }

    #[test]
    fn missing_markers_leave_source_unchanged_without_parsing_or_naming() {
        let source = "const a=;<<<";
        for target in [DOM, SSR] {
            let output = transform(
                source,
                "ElementMarker",
                "__selector",
                "__tag",
                target,
                "globalThis",
                &|_| panic!("alias requested"),
            )
            .unwrap();
            assert_eq!(output, source);
        }
        let plain = "export default ()=> <p __selector={x}>{x}</p>";
        for target in [DOM, SSR] {
            assert_eq!(lower(plain, target).unwrap(), plain);
        }
    }

    const NS: &str = "__crepusSvelteElementNamespace";
    const BASE: &str = "__crepusSvelteElementBase";

    fn lower_ns(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let (output, _) = transform_with_namespace(
            source,
            "ElementMarker",
            "__selector",
            "__tag",
            Some("__ns"),
            Some("__nsout"),
            target,
            "globalThis",
            &|base| base.to_owned(),
        )?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}");
        Ok(output)
    }

    #[test]
    fn no_namespace_metadata_keeps_direct_map_array_output() {
        let source = "export default ()=> <ElementMarker __selector={t} xmlns={n}/>";
        let old = lower(source, DOM).unwrap();
        assert_eq!(lower_ns(source, DOM).unwrap(), old);
        assert!(old.contains("undefined,read=>read)") && !old.contains("function"));
    }

    #[test]
    fn metadata_is_removed_before_or_after_selector_and_wrapper_is_uniform() {
        for source in [
            "export default ()=> <><ElementMarker __ns={n} __selector={t} class=\"a\"/><ElementMarker class=\"b\" __selector={u}/></>",
            "export default ()=> <><ElementMarker __selector={t} class=\"a\" __ns={n}/><ElementMarker class=\"b\" __selector={u}/></>",
        ] {
            let output = lower_ns(source, DOM).unwrap();
            assert!(!output.contains("__ns=") && !output.contains("__selector"), "{output}");
            assert!(
                output.contains(&format!("<div class=\"a\" __tag={{{TAG}}} __nsout={{{NS}}}/></>,()=>(n))")),
                "{output}"
            );
            assert!(output.contains(&format!("({TAG},{NS})=><><div class=\"b\" __tag={{{TAG}}}/></>)")), "{output}");
            assert_eq!(output.matches("mapArray").count(), 1);
            assert!(output.contains(&format!("function {MAP}<T,U>(")));
            assert!(output.contains(&format!("{BASE}(()=>{{const items=read();ns=readNS?.();return items;}},tag=>render(tag,ns),undefined,read=>read)")));
        }
    }

    #[test]
    fn nested_namespace_markers_lower_independently() {
        let output = lower_ns(
            "export default ()=> <ElementMarker __selector={a} __ns={x}><ElementMarker __ns={y} __selector={b}>z</ElementMarker></ElementMarker>",
            DOM,
        )
        .unwrap();
        assert!(
            !output.contains("ElementMarker") && !output.contains("__ns="),
            "{output}"
        );
        assert!(
            output.contains(",()=>(x))") && output.contains(",()=>(y))"),
            "{output}"
        );
    }

    #[test]
    fn ssr_erases_metadata_without_reading_it_and_keeps_xmlns() {
        let output = lower_ns(
            "export default ()=> <ElementMarker xmlns={o} __ns={n} __selector={t}/>",
            SSR,
        )
        .unwrap();
        assert!(
            !output.contains("(n)") && !output.contains("__ns") && !output.contains("__nsout"),
            "{output}"
        );
        assert!(output.contains("<div xmlns={o} __tag="), "{output}");
        assert_eq!(output.matches("{o}").count(), 1);
    }

    #[test]
    fn namespace_attributes_and_duplicates_are_validated() {
        for source in [
            "export default ()=> <ElementMarker __selector={a} __ns={x} __ns={y}/>",
            "export default ()=> <ElementMarker __selector={a} __ns=\"x\"/>",
            "export default ()=> <ElementMarker __selector={a} __ns={}/>",
            "export default ()=> <ElementMarker __selector={a} __nsout={y}/>",
        ] {
            for target in [DOM, SSR] {
                assert!(lower_ns(source, target).is_err(), "{source}");
            }
        }
    }

    #[test]
    fn namespace_names_avoid_collisions() {
        let output = lower_ns(
            "export default ()=>{const __crepusSvelteElementBase=1;const __crepusSvelteElementNamespace=2;return <ElementMarker __selector={__crepusSvelteElementBase} __ns={__crepusSvelteElementNamespace}/>}",
            DOM,
        )
        .unwrap();
        assert!(
            output.contains("mapArray as __crepusSvelteElementBase_}"),
            "{output}"
        );
        assert!(
            output.contains("(__crepusSvelteElementTag,__crepusSvelteElementNamespace_)=>"),
            "{output}"
        );
    }
}
