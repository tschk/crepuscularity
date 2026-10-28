//! Shared expression rewriting for generated framework templates.

/// Whether `c` can start a JS identifier.
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

/// Whether `c` can continue a JS identifier.
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Separate template names from JS variables so loops cannot capture props or helpers.
#[derive(Clone)]
pub(crate) struct LocalBinding {
    name: String,
    pub(crate) variable: String,
}

impl LocalBinding {
    pub(crate) fn loop_item(name: &str, depth: usize) -> Self {
        Self {
            name: name.into(),
            variable: format!("_crepusItem{depth}"),
        }
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_continue)
}

pub(crate) fn is_binding_path(bind: &str) -> bool {
    let mut parts = bind.trim().split('.');
    let root = parts.next().unwrap_or_default();
    is_identifier(root)
        && !matches!(
            root,
            "true" | "false" | "null" | "none" | "None" | "Null" | "undefined"
        )
        && parts.all(|part| {
            is_identifier(part) || (!part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
        })
}

pub(crate) fn validate_collection_expr(
    expression: &str,
) -> Result<(), crepuscularity_core::CrepusError> {
    if is_binding_path(expression) {
        Ok(())
    } else {
        crepuscularity_core::eval::validate_expr(expression)
    }
}

fn is_computed_property(property: &str) -> bool {
    matches!(
        property,
        "len"
            | "length"
            | "upper"
            | "lower"
            | "trim"
            | "is_empty"
            | "empty"
            | "abs"
            | "floor"
            | "ceil"
            | "round"
    )
}

pub(crate) fn component_local_name<'a>(name: &'a str, imports: &[&str]) -> &'a str {
    if matches!(
        name,
        "Array"
            | "Object"
            | "globalThis"
            | "undefined"
            | "toArray"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "let"
            | "static"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "eval"
            | "arguments"
    ) || name.starts_with("__crepus")
        || imports.contains(&name)
    {
        "__CrepusComponent"
    } else {
        name
    }
}

pub(crate) fn expression_runtime(markup: &str) -> String {
    let mut runtime = String::new();
    if markup.contains("__crepusInputRef(") {
        runtime.push_str(r#"function __crepusInputRef(value: any, handlers: any, action?: string): (element: any) => void {
  let previous: any;
  return (element: any): void => {
    if (previous) previous.onchange = null;
    previous = element;
    if (!element) return;
    if (value !== undefined) {
      const next = globalThis.String(value ?? "");
      if (element.value !== next) element.value = next;
    }
    if (action !== undefined) element.onchange = (event: any) => __crepusOwnHandler(handlers, action)?.[action]?.(event);
  };
}
"#);
    }
    if markup.contains("__crepusSliderValue(") {
        runtime.push_str(r#"function __crepusSliderValue(value: any, min: number, max: number, fallback: number): number {
  return typeof value === "number" && globalThis.Number.isFinite(value)
    ? globalThis.Math.min(max, globalThis.Math.max(min, value)) : fallback;
}
"#);
    }
    if markup.contains("__crepusTab") {
        runtime.push_str(TABS_RUNTIME);
    }
    if markup.contains("__crepusOwnHandler(") || markup.contains("__crepusInputRef(") {
        runtime.push_str(r#"function __crepusOwnHandler(handlers: any, key: string): any {
  return handlers != null && globalThis.Object.prototype.hasOwnProperty.call(handlers, key) ? handlers : undefined;
}
"#);
    }
    if markup.contains("__crepusText(") {
        runtime.push_str(r#"function __crepusText(value: any): string {
  if (globalThis.Array.isArray(value)) return `[${value.length} items]`;
  return typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? globalThis.String(value) : "";
}
"#);
    }
    if markup.contains("__crepusProperty(") {
        runtime.push_str(PROPERTY_RUNTIME);
    }
    runtime
}

const TABS_RUNTIME: &str = r#"function __crepusTabIndex(value: any, values: string[], index: number): number {
  const selected = values.indexOf(value);
  return index === (selected < 0 ? 0 : selected) ? 0 : -1;
}

function __crepusTabsRef(element: any): void {
  if (!element) return;
  globalThis.queueMicrotask(() => {
    const list = globalThis.Array.from(element.children).find((child: any) => child.getAttribute("role") === "tablist") as any;
    if (!list) return;
    const tabs = globalThis.Array.from(list.children).filter((child: any) => child.getAttribute("role") === "tab") as any[];
    const panels = globalThis.Array.from(element.children).filter((child: any) => child.getAttribute("role") === "tabpanel") as any[];
    if (element.getAttribute("data-crepus-tabs-unbound") === "true") {
      // Frameworks can recreate refs during renders; keep selection on the DOM instance.
      const state = element.__crepusUnboundTabs ||= { index: 0, listening: false };
      if (!state.listening) {
        // Record selection before host callbacks can trigger a synchronous rerender.
        element.addEventListener("click", (event: any) => {
          const currentList = globalThis.Array.from(element.children).find((child: any) => child.getAttribute("role") === "tablist") as any;
          const tab = event.target?.closest?.('[role="tab"]');
          if (!tab || tab.parentElement !== currentList || tab.disabled) return;
          state.index = globalThis.Array.from(currentList.children).filter((child: any) => child.getAttribute("role") === "tab").indexOf(tab);
          __crepusTabsRef(element);
        }, true);
        state.listening = true;
      }
      state.index = globalThis.Number.isInteger(state.index) ? globalThis.Math.max(0, globalThis.Math.min(tabs.length - 1, state.index)) : 0;
      tabs.forEach((tab: any, index: number) => {
        tab.setAttribute("aria-selected", globalThis.String(index === state.index));
        tab.tabIndex = index === state.index ? 0 : -1;
      });
      panels.forEach((panel: any, index: number) => { panel.hidden = index !== state.index; });
    }
    const prefix = element.getAttribute("data-crepus-tabs-id") || "crepus-tabs-" + (globalThis.crypto?.randomUUID?.() ?? globalThis.Math.random().toString(36).slice(2));
    element.setAttribute("data-crepus-tabs-id", prefix);
    tabs.forEach((tab: any, index: number) => {
      const panel = panels[index];
      if (!panel) return;
      tab.id ||= `${prefix}-tab-${index}`;
      panel.id ||= `${prefix}-panel-${index}`;
      tab.setAttribute("aria-controls", panel.id);
      panel.setAttribute("aria-labelledby", tab.id);
    });
  });
}

function __crepusTabsKey(event: any): void {
  if (event.defaultPrevented || event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return;
  const list = event.currentTarget;
  const tabs = globalThis.Array.from(list.children).filter((child: any) => child.getAttribute("role") === "tab" && !child.disabled) as any[];
  const index = tabs.indexOf(event.target.closest('[role="tab"]'));
  if (index < 0) return;
  let next: number;
  switch (event.key) {
    case "ArrowLeft": next = (index + tabs.length - 1) % tabs.length; break;
    case "ArrowRight": next = (index + 1) % tabs.length; break;
    case "Home": next = 0; break;
    case "End": next = tabs.length - 1; break;
    default: return;
  }
  event.preventDefault();
  tabs[next].focus();
  tabs[next].click();
}
"#;

fn tab_values_json(tabs: &[crate::ir::TabItem]) -> String {
    serde_json::Value::Array(
        tabs.iter()
            .map(|tab| serde_json::Value::String(tab.value.clone()))
            .collect(),
    )
    .to_string()
}

pub(crate) fn tab_selected_expr(
    bind: &str,
    tabs: &[crate::ir::TabItem],
    index: usize,
    locals: &[LocalBinding],
    scope: &str,
) -> String {
    if bind.trim().is_empty() {
        return (index == 0).to_string();
    }
    format!(
        "({}.indexOf({}) === {index})",
        tab_values_json(tabs),
        binding_expr(bind, locals, scope)
    )
}

pub(crate) fn tab_index_expr(
    bind: &str,
    tabs: &[crate::ir::TabItem],
    index: usize,
    locals: &[LocalBinding],
    scope: &str,
) -> String {
    if bind.trim().is_empty() {
        return if index == 0 { "0" } else { "-1" }.to_string();
    }
    format!(
        "__crepusTabIndex({}, {}, {index})",
        binding_expr(bind, locals, scope),
        tab_values_json(tabs)
    )
}

pub(crate) fn text_expr(bind: &str, locals: &[LocalBinding], scope: &str) -> String {
    if let Some(parts) = crepuscularity_core::text_binding::decode_text_binding(bind) {
        return parts
            .iter()
            .map(|part| match part {
                crepuscularity_core::ast::TextPart::Literal(value) => {
                    serde_json::Value::String(value.clone())
                        .to_string()
                        .replace('{', "\\u007b")
                        .replace('}', "\\u007d")
                }
                crepuscularity_core::ast::TextPart::Expr(value) => {
                    format!("__crepusText({})", scope_expr(value, locals, scope))
                }
            })
            .collect::<Vec<_>>()
            .join(" + ");
    }
    format!("__crepusText({})", scope_expr(bind, locals, scope))
}

const PROPERTY_RUNTIME: &str = r#"function __crepusProperty(value: any, key: string): any {
  if (typeof value === "string") {
    switch (key) {
      case "len": case "length": return Array.from(value).length;
      case "upper": return value.toUpperCase();
      case "lower": return value.toLowerCase();
      case "trim": return value.replace(/^[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+|[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+$/g, "");
      case "is_empty": case "empty": return value.length === 0;
    }
  }
  if (Array.isArray(value)) {
    if (key === "len" || key === "length") return value.length;
    if (key === "is_empty" || key === "empty") return value.length === 0;
    return null;
  }
  if (typeof value === "number" && ["abs", "floor", "ceil", "round"].includes(key)) {
    if (!globalThis.Number.isFinite(value)) throw new globalThis.Error("non-finite numeric property result");
    if (key === "abs") return globalThis.Math.abs(value);
    // Rust rounds half-way values away from zero, unlike Math.round for negatives.
    const result = key === "floor" ? globalThis.Math.floor(value)
      : key === "ceil" ? globalThis.Math.ceil(value)
      : globalThis.Math.sign(value) * (globalThis.Math.floor(globalThis.Math.abs(value))
        + (globalThis.Math.abs(value) % 1 >= 0.5 ? 1 : 0));
    if (result < -9223372036854775808 || result >= 9223372036854775808) {
      throw new globalThis.Error("numeric property result out of integer range");
    }
    return result;
  }
  return value != null && typeof value === "object" && Object.prototype.hasOwnProperty.call(value, key) ? value[key] : null;
}
"#;

fn binding_expr(bind: &str, locals: &[LocalBinding], scope: &str) -> String {
    if !is_binding_path(bind) {
        return scope_expr(bind, locals, scope);
    }
    let mut parts = bind.trim().split('.');
    let root = parts.next().unwrap_or_default();
    let mut expr = if let Some(local) = locals.iter().rev().find(|local| local.name == root) {
        local.variable.clone()
    } else {
        format!(
            "{}?.{root}",
            scope.trim_end_matches('.').trim_end_matches('?')
        )
    };
    for part in parts {
        if is_computed_property(part) {
            expr = format!("__crepusProperty({expr}, \"{part}\")");
        } else if is_identifier(part) {
            expr.push_str(&format!("?.{part}"));
        } else {
            expr.push_str(&format!("?.[\"{part}\"]"));
        }
    }
    expr
}

pub(crate) fn label_expr(
    parts: &[crate::ir::LabelPart],
    locals: &[LocalBinding],
    scope: &str,
) -> Option<String> {
    (!parts.is_empty()).then(|| {
        parts
            .iter()
            .map(|part| match part {
                crate::ir::LabelPart::Literal(value) => serde_json::Value::String(value.clone())
                    .to_string()
                    .replace('{', "\\u007b")
                    .replace('}', "\\u007d"),
                crate::ir::LabelPart::Expr(value) => {
                    format!("__crepusText({})", binding_expr(value, locals, scope))
                }
            })
            .collect::<Vec<_>>()
            .join(" + ")
    })
}

pub(crate) fn bound_value_expr(bind: &str, locals: &[LocalBinding], scope: &str) -> Option<String> {
    (!bind.trim().is_empty()).then(|| format!("({} ?? \"\")", binding_expr(bind, locals, scope)))
}

pub(crate) fn checked_expr(
    bind: Option<&str>,
    snapshot: bool,
    locals: &[LocalBinding],
    scope: &str,
) -> String {
    match bind.filter(|binding| !binding.trim().is_empty()) {
        Some(binding) => format!(
            "((value = {snapshot}) => typeof value === \"boolean\" ? value : {snapshot})({})",
            binding_expr(binding, locals, scope)
        ),
        None => snapshot.to_string(),
    }
}

pub(crate) fn slider_value_expr(
    bind: Option<&str>,
    value: f32,
    min: f32,
    max: f32,
    locals: &[LocalBinding],
    scope: &str,
) -> String {
    match bind.filter(|binding| !binding.trim().is_empty()) {
        Some(binding) => format!(
            "__crepusSliderValue({}, {min}, {max}, {value})",
            binding_expr(binding, locals, scope)
        ),
        None => value.to_string(),
    }
}

pub(crate) fn property_access(object: &str, key: &str) -> String {
    let encoded_key = serde_json::Value::String(key.into());
    let own = format!("__crepusOwnHandler({object}, {encoded_key})?");
    if is_identifier(key) {
        format!("{own}.{key}")
    } else {
        format!("{own}.[{encoded_key}]")
    }
}

/// Rewrite a template expression string (e.g. `count > 0`, `user.name`,
/// `items.length`) into valid JS that reads from the component's `scope`
/// object, so `condition`/`bind` strings from the IR can be dropped straight
/// into a JSX expression container.
///
/// - String values use DSL escape semantics, including literal unknown escapes.
/// - Loop locals become generated aliases; other bare identifiers use `scope.`.
/// - JS literal keywords stay unchanged; DSL null aliases become `null`.
/// - Only path roots receive the scope prefix; property access guards nullish values.
/// - Numeric literals, including scientific notation, are preserved.
/// - `scope` is the access prefix (`scope.` or `scope().`).
/// - An empty/whitespace expression becomes `undefined`.
pub(crate) fn scope_expr(expr: &str, locals: &[LocalBinding], scope: &str) -> String {
    if expr.trim().is_empty() {
        return "undefined".to_string();
    }
    if is_binding_path(expr)
        && expr
            .trim()
            .split('.')
            .skip(1)
            .any(|part| part.bytes().all(|c| c.is_ascii_digit()))
    {
        return binding_expr(expr, locals, scope);
    }
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut primary_start = 0;
    let mut groups = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            primary_start = out.len();
            let quote = c;
            let start = i;
            let mut needs_encoding = false;
            i += 1;
            while i < chars.len() {
                let cc = chars[i];
                i += 1;
                if cc == '\\' && i < chars.len() {
                    let escaped = chars[i];
                    needs_encoding |= !matches!(escaped, 'n' | 't' | '\\') && escaped != quote;
                    i += 1;
                    continue;
                }
                needs_encoding |= cc.is_control();
                if cc == quote {
                    break;
                }
            }
            let literal: String = chars[start..i].iter().collect();
            if needs_encoding {
                if let Ok(crepuscularity_core::context::TemplateValue::Str(value)) =
                    crepuscularity_core::eval::eval_expr(
                        &literal,
                        &crepuscularity_core::TemplateContext::new(),
                    )
                {
                    if let Ok(encoded) = serde_json::to_string(&value) {
                        out.push_str(&encoded);
                        continue;
                    }
                }
            }
            out.push_str(&literal);
            continue;
        }
        if c == '-' && out.ends_with('-') {
            out.push(' ');
        }
        let signed_number = c == '-'
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
            && chars[..i]
                .iter()
                .rfind(|c| !c.is_whitespace())
                .is_none_or(|c| {
                    matches!(
                        c,
                        '(' | '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|'
                    )
                });
        if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
            || signed_number
        {
            primary_start = out.len();
            let start = i;
            if signed_number {
                i += 1;
            }
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            if chars.get(i) == Some(&'.')
                && (!chars[i + 1..]
                    .iter()
                    .find(|c| !c.is_whitespace())
                    .is_some_and(|c| is_ident_start(*c))
                    || (matches!(chars.get(i + 1), Some('e' | 'E'))
                        && chars
                            .get(i + 2)
                            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '+' | '-'))))
            {
                i += 1;
                while chars.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            if matches!(chars.get(i), Some('e' | 'E')) {
                let mut end = i + 1;
                if matches!(chars.get(end), Some('+' | '-')) {
                    end += 1;
                }
                let digits = end;
                while chars.get(end).is_some_and(char::is_ascii_digit) {
                    end += 1;
                }
                if end > digits {
                    i = end;
                }
            }
            out.extend(chars[start..i].iter().copied());
            continue;
        }
        if c == '.' {
            let mut start = i + 1;
            while chars.get(start).is_some_and(|c| c.is_whitespace()) {
                start += 1;
            }
            let mut end = start;
            while chars.get(end).is_some_and(|c| is_ident_continue(*c)) {
                end += 1;
            }
            let property: String = chars[start..end].iter().collect();
            if is_computed_property(&property) {
                let mut receiver = out.split_off(primary_start);
                if receiver.ends_with('?') {
                    receiver.pop();
                }
                out.push_str(&format!("__crepusProperty({receiver}, \"{property}\")"));
                i = end;
                continue;
            }
        }
        if c == '.'
            && !out.ends_with('?')
            && chars[i + 1..]
                .iter()
                .find(|c| !c.is_whitespace())
                .is_some_and(|c| is_ident_start(*c))
        {
            out.push_str("?.");
            i += 1;
            continue;
        }
        if is_ident_start(c) {
            let prev_is_dot = out.trim_end().ends_with('.');
            if !prev_is_dot {
                primary_start = out.len();
            }
            let start = i;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            if !prev_is_dot && matches!(ident.as_str(), "none" | "None" | "Null") {
                out.push_str("null");
            } else if prev_is_dot
                || matches!(ident.as_str(), "true" | "false" | "null" | "undefined")
            {
                out.push_str(&ident);
            } else if let Some(local) = locals.iter().rev().find(|local| local.name == ident) {
                out.push_str(&local.variable);
            } else {
                out.push_str(scope);
                out.push_str(&ident);
            }
            continue;
        }
        if c == '(' || c == '[' {
            let follows_primary = out
                .trim_end()
                .chars()
                .last()
                .is_some_and(|c| is_ident_continue(c) || matches!(c, ')' | ']' | '\'' | '"'));
            groups.push(if follows_primary {
                primary_start
            } else {
                out.len()
            });
        } else if matches!(c, ')' | ']') {
            if let Some(start) = groups.pop() {
                primary_start = start;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

pub(crate) const ROTATING_TEXT_RUNTIME: &str = r#"function __crepusRotate(node: HTMLElement, { phrases, interval }: { phrases: string[]; interval: number }) {
  if (phrases.length < 2) return;
  const motion = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)");
  let index = 0;
  let timer: ReturnType<typeof globalThis.setInterval> | undefined;
  const syncMotion = () => {
    if (motion?.matches) {
      if (timer !== undefined) globalThis.clearInterval(timer);
      timer = undefined;
      index = 0;
      node.textContent = phrases[0];
    } else if (timer === undefined) {
      timer = globalThis.setInterval(() => {
        index = (index + 1) % phrases.length;
        node.textContent = phrases[index];
      }, globalThis.Math.max(1, globalThis.Math.min(2147483647, interval)));
    }
  };
  syncMotion();
  motion?.addEventListener("change", syncMotion);
  return { destroy() {
    motion?.removeEventListener("change", syncMotion);
    if (timer !== undefined) globalThis.clearInterval(timer);
  } };
}
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn null_aliases_do_not_rewrite_strings_or_property_names() {
        assert_eq!(
            super::scope_expr("None == obj.None && 'none' == obj.Null", &[], "scope."),
            "null == scope.obj?.None && 'none' == scope.obj?.Null"
        );
        for alias in ["none", "None", "Null"] {
            assert!(!super::is_binding_path(alias));
        }
    }

    #[test]
    fn property_access_guards_nullish_primaries_without_changing_numbers() {
        for (source, expected) in [
            ("user . profile . name", "scope.user ?. profile ?. name"),
            ("(user.profile).name", "(scope.user?.profile)?.name"),
            ("user?.profile?.name", "scope.user?.profile?.name"),
            ("None.name", "null?.name"),
            (
                "'user.name'.length",
                "__crepusProperty('user.name', \"length\")",
            ),
            ("1.name", "1?.name"),
            ("1. name", "1?. name"),
            ("1.e3", "1.e3"),
            ("1.e-3", "1.e-3"),
            (".5", ".5"),
            ("1.", "1."),
        ] {
            assert_eq!(
                super::scope_expr(source, &[], "scope."),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn computed_property_receivers_preserve_primary_boundaries() {
        for (input, expected) in [
            (
                "name.upper.length",
                "__crepusProperty(__crepusProperty(scope.name, \"upper\"), \"length\")",
            ),
            (
                "(name + other).length",
                "__crepusProperty((scope.name + scope.other), \"length\")",
            ),
            (
                "name?.upper?.length",
                "__crepusProperty(__crepusProperty(scope.name, \"upper\"), \"length\")",
            ),
            ("-2.length", "__crepusProperty(-2, \"length\")"),
            (
                "(name.trim).upper",
                "__crepusProperty((__crepusProperty(scope.name, \"trim\")), \"upper\")",
            ),
            (
                "obj['name'].length",
                "__crepusProperty(scope.obj['name'], \"length\")",
            ),
        ] {
            assert_eq!(super::scope_expr(input, &[], "scope."), expected, "{input}");
        }
        assert_eq!(
            super::binding_expr("rows.0.name.length", &[], "scope."),
            "__crepusProperty(scope?.rows?.[\"0\"]?.name, \"length\")"
        );
    }

    #[test]
    fn numeric_properties_use_the_shared_runtime_in_expressions_and_bindings() {
        for property in ["abs", "floor", "ceil", "round"] {
            assert_eq!(
                super::scope_expr(&format!("amount.{property}"), &[], "scope."),
                format!("__crepusProperty(scope.amount, \"{property}\")")
            );
            assert_eq!(
                super::binding_expr(&format!("amount.{property}"), &[], "scope."),
                format!("__crepusProperty(scope?.amount, \"{property}\")")
            );
        }
        assert!(
            super::expression_runtime("__crepusProperty(amount, \"round\")")
                .contains("numeric property result out of integer range")
        );
        assert!(!super::expression_runtime("plain text").contains("function __crepusProperty"));
    }

    #[test]
    fn indexed_paths_preserve_numeric_keys_and_lexical_scope() {
        assert_eq!(
            super::scope_expr("rows.01.children", &[], "scope()."),
            "scope()?.rows?.[\"01\"]?.children"
        );
        let locals = [super::LocalBinding::loop_item("row", 0)];
        assert_eq!(
            super::scope_expr("row.0.children", &locals, "scope."),
            "_crepusItem0?.[\"0\"]?.children"
        );
    }

    #[test]
    fn absent_checked_bindings_keep_snapshot() {
        for bind in [None, Some(""), Some("  ")] {
            assert_eq!(super::checked_expr(bind, true, &[], "scope."), "true");
            assert_eq!(super::checked_expr(bind, false, &[], "scope."), "false");
        }
    }

    #[test]
    fn blank_value_bindings_remain_unbound() {
        for bind in ["", " ", "\n\t"] {
            assert_eq!(super::bound_value_expr(bind, &[], "scope."), None);
        }
    }

    #[test]
    fn checked_path_detection_preserves_expressions() {
        for expression in [
            "true",
            "false",
            "null",
            "undefined",
            "count > 0",
            "1.5",
            "user.enabled == true",
        ] {
            assert_eq!(
                super::binding_expr(expression, &[], "scope."),
                super::scope_expr(expression, &[], "scope.")
            );
        }
    }

    use super::{scope_expr, LocalBinding};

    #[test]
    fn action_property_access_preserves_exact_keys() {
        for key in [
            "go",
            "$go",
            "device.info",
            " go ",
            "",
            "__proto__",
            "constructor",
        ] {
            let access = super::property_access("handlers()", key);
            let encoded = serde_json::Value::String(key.into());
            assert!(access.starts_with(&format!("__crepusOwnHandler(handlers(), {encoded})?.")));
            assert_eq!(access.matches("handlers()").count(), 1);
        }
    }

    #[test]
    fn javascript_strings_keep_dsl_escape_and_control_character_values() {
        for literal in [
            r"'\q'",
            r"'\r'",
            r"'\u0061'",
            r"'\é'",
            "'line\nbreak'",
            r#""keep\'quote""#,
        ] {
            let crepuscularity_core::context::TemplateValue::Str(expected) =
                crepuscularity_core::eval::eval_expr(
                    literal,
                    &crepuscularity_core::TemplateContext::new(),
                )
                .unwrap()
            else {
                panic!("expected a string");
            };
            let emitted = scope_expr(literal, &[], "scope.");
            let actual: String = serde_json::from_str(&emitted).unwrap();
            assert_eq!(actual, expected, "{literal:?}");
        }
    }

    #[test]
    fn latest_local_alias_wins_without_rewriting_properties_or_strings() {
        let locals = [
            LocalBinding::loop_item("scope", 0),
            LocalBinding::loop_item("scope", 1),
        ];
        assert_eq!(
            scope_expr(
                "scope.label + data.scope + 'scope.label'",
                &locals,
                "scope."
            ),
            "_crepusItem1?.label + scope.data?.scope + 'scope.label'"
        );
        assert_eq!(
            super::binding_expr("scope.label", &locals, "scope."),
            "_crepusItem1?.label"
        );
    }

    #[test]
    fn numeric_tokens_do_not_change_identifier_or_string_scoping() {
        for prefix in ["scope.", "scope()."] {
            assert_eq!(
                scope_expr(
                    r#"1e3 + e3 + user.e3 + item.e3 + "1e3""#,
                    &[LocalBinding::loop_item("item", 0)],
                    prefix
                ),
                format!(r#"1e3 + {prefix}e3 + {prefix}user?.e3 + _crepusItem0?.e3 + "1e3""#),
            );
        }
    }
}
