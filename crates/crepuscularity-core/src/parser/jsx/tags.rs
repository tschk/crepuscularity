//! JSX tag parsing (if, for, match, elements).

use crate::ast::*;

use super::super::RawParseError;
use super::attrs::{jsx_parse_attrs, JsxAttr, JsxAttrValue};
use super::builders::{
    jsx_build_element, jsx_build_embed, jsx_build_include, jsx_build_let, jsx_validate_source_attrs,
};
use super::template::parse_jsx_nodes;
use super::text::{jsx_close, jsx_err, jsx_open_tag_tail};

// Keep JSX-only admission consistent across ordinary tags and branch parsers.
// The shared scanner also serves frontends that support attribute shorthand.
fn parse_attrs<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let (attrs, rest, self_closing) = jsx_parse_attrs(norm_root, src)?;
    if attrs.iter().any(|attribute| attribute.key.starts_with('{')) {
        return Err(jsx_err(
            norm_root,
            src,
            "JSX attribute shorthand is not supported; use a named attribute binding",
        ));
    }
    Ok((attrs, rest, self_closing))
}

pub(crate) fn parse_jsx_tag<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(Node, &'a str), RawParseError> {
    let src = src.trim_start();
    let after_lt = &src[1..];
    let name_end = after_lt
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(after_lt.len());
    let tag = &after_lt[..name_end];
    let rest = after_lt[name_end..].trim_start();

    let (attrs, after_gt, self_closing) = parse_attrs(norm_root, rest)?;

    if self_closing && matches!(tag, "if" | "for" | "match") {
        return Err(jsx_err(
            norm_root,
            src,
            format!("<{tag}> cannot be self-closing; use a matching closing tag"),
        ));
    }

    if matches!(tag, "include" | "island" | "crepus-island") {
        jsx_validate_source_attrs(tag, &attrs)
            .map_err(|message| jsx_err(norm_root, src, message))?;
    }
    if matches!(tag, "island" | "crepus-island") {
        if !self_closing {
            return Err(jsx_err(
                norm_root,
                src,
                "islands must be self-closing; child content is unsupported",
            ));
        }
        if let Some(adapter) = attrs.iter().find(|attribute| attribute.key == "adapter") {
            if adapter.as_str().is_none_or(|value| value.trim().is_empty()) {
                return Err(jsx_err(
                    norm_root,
                    src,
                    "island adapter must be a nonempty literal",
                ));
            }
        }
    }
    if matches!(tag, "let" | "let-default") {
        if !self_closing {
            return Err(jsx_err(
                norm_root,
                src,
                "declarations must be self-closing; child content is unsupported",
            ));
        }
        let mut names = attrs.iter().filter(|attribute| attribute.key == "name");
        let name = names.next().and_then(|attribute| attribute.as_str());
        if names.next().is_some() || name.is_none_or(|value| value.trim().is_empty()) {
            return Err(jsx_err(
                norm_root,
                src,
                "declaration requires exactly one nonempty literal variable name",
            ));
        }
        if !name.is_some_and(crate::eval::is_variable_name) {
            return Err(jsx_err(
                norm_root,
                src,
                "declaration requires a supported variable name",
            ));
        }
        let mut values = attrs.iter().filter(|attribute| attribute.key == "value");
        let value = values.next().and_then(|attribute| attribute.as_expr());
        if values.next().is_some()
            || value
                .as_ref()
                .is_none_or(|expression| expression.trim().is_empty())
        {
            return Err(jsx_err(
                norm_root,
                src,
                "declaration requires exactly one nonempty value expression",
            ));
        }
    }
    match tag {
        "if" => parse_jsx_if(norm_root, attrs, after_gt, "if", 0),
        "else" | "else-if" => Err(jsx_err(
            norm_root,
            src,
            format!("<{tag}> encountered outside <if>"),
        )),
        "for" => parse_jsx_for(norm_root, attrs, after_gt),
        "match" => parse_jsx_match(norm_root, attrs, after_gt),
        "island" | "crepus-island" if self_closing => Ok((jsx_build_embed(attrs), after_gt)),
        "include" if self_closing => Ok((jsx_build_include(attrs, vec![]), after_gt)),
        "include" => {
            let (slot, rest) = parse_jsx_nodes(norm_root, after_gt)?;
            let rest = jsx_close(norm_root, rest, "include")?;
            Ok((jsx_build_include(attrs, slot), rest))
        }
        "let" => Ok((Node::LetDecl(jsx_build_let(attrs, false)), after_gt)),
        "let-default" => Ok((Node::LetDecl(jsx_build_let(attrs, true)), after_gt)),
        _ if self_closing => Ok((
            Node::Element(
                jsx_build_element(tag, attrs, vec![])
                    .map_err(|message| jsx_err(norm_root, after_gt, message))?,
            ),
            after_gt,
        )),
        _ => {
            let (children, rest) = parse_jsx_nodes(norm_root, after_gt)?;
            let rest = jsx_close(norm_root, rest, tag)?;
            Ok((
                Node::Element(
                    jsx_build_element(tag, attrs, children)
                        .map_err(|message| jsx_err(norm_root, after_gt, message))?,
                ),
                rest,
            ))
        }
    }
}

fn parse_jsx_if<'a>(
    norm_root: &'a str,
    attrs: Vec<JsxAttr>,
    children_src: &'a str,
    closing_tag: &str,
    branch_depth: usize,
) -> Result<(Node, &'a str), RawParseError> {
    if branch_depth >= super::MAX_JSX_DEPTH {
        return Err(jsx_err(
            norm_root,
            children_src,
            format!(
                "maximum JSX else-if chain depth ({}) exceeded",
                super::MAX_JSX_DEPTH
            ),
        ));
    }
    let mut conditions = attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "condition" | "test" | "cond"));
    let condition = conditions.next().and_then(|attribute| attribute.as_expr());
    if conditions.next().is_some()
        || condition
            .as_ref()
            .is_none_or(|expression| expression.trim().is_empty())
    {
        return Err(jsx_err(
            norm_root,
            children_src,
            "conditional tag requires exactly one nonempty condition expression",
        ));
    }
    let condition = condition.expect("condition validated above");

    let (then_children, rest) = parse_jsx_nodes(norm_root, children_src)?;
    let rest = if closing_tag == "else-if" {
        jsx_close(norm_root, rest, closing_tag)?
    } else {
        rest
    };
    let rest = rest.trim_start();

    let (else_children, rest) = if let Some(after_name) = jsx_open_tag_tail(rest, "else-if") {
        let after_name = after_name.trim_start();
        let (ei_attrs, ei_body, self_closing) = parse_attrs(norm_root, after_name)?;
        if self_closing {
            return Err(jsx_err(
                norm_root,
                rest,
                "<else-if> cannot be self-closing; use a matching closing tag",
            ));
        }
        let (nested, next) =
            parse_jsx_if(norm_root, ei_attrs, ei_body, "else-if", branch_depth + 1)?;
        (Some(vec![nested]), next)
    } else if let Some(after_name) = jsx_open_tag_tail(rest, "else") {
        let after_name = after_name.trim_start();
        let (else_attrs, else_body, self_closing) = parse_attrs(norm_root, after_name)?;
        if !else_attrs.is_empty() {
            return Err(jsx_err(
                norm_root,
                after_name,
                "else branches do not accept attributes",
            ));
        }
        if self_closing {
            (Some(vec![]), else_body)
        } else {
            let (else_nodes, after_nodes) = parse_jsx_nodes(norm_root, else_body)?;
            let after_close = jsx_close(norm_root, after_nodes, "else")?;
            (Some(else_nodes), after_close)
        }
    } else {
        (None, rest)
    };

    let rest = if closing_tag == "if" {
        jsx_close(norm_root, rest, closing_tag)?
    } else {
        rest
    };
    Ok((
        Node::If(IfBlock {
            condition,
            then_children,
            else_children,
        }),
        rest,
    ))
}

fn parse_jsx_for<'a>(
    norm_root: &'a str,
    attrs: Vec<JsxAttr>,
    children_src: &'a str,
) -> Result<(Node, &'a str), RawParseError> {
    let mut patterns = attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "let" | "var"));
    let pattern = patterns.next().and_then(|attribute| attribute.as_str());
    if patterns.next().is_some() || pattern.is_none_or(|pattern| pattern.trim().is_empty()) {
        return Err(jsx_err(
            norm_root,
            children_src,
            "for tag requires exactly one nonempty literal loop variable (let or var)",
        ));
    }
    let pattern = pattern.expect("loop variable validated above").to_string();
    if !crate::eval::is_variable_name(&pattern) {
        return Err(jsx_err(
            norm_root,
            children_src,
            "loop requires a supported item variable name",
        ));
    }
    let mut iterators = attrs.iter().filter(|attribute| attribute.key == "in");
    let iterator = iterators.next().and_then(|attribute| attribute.as_expr());
    if iterators.next().is_some()
        || iterator
            .as_ref()
            .is_none_or(|expression| expression.trim().is_empty())
    {
        return Err(jsx_err(
            norm_root,
            children_src,
            "for tag requires exactly one nonempty iterator expression (in)",
        ));
    }
    let iterator = iterator.expect("iterator validated above");

    let (body, rest) = parse_jsx_nodes(norm_root, children_src)?;
    let rest = jsx_close(norm_root, rest, "for")?;
    Ok((
        Node::For(ForBlock {
            pattern,
            iterator,
            body,
        }),
        rest,
    ))
}

fn parse_jsx_match<'a>(
    norm_root: &'a str,
    attrs: Vec<JsxAttr>,
    children_src: &'a str,
) -> Result<(Node, &'a str), RawParseError> {
    let mut expressions = attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "on" | "value"));
    let expr = expressions.next().and_then(|attribute| attribute.as_expr());
    if expressions.next().is_some()
        || expr
            .as_ref()
            .is_none_or(|expression| expression.trim().is_empty())
    {
        return Err(jsx_err(
            norm_root,
            children_src,
            "match tag requires exactly one nonempty subject expression",
        ));
    }
    let expr = expr.expect("match subject validated above");

    let mut arms = Vec::new();
    let mut rest = children_src.trim_start();

    while let Some(after_name) = jsx_open_tag_tail(rest, "case") {
        let after_name = after_name.trim_start();
        let (case_attrs, case_body, self_closing) = parse_attrs(norm_root, after_name)?;
        let mut patterns = case_attrs
            .iter()
            .filter(|attribute| matches!(attribute.key.as_str(), "pattern" | "match" | "when"));
        let pattern_attribute = patterns.next();
        let pattern = match pattern_attribute.map(|attribute| &attribute.value) {
            None => Some("_".to_string()),
            Some(JsxAttrValue::Str(pattern) | JsxAttrValue::Expr(pattern)) => Some(pattern.clone()),
            Some(JsxAttrValue::Bool(_)) => None,
        };
        if patterns.next().is_some()
            || pattern
                .as_ref()
                .is_none_or(|pattern| pattern.trim().is_empty())
        {
            return Err(jsx_err(
                norm_root,
                after_name,
                "case requires at most one nonempty literal or braced pattern",
            ));
        }
        let pattern = pattern.expect("case pattern validated above");
        let (body, after_body): (Vec<Node>, &str) = if self_closing {
            (vec![], case_body)
        } else {
            let (b, r) = parse_jsx_nodes(norm_root, case_body)?;
            let r = jsx_close(norm_root, r, "case")?;
            (b, r)
        };
        arms.push(MatchArm { pattern, body });
        rest = after_body.trim_start();
    }

    let rest = jsx_close(norm_root, rest, "match")?;
    Ok((Node::Match(MatchBlock { expr, arms }), rest))
}
