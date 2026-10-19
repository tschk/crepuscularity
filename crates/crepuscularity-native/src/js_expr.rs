/// Whether `c` can start a JS identifier.
pub(crate) fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

/// Whether `c` can continue a JS identifier.
pub(crate) fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Rewrite a template expression string (e.g. `count > 0`, `user.name`,
/// `items.length`) into valid JS that reads from the component's scope
/// object, so `condition`/`bind` strings from the IR can be dropped straight
/// into a JSX expression container.
///
/// - String literals (single/double quoted) are left untouched.
/// - Bare identifiers are prefixed with `prefix`, except those in `locals`
///   (real JS variables from an enclosing `ForEach`) and JS literal keywords
///   (`true`, `false`, `null`, `undefined`).
/// - Only the first segment of a dotted path is prefixed: `user.name` becomes
///   `scope.user.name`, never `scope.user.scope.name`.
/// - An empty/whitespace expression becomes `undefined`.
pub(crate) fn scope_expr(expr: &str, locals: &[String], prefix: &str) -> String {
    if expr.trim().is_empty() {
        return "undefined".to_string();
    }
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                let cc = chars[i];
                out.push(cc);
                i += 1;
                if cc == '\\' && i < chars.len() {
                    out.push(chars[i]);
                    i += 1;
                    continue;
                }
                if cc == quote {
                    break;
                }
            }
            continue;
        }
        if is_ident_start(c) {
            let prev_is_dot = out.trim_end().ends_with('.');
            let start = i;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            if prev_is_dot
                || locals.iter().any(|l| l == &ident)
                || matches!(ident.as_str(), "true" | "false" | "null" | "undefined")
            {
                out.push_str(&ident);
            } else {
                out.push_str(prefix);
                out.push_str(&ident);
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}
