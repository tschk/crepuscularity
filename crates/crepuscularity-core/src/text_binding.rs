//! Mixed text represented as ordinary DSL string concatenation.

use crate::ast::TextPart;
use crate::parser::{match_delimiter, quote_expression_string};
use crate::{eval::eval_expr, TemplateContext, TemplateValue};

/// Keep single-expression bindings unchanged; force mixed operands to display text.
pub fn encode_text_binding(parts: &[TextPart]) -> Option<String> {
    if let [TextPart::Expr(expression)] = parts {
        return Some(expression.trim().to_owned());
    }
    if !parts.iter().any(|part| matches!(part, TextPart::Expr(_))) {
        return None;
    }
    let mut binding = String::from("\"\"");
    for part in parts {
        binding.push_str(" + ");
        match part {
            TextPart::Literal(value) => binding.push_str(&quote_expression_string(value)),
            TextPart::Expr(expression) => {
                binding.push('(');
                binding.push_str(expression);
                binding.push(')');
            }
        }
    }
    Some(binding)
}

/// Recover compiler-generated concatenations so hosts can preserve display coercion.
/// Other valid DSL expressions remain the host's normal evaluator's responsibility.
pub fn decode_text_binding(binding: &str) -> Option<Vec<TextPart>> {
    let mut rest = binding.trim().strip_prefix("\"\"")?.trim_start();
    let mut parts = Vec::new();
    loop {
        rest = rest.strip_prefix('+')?.trim_start();
        if rest.starts_with('"') {
            let mut escaped = false;
            let end = rest.char_indices().skip(1).find_map(|(index, ch)| {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    return Some(index);
                }
                None
            })?;
            let TemplateValue::Str(value) =
                eval_expr(&rest[..=end], &TemplateContext::new()).ok()?
            else {
                return None;
            };
            parts.push(TextPart::Literal(value));
            rest = &rest[end + 1..];
        } else if rest.starts_with('(') {
            let end = match_delimiter(rest, '(', ')')?;
            let expression = rest[1..end].trim();
            if expression.is_empty() {
                return None;
            }
            parts.push(TextPart::Expr(expression.into()));
            rest = &rest[end + 1..];
        } else {
            return None;
        }
        rest = rest.trim_start();
        if rest.is_empty() {
            return Some(parts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_text_round_trips_literals_and_nested_expressions() {
        let parts = vec![
            TextPart::Literal("  quotes \" \\n\n\t {literal} </script> 界  ".into()),
            TextPart::Expr("count + (other * 2)".into()),
            TextPart::Literal("tail".into()),
            TextPart::Expr("\")\"".into()),
        ];
        let binding = encode_text_binding(&parts).unwrap();
        assert_eq!(decode_text_binding(&binding), Some(parts));
    }

    #[test]
    fn concatenation_uses_the_dsl_display_contract_and_stays_live() {
        let parts = vec![
            TextPart::Literal("Values: ".into()),
            TextPart::Expr("value".into()),
            TextPart::Literal("!".into()),
        ];
        let binding = encode_text_binding(&parts).unwrap();
        let mut context = TemplateContext::new();
        for (value, expected) in [
            (TemplateValue::Bool(false), "Values: false!"),
            (
                TemplateValue::List(vec![TemplateContext::new()]),
                "Values: [1 items]!",
            ),
            (TemplateValue::Scope(Default::default()), "Values: !"),
            (TemplateValue::Null, "Values: !"),
            (TemplateValue::Int(7), "Values: 7!"),
        ] {
            context.vars.insert("value".into(), value);
            assert!(
                matches!(eval_expr(&binding, &context).unwrap(), TemplateValue::Str(value) if value == expected)
            );
        }
    }

    #[test]
    fn existing_single_expressions_and_literals_keep_their_representation() {
        assert_eq!(
            encode_text_binding(&[TextPart::Expr(" count ".into())]),
            Some("count".into())
        );
        assert_eq!(
            encode_text_binding(&[TextPart::Literal("{literal}".into())]),
            None
        );
        assert_eq!(decode_text_binding("count + 1"), None);
    }

    #[test]
    fn malformed_concatenations_never_decode_partial_text() {
        for binding in [
            r#""" + "unterminated"#,
            r#""" + (count"#,
            r#""" + ()"#,
            r#""" + (count) trailing"#,
            r#""" + (count) +"#,
            r#""" + bare"#,
        ] {
            assert_eq!(decode_text_binding(binding), None, "{binding}");
        }
    }
    #[test]
    fn existing_concat_literals_keep_expression_escape_semantics() {
        let literal = r#""\r\'\q\n\t\"\\""#;
        let binding = format!("\"\" + {literal} + (count)");
        let mut context = TemplateContext::new();
        context.set("count", 7);
        let parts = decode_text_binding(&binding).unwrap();
        let reconstructed: String = parts
            .iter()
            .map(|part| match part {
                TextPart::Literal(value) => value.clone(),
                TextPart::Expr(expression) => {
                    crate::context::value_to_str(&eval_expr(expression, &context).unwrap())
                }
            })
            .collect();
        assert_eq!(
            reconstructed,
            crate::context::value_to_str(&eval_expr(&binding, &context).unwrap())
        );
        assert!(reconstructed.starts_with(r"\r\'\q"));
    }
}
