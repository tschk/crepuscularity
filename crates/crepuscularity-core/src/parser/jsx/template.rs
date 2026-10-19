//! JSX template entry point and node sequence parsing.

use std::cell::Cell;

use crate::ast::*;

use super::super::indent::try_parse_let_decl;
use super::super::RawParseError;
use super::attrs::jsx_brace_expr;
use super::tags::parse_jsx_tag;
use super::text::{
    jsx_comment_container, jsx_err, jsx_open_tag_tail, jsx_text_node, map_jsx_offset,
    normalize_jsx_mapped,
};

thread_local! {
    // Includes the root sequence; a tag's child sequence adds one level.
    static JSX_DEPTH: Cell<usize> = const { Cell::new(0) };
}

struct JsxDepthGuard;

impl Drop for JsxDepthGuard {
    fn drop(&mut self) {
        JSX_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

fn depth_error(norm_root: &str, src: &str) -> RawParseError {
    jsx_err(
        norm_root,
        src,
        format!(
            "maximum JSX nesting depth ({}) exceeded",
            super::MAX_JSX_DEPTH
        ),
    )
}

pub(crate) fn parse_jsx_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    let (norm, map) = normalize_jsx_mapped(src);
    let root = norm.as_str();
    let parsed = parse_jsx_nodes_with_header(root, root, true).and_then(|(nodes, rest)| {
        let rest = rest.trim_start();
        if rest.is_empty() {
            Ok(nodes)
        } else {
            Err(jsx_err(
                root,
                rest,
                format!(
                    "unexpected trailing markup in JSX template: {}",
                    super::super::diagnostic_prefix(rest, 40)
                ),
            ))
        }
    });
    match parsed {
        Ok(nodes) => Ok(nodes),
        Err(mut err) => {
            if let Some(off) = err.byte_offset.take() {
                err.byte_offset = Some(map_jsx_offset(&map, off));
            }
            Err(err)
        }
    }
}

pub(crate) fn parse_jsx_nodes<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    parse_jsx_nodes_with_header(norm_root, src, false)
}

fn parse_jsx_nodes_with_header<'a>(
    norm_root: &'a str,
    src: &'a str,
    header_comments: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    JSX_DEPTH.with(|depth| {
        // The extra sequence at the limit may contain text and a closing tag,
        // but cannot open another tag (checked in the node loop below).
        if depth.get() > super::MAX_JSX_DEPTH {
            return Err(depth_error(norm_root, src));
        }
        depth.set(depth.get() + 1);
        let _guard = JsxDepthGuard;
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            parse_jsx_nodes_inner(norm_root, src, header_comments)
        })
    })
}

fn parse_jsx_nodes_inner<'a>(
    norm_root: &'a str,
    src: &'a str,
    mut header_comments: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    let mut nodes = Vec::new();
    let mut rest = src;

    loop {
        let t = rest.trim_start();
        if t.is_empty() || t.starts_with('<') || t.starts_with("{=") {
            if let Some(space) =
                super::super::markup_space_prefix(rest, JSX_DEPTH.with(Cell::get) > 1)
            {
                nodes.push(space);
            }
        }

        if t.is_empty() {
            rest = t;
            break;
        }
        if t.starts_with("</")
            || jsx_open_tag_tail(t, "else").is_some()
            || jsx_open_tag_tail(t, "else-if").is_some()
        {
            rest = t;
            break;
        }
        if header_comments && t.starts_with('#') {
            rest = &t[t.find('\n').unwrap_or(t.len())..];
            continue;
        }
        if t.starts_with("$:") {
            let end = t.find('\n').unwrap_or(t.len());
            let line = t[..end].trim();
            rest = &t[end..];
            let decl = try_parse_let_decl(line)
                .map_err(|err| jsx_err(norm_root, t, err.message))?
                .ok_or_else(|| jsx_err(norm_root, t, "unsupported JSX declaration"))?;
            nodes.push(Node::LetDecl(decl));
            continue;
        }
        if let Some(next) = jsx_comment_container(norm_root, t)? {
            if let Some(space) =
                super::super::markup_space_prefix(rest, JSX_DEPTH.with(Cell::get) > 1)
            {
                nodes.push(space);
            }
            rest = next;
            continue;
        }
        if header_comments && is_module_declaration(t) {
            return Err(jsx_err(
                norm_root,
                t,
                "JSX templates do not support JavaScript module imports or exports; provide template markup and host scope/handlers",
            ));
        }
        header_comments = false;
        if t.starts_with('<') {
            if JSX_DEPTH.with(Cell::get) > super::MAX_JSX_DEPTH {
                return Err(depth_error(norm_root, t));
            }
            rest = t;
            let (node, next) = parse_jsx_tag(norm_root, rest)?;
            nodes.push(node);
            rest = next;
            continue;
        }
        if t.starts_with("{=") {
            rest = t;
            let (mut expr, next) = jsx_brace_expr(norm_root, rest)?;
            if expr.starts_with('=') {
                expr = expr[1..].trim().to_string();
            }
            if expr.is_empty() {
                return Err(jsx_err(
                    norm_root,
                    rest,
                    "raw HTML requires a nonempty expression",
                ));
            }
            nodes.push(Node::RawHtml(expr));
            rest = next;
            continue;
        }
        let prev_len = rest.len();
        let (node_opt, next) = jsx_text_node(norm_root, rest, JSX_DEPTH.with(Cell::get) > 1)?;
        if let Some(node) = node_opt {
            nodes.push(node);
        }
        rest = next;
        if rest.len() == prev_len {
            let skip = rest
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            rest = &rest[skip..];
        }
    }

    Ok((nodes, rest))
}

fn is_module_declaration(source: &str) -> bool {
    let keyword_tail = |keyword: &str| {
        source.strip_prefix(keyword).filter(|tail| {
            tail.starts_with(char::is_whitespace) || tail.starts_with(['{', '*', '\'', '"'])
        })
    };
    if let Some(tail) = keyword_tail("export") {
        let tail = tail.trim_start();
        return tail.starts_with(['{', '*'])
            || [
                "default",
                "const",
                "let",
                "var",
                "function",
                "class",
                "async",
                "type",
                "interface",
            ]
            .iter()
            .any(|keyword| {
                tail.strip_prefix(keyword).is_some_and(|rest| {
                    rest.chars()
                        .next()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$')
                })
            });
    }
    keyword_tail("import").is_some_and(|tail| {
        let tail = tail.trim_start();
        tail.starts_with(['{', '*', '\'', '"'])
            || tail.split_whitespace().any(|word| word == "from")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_closing_control_tags_cannot_absorb_following_nodes() {
        for source in [
            "<if condition={ok} /><p /></if>",
            "<for let=\"item\" in={items} /><p /></for>",
            "<match on={value} /><case /></match>",
            "<if condition={a}><else-if condition={b} /><p /></else-if></if>",
        ] {
            let error = parse_jsx_template(source).expect_err(source);
            assert!(error.message.contains("cannot be self-closing"));
        }
        assert!(parse_jsx_template("<if condition={ok}><else /></if><p />").is_ok());
    }

    #[test]
    fn match_requires_exact_case_tag_names() {
        for tag in ["casework", "case-widget", "case界"] {
            let source = format!("<match on={{value}}><{tag} /></match>");
            assert!(parse_jsx_template(&source).is_err(), "accepted {tag}");
        }
        let nodes = parse_jsx_template(
            "<match on={value}><case pattern=\"a\" /><case pattern=\"_\"><p /></case></match>",
        )
        .unwrap();
        let Node::Match(block) = &nodes[0] else {
            panic!("expected match")
        };
        assert_eq!(block.arms.len(), 2);
        assert!(block.arms[0].body.is_empty());
        assert_eq!(block.arms[1].body.len(), 1);
    }

    #[test]
    fn rejects_unconsumed_top_level_markup() {
        for source in [
            "</orphan>",
            "<div /></orphan><p />",
            "<else><p /></else>",
            "<div /><else-if condition={ok}><p /></else-if>",
        ] {
            let error = parse_jsx_template(source).expect_err(source);
            assert!(error.message.contains("unexpected trailing markup"));
            let offset = error
                .byte_offset
                .expect("error should locate trailing markup");
            assert!(source[offset..].starts_with("</") || source[offset..].starts_with("<else"));
        }
    }

    #[test]
    fn trailing_markup_offset_maps_to_original_unicode_source() {
        let source = "<p>｛name｝</p>\n  </界>";
        let error = parse_jsx_template(source).expect_err("unmatched closing tag");
        assert_eq!(error.byte_offset, source.find("</界>"));
    }

    #[test]
    fn else_if_chain_uses_matching_closing_tags() {
        let source = "<if condition={a}><p>A</p><else-if condition={b}><p>B</p></else-if><else-if condition={c}><p>C</p></else-if><else><p>D</p></else></if>";
        let nodes = parse_jsx_template(source).expect("valid else-if chain");
        let Node::If(first) = &nodes[0] else {
            panic!("expected if");
        };
        assert_eq!(first.condition, "a");
        let Node::If(second) = &first.else_children.as_ref().unwrap()[0] else {
            panic!("expected first else-if");
        };
        assert_eq!(second.condition, "b");
        let Node::If(third) = &second.else_children.as_ref().unwrap()[0] else {
            panic!("expected second else-if");
        };
        assert_eq!(third.condition, "c");
        assert!(
            matches!(&third.else_children.as_ref().unwrap()[0], Node::Element(el) if el.tag == "p")
        );
    }

    #[test]
    fn rejects_excessive_else_if_chain_depth() {
        let branches = "<else-if condition={false}><p /></else-if>".repeat(300);
        let source = format!("<if condition={{false}}>{branches}</if>");
        let error = parse_jsx_template(&source).expect_err("excessive chain depth");
        assert!(error.message.contains("maximum JSX else-if chain depth"));
    }

    #[test]
    fn else_if_rejects_missing_or_mismatched_closing_tags() {
        for source in [
            "<if condition={a}><else-if condition={b}><p>B</p></if>",
            "<if condition={a}><else-if condition={b}><p>B</p></else></if>",
        ] {
            assert!(parse_jsx_template(source).is_err(), "{source}");
        }
    }

    #[test]
    fn else_prefix_element_names_remain_elements() {
        for tag in ["elsewhere", "else-if-widget", "else界"] {
            let source = format!("<{tag}>text</{tag}>");
            let nodes = parse_jsx_template(&source).expect("ordinary element");
            assert!(matches!(&nodes[0], Node::Element(el) if el.tag == tag));

            let source = format!("<if condition={{ok}}><{tag} /><else><{tag} /></else></if>");
            let nodes = parse_jsx_template(&source).expect("ordinary element inside branches");
            let Node::If(block) = &nodes[0] else {
                panic!("expected if block");
            };
            assert!(matches!(&block.then_children[0], Node::Element(el) if el.tag == tag));
            assert!(
                matches!(&block.else_children.as_ref().unwrap()[0], Node::Element(el) if el.tag == tag)
            );
        }
    }

    #[test]
    fn accepts_nested_if_else_and_multiple_roots() {
        for source in [
            "<if condition={ok}><p>yes</p><else><p>no</p></else></if><div />\n",
            "<if condition={ok}><p>yes</p><else /></if><div />",
            "<if condition={ok}><p>yes</p><else\t><p>no</p></else></if><div />",
        ] {
            assert_eq!(parse_jsx_template(source).expect("valid template").len(), 2);
        }
    }
    fn nested_tags(depth: usize) -> String {
        format!("{}text{}", "<div>".repeat(depth), "</div>".repeat(depth))
    }

    #[test]
    fn quoted_attribute_angles_are_not_nesting() {
        let source = format!("<div title=\"{}\" />", "<".repeat(300));
        assert!(parse_jsx_template(&source).is_ok());
    }

    #[test]
    fn quoted_closers_cannot_bypass_actual_depth_limit() {
        let source = format!(
            "{}text{}",
            "<div title=\"</fake>\">".repeat(super::super::MAX_JSX_DEPTH + 1),
            "</div>".repeat(super::super::MAX_JSX_DEPTH + 1),
        );
        assert!(parse_jsx_template(&source)
            .unwrap_err()
            .message
            .contains("maximum JSX nesting depth"));
    }

    #[test]
    fn jsx_depth_limit_has_exact_tag_boundary_and_allows_wide_siblings() {
        let limit = super::super::MAX_JSX_DEPTH;
        assert!(parse_jsx_template(&nested_tags(limit)).is_ok());
        let expression = format!(
            "{}{{value}}{}",
            "<div>".repeat(limit),
            "</div>".repeat(limit)
        );
        assert!(parse_jsx_template(&expression).is_ok());
        assert!(parse_jsx_template(&nested_tags(limit + 1))
            .unwrap_err()
            .message
            .contains("maximum JSX nesting depth"));
        let source = format!(
            "{}<span />{}",
            "<div>".repeat(limit),
            "</div>".repeat(limit)
        );
        assert!(parse_jsx_template(&source)
            .unwrap_err()
            .message
            .contains("maximum JSX nesting depth"));
        assert_eq!(
            parse_jsx_template(&"<div />".repeat(limit * 2))
                .unwrap()
                .len(),
            limit * 2
        );
    }

    #[test]
    fn jsx_depth_counter_recovers_after_errors_and_is_thread_local() {
        for _ in 0..2 {
            assert!(parse_jsx_template(&nested_tags(super::super::MAX_JSX_DEPTH + 1)).is_err());
            assert!(parse_jsx_template("<div>").is_err());
            assert!(parse_jsx_template("<div />").is_ok());
            assert_eq!(super::JSX_DEPTH.with(std::cell::Cell::get), 0);
        }
        super::JSX_DEPTH.with(|depth| depth.set(super::super::MAX_JSX_DEPTH + 1));
        let _guard = super::JsxDepthGuard;
        std::thread::spawn(|| assert!(parse_jsx_template("<div />").is_ok()))
            .join()
            .unwrap();
        assert!(parse_jsx_template("<div />").is_err());
        // Restore the synthetic parent depth before its RAII guard is dropped.
        super::JSX_DEPTH.with(|depth| depth.set(1));
    }
    #[test]
    fn jsx_comment_only_containers_are_ignored_at_node_boundaries() {
        for source in [
            r#"{/* root } <fake> ' \" */}<div />{/* tail */}"#,
            "{ /* first */ /* second { } */ }<div />",
            "{// braces } and <fake> ignored\n}<div />",
            "{// ignored\u{2028}}<div />",
            "{// ignored\u{2029}}<div />",
            "<div>{/* inner */}<span />{/* tail */}</div>",
        ] {
            assert_eq!(parse_jsx_template(source).unwrap().len(), 1, "{source}");
        }
        let limit = super::super::MAX_JSX_DEPTH;
        let source = format!(
            "{}{{/* {{ <fake> ' */}}{}",
            "<div>".repeat(limit),
            "</div>".repeat(limit)
        );
        assert!(parse_jsx_template(&source).is_ok());
    }

    #[test]
    fn inline_jsx_comments_preserve_adjacent_text_and_spaces() {
        for (source, expected) in [
            ("<div>before{/* comment */}after</div>", "beforeafter"),
            (
                "<div> before {/* comment */} after </div>",
                " before  after ",
            ),
            ("<div>a{/*one*/}{/*two*/}b</div>", "ab"),
            (r#"<div>before{/*note*/}"after"</div>"#, r#"before"after""#),
            (r#"<div>"before{/*note*/}after"</div>"#, "beforeafter"),
            ("<div>a{// closing } ignored\n}b</div>", "ab"),
        ] {
            let nodes = parse_jsx_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element");
            };
            let text: String = element
                .children
                .iter()
                .flat_map(|node| {
                    let Node::Text(parts) = node else {
                        panic!("text");
                    };
                    parts.iter().map(|part| {
                        let TextPart::Literal(value) = part else {
                            panic!("literal");
                        };
                        value.as_str()
                    })
                })
                .collect();
            assert_eq!(text, expected, "{source}");
        }
    }

    #[test]
    fn jsx_comments_cannot_hide_code_or_unterminated_containers() {
        for source in [
            "{/* note */ value}",
            "{value /* note */}",
            "<div>before{/* note */ value}after</div>",
            "{ /* unterminated }",
            "{/* closed block */",
            "{// no newline }",
        ] {
            let error = parse_jsx_template(source).unwrap_err();
            assert!(error.message.contains("comment"), "{source}: {error:?}");
            assert!(error.byte_offset.is_some(), "{error:?}");
        }
        let source = "<div>界{/* unclosed }</div>";
        assert_eq!(
            parse_jsx_template(source).unwrap_err().byte_offset,
            source.find("/*")
        );
    }

    #[test]
    fn jsx_comment_syntax_inside_strings_remains_literal() {
        let nodes =
            parse_jsx_template(r#"<div title="/* { } */">{'/* { } */'}a{'// }'}b</div>"#).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(element.bindings[0].value, "\"/* { } */\"");
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![
                TextPart::Expr("'/* { } */'".into()),
                TextPart::Literal("a".into()),
                TextPart::Expr("'// }'".into()),
                TextPart::Literal("b".into()),
            ])
        );
    }
    #[test]
    fn jsx_node_boundary_interpolations_use_reactive_text_parts() {
        let nodes = parse_jsx_template("<button>{/*comment*/}{count}</button>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if matches!(&parts[..], [TextPart::Expr(value)] if value == "count"))
        );
        assert!(
            matches!(&parse_jsx_template("{=html}").unwrap()[0], Node::RawHtml(value) if value == "html")
        );
    }
    #[test]
    fn leading_jsx_expressions_share_the_complete_contiguous_text_region() {
        for (source, expected) in [
            (
                "<button>{count}suffix</button>",
                vec![
                    TextPart::Expr("count".into()),
                    TextPart::Literal("suffix".into()),
                ],
            ),
            (
                "<button>{count}{other}</button>",
                vec![
                    TextPart::Expr("count".into()),
                    TextPart::Expr("other".into()),
                ],
            ),
            (
                "<button>{count}{/* ignored */}suffix{other}</button>",
                vec![
                    TextPart::Expr("count".into()),
                    TextPart::Literal("suffix".into()),
                    TextPart::Expr("other".into()),
                ],
            ),
        ] {
            let nodes = parse_jsx_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element");
            };
            assert!(
                matches!(&element.children[..], [Node::Text(parts)] if parts == &expected),
                "{source}: {:?}",
                element.children
            );
        }
    }
    #[test]
    fn comment_tokens_in_backtick_interpolations_are_not_consumed() {
        let nodes = parse_jsx_template("<div>{`/* { } */ // quoted`}</div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if matches!(&parts[..], [TextPart::Expr(value)] if value == "`/* { } */ // quoted`"))
        );
    }
    #[test]
    fn ignored_jsx_comments_do_not_end_root_header_eligibility() {
        let nodes =
            parse_jsx_template("{/* ignored */}\n# header\n$: let count = 1\n<div />").unwrap();
        assert!(matches!(&nodes[..], [Node::LetDecl(_), Node::Element(_)]));
        for source in [
            "<div>{/* ignored */}# nested</div>",
            "<div />{/* ignored */}# after",
        ] {
            let nodes = parse_jsx_template(source).unwrap();
            let children = if let Node::Element(element) = &nodes[0] {
                if element.children.is_empty() {
                    &nodes[1..]
                } else {
                    &element.children[..]
                }
            } else {
                panic!("element");
            };
            assert!(
                matches!(children, [Node::Text(parts)] if parts.iter().any(|part| matches!(part, TextPart::Literal(value) if value.starts_with('#'))))
            );
        }
    }
    #[test]
    fn raw_html_boundaries_remain_distinct_in_contiguous_jsx_text() {
        for source in [
            r#"<div>{1}{="<b>x</b>"}tail</div>"#,
            r#"<div>{1}{/* gap */}{="<b>x</b>"}{/* gap */}tail</div>"#,
            r#"<div>prefix{="<b>x</b>"}{2}tail</div>"#,
        ] {
            let nodes = parse_jsx_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element");
            };
            assert!(
                matches!(&element.children[..], [Node::Text(_), Node::RawHtml(value), Node::Text(_)] if value == r#""<b>x</b>""#),
                "{source}: {:?}",
                element.children
            );
        }
        let nodes = parse_jsx_template(r#"{1}{="<b>x</b>"}{2}"#).unwrap();
        assert!(matches!(
            &nodes[..],
            [Node::Text(_), Node::RawHtml(_), Node::Text(_)]
        ));
    }
}
