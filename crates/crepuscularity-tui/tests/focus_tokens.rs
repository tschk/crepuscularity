use crepuscularity_core::parse_template;
use crepuscularity_tui::{collect_focusable_ids, render::collect_focusable_ids_from_nodes};

#[test]
fn public_focus_collectors_require_a_focusable_token_not_a_substring() {
    let source = "button #prefix not-focusable\nbutton #suffix focusable-extra\nbutton #middle prefixfocusablesuffix\nbutton #real focusable\nbutton #sibling focusable border";
    let nodes = parse_template(source).unwrap();
    assert_eq!(collect_focusable_ids(source).unwrap(), ["real", "sibling"]);
    assert_eq!(
        collect_focusable_ids_from_nodes(&nodes),
        ["real", "sibling"]
    );
}

#[test]
fn public_focus_collectors_keep_syntactic_conditional_branches() {
    let source = "if enabled\n  button #then focusable\nelse\n  button #else focusable\nfor item in missing\n  button #loop focusable";
    let nodes = parse_template(source).unwrap();
    assert_eq!(
        collect_focusable_ids(source).unwrap(),
        ["then", "else", "loop"]
    );
    assert_eq!(
        collect_focusable_ids_from_nodes(&nodes),
        ["then", "else", "loop"]
    );
}

#[test]
fn public_focus_collectors_resolve_static_target_tokens_without_expressions() {
    let platform = std::env::consts::OS;
    let source = format!(
        r#"<div>
<button id="plain" class="border focusable" />
<button id="terminal" class="tui:focusable" />
<button id="platform" class="{platform}:focusable" />
<button id="web" class="web:focusable" />
<button id="unknown" class="unknown:focusable" />
<button id="substring" class="tui:not-focusable" />
<button id="dynamic" class={{classes}} />
<button id="interpolated" class="not-{{focusable}}" />
<button id="conditional" class:focusable={{1 +}} />
</div>"#
    );
    let mut nodes = parse_template(&source).unwrap();
    let mut expected = vec!["plain", "terminal"];
    if matches!(platform, "macos" | "windows" | "linux") {
        expected.push("platform");
    }
    assert_eq!(collect_focusable_ids(&source).unwrap(), expected);
    assert_eq!(collect_focusable_ids_from_nodes(&nodes), expected);
    // The AST API also accepts a whitespace-containing static class entry.
    let crepuscularity_core::ast::Node::Element(root) = &mut nodes[0] else {
        panic!("fixture root is not an element");
    };
    let crepuscularity_core::ast::Node::Element(button) = &mut root.children[0] else {
        panic!("fixture child is not a button");
    };
    button.classes = vec!["border\tfocusable\ntext-red-500".into()];
    assert_eq!(collect_focusable_ids_from_nodes(&nodes), expected);
}
