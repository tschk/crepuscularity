/// The handful of token differences between the SwiftUI and Compose emitters:
/// the state-store expression, the keyword-argument separator, and the null
/// literal, and string interpolation marker.
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
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        if self.escape_dollar {
            escaped.replace('$', "\\$")
        } else {
            escaped
        }
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
        if out.is_empty() {
            "item".to_string()
        } else {
            out
        }
    }

    pub(super) fn scope_args(&self, scope_name: Option<&str>, scope_var: Option<&str>) -> String {
        match (scope_name, scope_var) {
            (Some(scope_name), Some(scope_var)) => {
                let sep = self.kw_sep;
                format!(
                    ", scopeName{sep}\"{}\", scope{sep}{}",
                    self.escape(scope_name),
                    scope_var
                )
            }
            _ => String::new(),
        }
    }

    fn model(
        &self,
        accessor: &str,
        expr: &str,
        scope_name: Option<&str>,
        scope_var: Option<&str>,
    ) -> String {
        format!(
            "{}.{accessor}(\"{}\"{})",
            self.store,
            self.escape(expr),
            self.scope_args(scope_name, scope_var)
        )
    }

    pub(super) fn model_text(
        &self,
        expr: &str,
        scope_name: Option<&str>,
        scope_var: Option<&str>,
    ) -> String {
        self.model("text", expr, scope_name, scope_var)
    }

    pub(super) fn model_bool(
        &self,
        expr: &str,
        scope_name: Option<&str>,
        scope_var: Option<&str>,
    ) -> String {
        self.model("bool", expr, scope_name, scope_var)
    }

    pub(super) fn model_number(
        &self,
        expr: &str,
        scope_name: Option<&str>,
        scope_var: Option<&str>,
    ) -> String {
        self.model("number", expr, scope_name, scope_var)
    }

    pub(super) fn model_items(
        &self,
        expr: &str,
        scope_name: Option<&str>,
        scope_var: Option<&str>,
    ) -> String {
        self.model("items", expr, scope_name, scope_var)
    }
}

#[cfg(test)]
mod tests {
    use super::{KOTLIN, SWIFT};

    #[test]
    fn kotlin_strings_keep_interpolation_markers_literal() {
        for (input, expected) in [
            ("Hello Ada", "Hello Ada"),
            ("$name", "\\$name"),
            ("${run { 7 }}", "\\${run { 7 }}"),
            ("$$name", "\\$\\$name"),
            ("$Δ", "\\$Δ"),
            (r"\$name", r"\\\$name"),
            (r"\u0024name", r"\\u0024name"),
            ("\"quote\"\\", "\\\"quote\\\"\\\\"),
        ] {
            assert_eq!(KOTLIN.escape(input), expected);
        }
    }

    #[test]
    fn swift_strings_preserve_dollars_and_existing_escaping() {
        assert_eq!(
            SWIFT.escape(r#"$name ${value} \"quoted\""#),
            r#"$name ${value} \\\"quoted\\\""#
        );
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
