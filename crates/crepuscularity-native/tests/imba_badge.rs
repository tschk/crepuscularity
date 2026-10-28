use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::json;

#[test]
fn badge_preserves_literal_content_tone_and_identity() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"badge", "label":"Badge {name}\"\\/\nNext", "tone":"tone{raw}\"\\/\n",
        "style":{"id":"badge-id","classes":["rounded","px-2"],"ariaLabel":"Badge label"}
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    let badge = output
        .source
        .lines()
        .find(|line| line.contains("<span"))
        .unwrap();
    for attribute in [
        "id=(\"badge-id\")",
        "className=(\"rounded px-2\")",
        "aria-label=(\"Badge label\")",
        r#"data-tone=("tone\u007braw\u007d\"\\/\n")"#,
    ] {
        assert!(badge.contains(attribute), "{badge}");
    }
    assert!(
        badge.contains(r#"> "Badge \u007bname\u007d\"\\/\nNext""#),
        "{badge}"
    );
    assert!(!output.bindings.contains("export function e0"));
}

#[test]
fn absent_badge_tone_does_not_emit_an_empty_tone_attribute() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"badge", "label":"Plain"
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(
        output.source.contains("<span> \"Plain\""),
        "{}",
        output.source
    );
    assert!(!output.source.contains("data-tone"));
}
