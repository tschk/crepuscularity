extern crate crepuscularity_gpui_kit as gpui;

use gpui::prelude::*;

#[test]
fn template_output_uses_kit_types() {
    let heading = "Kit template";
    let visible = true;
    let element = view!(
        r#"
div flex flex-col gap-2 p-4 class:bg-white={visible}
  "{heading}"
  if {visible}
    "Visible"
  else
    "Hidden"
  for item in {0..3}
    "Row {item}"
  match {visible}
    {true} =>
      "Enabled"
    {false} =>
      "Disabled"
"#
    );

    // This annotation checks the actual upstream type, not just this facade's
    // re-export. A CE element cannot satisfy it.
    let _: gpui::kit::AnyElement = element.into_any_element();
}

#[test]
fn raw_expression_keeps_native_kit_element_type() {
    let child = gpui::kit::div().child("Native child");
    let element = view!(
        r#"
div p-2
  {child}
"#
    );
    let _: gpui::kit::AnyElement = element.into_any_element();
}
