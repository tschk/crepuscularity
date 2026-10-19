#[derive(Debug, Clone)]
pub(super) struct ScopeBinding {
    pub name: String,
    pub variable: String,
}

/// The handful of token differences between the SwiftUI and Compose emitters:
/// the state store, keyword separator, null literal, and string interpolation.
#[derive(Debug, Clone, Copy)]
pub(super) struct LangSyntax {
    pub store: &'static str,
    pub kw_sep: &'static str,
    pub null: &'static str,
    escape_dollar: bool,
}

pub(super) const SWIFT: LangSyntax = LangSyntax {
    store: "CrepusActions.model",
    kw_sep: ": ",
    null: "nil",
    escape_dollar: false,
};

pub(super) const KOTLIN: LangSyntax = LangSyntax {
    store: "CrepusStateStore",
    kw_sep: " = ",
    null: "null",
    escape_dollar: true,
};

impl LangSyntax {
    pub(super) fn escape(&self, s: &str) -> String {
        let mut escaped = String::new();
        for ch in s.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                '$' if self.escape_dollar => escaped.push_str("\\$"),
                ch if ch.is_control() => {
                    if self.escape_dollar {
                        escaped.push_str(&format!("\\u{:04x}", ch as u32));
                    } else {
                        escaped.push_str(&format!("\\u{{{:x}}}", ch as u32));
                    }
                }
                ch => escaped.push(ch),
            }
        }
        escaped
    }

    pub(super) fn identifier(&self, name: &str) -> String {
        let mut out = String::new();
        for (idx, ch) in name.chars().enumerate() {
            if (idx == 0 && (ch.is_ascii_alphabetic() || ch == '_'))
                || (idx > 0 && (ch.is_ascii_alphanumeric() || ch == '_'))
            {
                out.push(ch);
            } else {
                out.push('_');
            }
        }
        if out.chars().all(|ch| ch == '_') {
            out = "item".to_string();
        }
        format!("`{out}`")
    }

    pub(super) fn loop_item_var(&self, name: &str) -> String {
        // Encoded locals cannot shadow bridge symbols or collide with generated suffixes.
        let encoded: String = name
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.identifier(&format!("__crepusItem_{encoded}"))
    }

    pub(super) fn loop_binding(&self, name: &str, scopes: &[ScopeBinding]) -> ScopeBinding {
        let variable = self.loop_item_var(name);
        let variable = if scopes.iter().any(|scope| scope.name == name) {
            self.identifier(&format!(
                "{}__depth{}",
                variable.trim_matches('`'),
                scopes.len()
            ))
        } else {
            variable
        };
        ScopeBinding {
            name: name.to_string(),
            variable,
        }
    }

    pub(super) fn binding_path(&self, bind: &str, scopes: &[ScopeBinding]) -> String {
        let bind = bind.trim();
        for scope in scopes.iter().rev() {
            if let Some(suffix) = bind
                .strip_prefix(scope.name.as_str())
                .filter(|suffix| suffix.is_empty() || suffix.starts_with('.'))
            {
                return format!(
                    "{} + \"{}\"",
                    self.binding_path_var(&scope.variable),
                    self.escape(suffix)
                );
            }
        }
        format!("\"{}\"", self.escape(bind))
    }

    pub(super) fn binding_path_var(&self, var: &str) -> String {
        self.identifier(&format!("{}__crepusPath", var.trim_matches('`')))
    }

    fn scope_args(&self, expr: &str, scopes: &[ScopeBinding]) -> String {
        let expr = expr.trim();
        let Some(inner) = scopes.last() else {
            return String::new();
        };
        let simple = crate::scoped_expression::is_binding_path(expr);
        let selected = if simple {
            scopes
                .iter()
                .rev()
                .find(|scope| {
                    expr.strip_prefix(scope.name.as_str())
                        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('.'))
                })
                .unwrap_or(inner)
        } else {
            inner
        };
        let sep = self.kw_sep;
        let mut visible = Vec::new();
        for scope in scopes.iter().rev() {
            if !visible
                .iter()
                .any(|seen: &&ScopeBinding| seen.name == scope.name)
            {
                visible.push(scope);
            }
        }
        // Ordinary paths carry only the selected item, avoiding ancestor collection copies.
        if simple || visible.len() == 1 {
            return format!(
                ", scopeName{sep}\"{}\", scope{sep}{}",
                self.escape(&selected.name),
                selected.variable
            );
        }
        visible.reverse();
        let aliases = visible
            .iter()
            .map(|scope| {
                if self.escape_dollar {
                    format!("\"{}\" to {}", self.escape(&scope.name), scope.variable)
                } else {
                    format!("\"{}\": {}", self.escape(&scope.name), scope.variable)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let environment = if self.escape_dollar {
            format!(
                "JsonObject(mapOf(\"aliases\" to JsonObject(mapOf({aliases})), \"local\" to {}))",
                inner.variable
            )
        } else {
            format!("[\"aliases\": [{aliases}], \"local\": {}]", inner.variable)
        };
        format!(", scopeName{sep}\"@crepus:lexical\", scope{sep}{environment}")
    }

    fn model(&self, accessor: &str, expr: &str, scopes: &[ScopeBinding]) -> String {
        let expr = expr.trim();
        format!(
            "{}.{accessor}(\"{}\"{})",
            self.store,
            self.escape(expr),
            self.scope_args(expr, scopes)
        )
    }

    pub(super) fn model_text(&self, expr: &str, scopes: &[ScopeBinding]) -> String {
        if expr.is_empty() {
            return "\"\"".to_string();
        }
        self.model("text", expr, scopes)
    }

    pub(super) fn text_content(
        &self,
        content: &str,
        bind: Option<&str>,
        scopes: &[ScopeBinding],
    ) -> String {
        if let Some(bind) = bind {
            if let Some(parts) = crepuscularity_core::text_binding::decode_text_binding(bind) {
                let parts: Vec<_> = parts
                    .into_iter()
                    .map(|part| match part {
                        crepuscularity_core::ast::TextPart::Literal(value) => {
                            crate::ir::LabelPart::Literal(value)
                        }
                        crepuscularity_core::ast::TextPart::Expr(value) => {
                            crate::ir::LabelPart::Expr(value)
                        }
                    })
                    .collect();
                return self.label_text(content, &parts, scopes);
            }
            return self.model_text(bind, scopes);
        }
        format!("\"{}\"", self.escape(content))
    }

    pub(super) fn label_text(
        &self,
        label: &str,
        parts: &[crate::ir::LabelPart],
        scopes: &[ScopeBinding],
    ) -> String {
        // shortcut: keep unsupported snapshots until the native bridge evaluates the full DSL.
        if parts.is_empty()
            || parts.iter().any(|part| match part {
                crate::ir::LabelPart::Expr(value) => {
                    !crate::scoped_expression::is_binding_path(value)
                        && crepuscularity_core::eval::eval_literal(value).is_none()
                }
                crate::ir::LabelPart::Literal(_) => false,
            })
        {
            return format!("\"{}\"", self.escape(label));
        }
        parts
            .iter()
            .map(|part| match part {
                crate::ir::LabelPart::Literal(value) => format!("\"{}\"", self.escape(value)),
                crate::ir::LabelPart::Expr(value) => {
                    if let Some(literal) = crepuscularity_core::eval::eval_literal(value) {
                        format!(
                            "\"{}\"",
                            self.escape(&crepuscularity_core::context::value_to_str(&literal))
                        )
                    } else {
                        self.model_text(value, scopes)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }

    pub(super) fn model_bool(&self, expr: &str, scopes: &[ScopeBinding]) -> String {
        self.model("bool", expr, scopes)
    }

    pub(super) fn model_number(&self, expr: &str, scopes: &[ScopeBinding]) -> String {
        self.model("number", expr, scopes)
    }

    pub(super) fn model_items(&self, expr: &str, scopes: &[ScopeBinding]) -> String {
        self.model("items", expr, scopes)
    }
}

pub(super) fn bool_literal(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

pub(super) fn indent_str(level: usize) -> std::borrow::Cow<'static, str> {
    /// 64 levels of four-space indent, sliced instead of rebuilt per call.
    const SPACES: &str = "                                                                                                                                                                                                                                                                ";
    let width = level * 4;
    if width <= SPACES.len() {
        std::borrow::Cow::Borrowed(&SPACES[..width])
    } else {
        std::borrow::Cow::Owned("    ".repeat(level))
    }
}

pub(crate) fn progress_values(value: f32, max: f32) -> (f32, f32) {
    if !max.is_finite() || max <= 0.0 {
        return (0.0, 1.0);
    }
    let value = if value.is_finite() {
        value.clamp(0.0, max)
    } else {
        0.0
    };
    (value, max)
}

pub(crate) fn slider_values(
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
) -> (f32, f32, f32, Option<f32>) {
    if !min.is_finite() || !max.is_finite() || min > max || !(max - min).is_finite() {
        return (0.0, 0.0, 100.0, None);
    }
    let value = if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    };
    let step = step.filter(|step| step.is_finite() && *step > 0.0 && *step <= max - min);
    (value, min, max, step)
}

pub(crate) fn meter_values(value: f32, min: f32, max: f32) -> (f32, f32, f32) {
    if !min.is_finite() || !max.is_finite() || min > max {
        return (0.0, 0.0, 1.0);
    }
    if min == max {
        return (meter_fraction(value, min, max), 0.0, 1.0);
    }
    let value = if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    };
    (value, min, max)
}

pub(super) fn meter_fraction(value: f32, min: f32, max: f32) -> f32 {
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max {
        return 0.0;
    }
    if min == max {
        return if value >= max { 1.0 } else { 0.0 };
    }
    ((f64::from(value) - f64::from(min)) / (f64::from(max) - f64::from(min))).clamp(0.0, 1.0) as f32
}

/// Drops NaN and infinities. Finite values, including sizing sentinels, stay.
pub(super) fn finite_opt(value: Option<f32>) -> Option<f32> {
    value.filter(|value| value.is_finite())
}

pub(super) fn nonnegative_opt(value: Option<f32>) -> Option<f32> {
    finite_opt(value).filter(|value| *value >= 0.0)
}

pub(super) fn text_line_limit(style: &crate::ir::ViewStyle) -> Option<i32> {
    if style.text_overflow.as_deref() == Some("truncate") {
        Some(1)
    } else {
        style.line_clamp.filter(|lines| *lines > 0)
    }
}

// Negative sizing sentinels must not become native lengths.
pub(super) fn vertical_divider_height(height: Option<f32>, max_height: Option<f32>) -> Option<f32> {
    match finite_opt(height) {
        Some(-1.0) => None,
        Some(-2.0) => Some(16.0),
        Some(height) if height >= 0.0 => Some(height),
        _ if finite_opt(max_height) == Some(-1.0) => None,
        _ => Some(16.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_identifiers_quote_keywords_and_avoid_underscore_only_names() {
        for syntax in [SWIFT, KOTLIN] {
            assert_eq!(syntax.identifier("class"), "`class`");
            assert_eq!(syntax.identifier("when"), "`when`");
            assert_eq!(syntax.identifier("_"), "`item`");
            assert_eq!(syntax.identifier("界"), "`item`");
            assert_eq!(syntax.identifier("my-item"), "`my_item`");
        }
    }

    #[test]
    fn lexical_scopes_select_nearest_alias_and_preserve_local_fallback() {
        for syntax in [SWIFT, KOTLIN] {
            let outer = syntax.loop_binding("row", &[]);
            let child = syntax.loop_binding("child", std::slice::from_ref(&outer));
            let mut scopes = vec![outer.clone(), child];
            assert!(syntax
                .scope_args(" row.title ", &scopes)
                .ends_with(&outer.variable));
            assert!(syntax
                .scope_args("title", &scopes)
                .ends_with(&scopes[1].variable));
            let shadow = syntax.loop_binding("row", &scopes);
            assert_ne!(shadow.variable, outer.variable);
            scopes.push(shadow.clone());
            assert!(syntax
                .binding_path("row.title", &scopes)
                .starts_with(&syntax.binding_path_var(&shadow.variable)));
            let compound = syntax.scope_args("row.title == child.title", &scopes);
            assert!(compound.contains("@crepus:lexical"));
            assert!(compound.contains(&shadow.variable));
            assert!(!compound.contains(&outer.variable));
            assert!(!syntax
                .scope_args("rowboat.title", &scopes)
                .contains("@crepus:lexical"));
        }
    }

    #[test]
    fn strings_escape_line_breaks_and_kotlin_interpolation() {
        let text = "line\n\r\t\"\\$name ${value}";
        assert_eq!(SWIFT.escape(text), "line\\n\\r\\t\\\"\\\\$name ${value}");
        assert_eq!(
            KOTLIN.escape(text),
            "line\\n\\r\\t\\\"\\\\\\$name \\${value}"
        );
    }

    #[test]
    fn control_characters_use_native_unicode_escapes() {
        assert_eq!(
            SWIFT.escape("\0\u{8}\u{c}\u{7f}\u{85}"),
            "\\u{0}\\u{8}\\u{c}\\u{7f}\\u{85}"
        );
        assert_eq!(
            KOTLIN.escape("\0\u{8}\u{c}\u{7f}\u{85}"),
            "\\u0000\\u0008\\u000c\\u007f\\u0085"
        );
    }
    #[test]
    fn meter_fraction_handles_direct_ir_and_extreme_values() {
        assert_eq!(super::meter_fraction(0.0, -f32::MAX, f32::MAX), 0.5);
        for (value, min, max) in [
            (f32::NAN, 0.0, 1.0),
            (0.0, f32::NEG_INFINITY, 1.0),
            (0.0, 0.0, f32::INFINITY),
            (0.0, 2.0, 1.0),
        ] {
            assert_eq!(super::meter_fraction(value, min, max), 0.0);
        }
    }
    #[test]
    fn html_meter_values_preserve_scale_and_native_degenerate_semantics() {
        for (value, min, max, expected) in [
            (-2.0, -5.0, 5.0, (-2.0, -5.0, 5.0)),
            (-9.0, -5.0, 5.0, (-5.0, -5.0, 5.0)),
            (9.0, -5.0, 5.0, (5.0, -5.0, 5.0)),
            (f32::NAN, -5.0, 5.0, (-5.0, -5.0, 5.0)),
            (1.0, f32::NAN, 5.0, (0.0, 0.0, 1.0)),
            (1.0, 5.0, -5.0, (0.0, 0.0, 1.0)),
            (4.0, 5.0, 5.0, (0.0, 0.0, 1.0)),
            (5.0, 5.0, 5.0, (1.0, 0.0, 1.0)),
            (0.0, -f32::MAX, f32::MAX, (0.0, -f32::MAX, f32::MAX)),
        ] {
            assert_eq!(super::meter_values(value, min, max), expected);
        }
    }
}
