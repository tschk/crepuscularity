//! Integration tests for layout, colors, template vars, hit testing, and include security.

use std::fs;
use std::path::PathBuf;

use crepuscularity_core::context::TemplateContext;
use sha2::{Digest, Sha256};

use crate::{
    layout_tree, lookup_named_color, parse_classes, parse_hex, render_template_to_framebuffer,
    EmbeddedDocument, EmbeddedNode, PanelConfig, Rect, Rgb565View, Rgb888Buffer, ScreenSize,
    Template, Ui,
};

fn screen() -> ScreenSize {
    ScreenSize::new(128, 64)
}

fn render(template: &str, ctx: &TemplateContext) -> (Rgb888Buffer, EmbeddedDocument) {
    let screen = screen();
    let mut fb = Rgb888Buffer::new(screen, crate::DEFAULT_BG);
    let doc = render_template_to_framebuffer(template, ctx, screen, &mut fb).expect("render");
    (fb, doc)
}

#[test]
fn layout_flex_col_assigns_vertical_bounds() {
    let mut root = EmbeddedNode {
        id: None,
        tag: "motion".into(),
        text: None,
        on_click: None,
        style: parse_classes(&[
            "flex-col".into(),
            "w-full".into(),
            "h-full".into(),
            "gap-4".into(),
        ]),
        bounds: Default::default(),
        children: vec![
            EmbeddedNode {
                id: None,
                tag: "text".into(),
                text: Some("A".into()),
                on_click: None,
                style: parse_classes(&["h-[8]".into()]),
                bounds: Default::default(),
                children: vec![],
            },
            EmbeddedNode {
                id: None,
                tag: "text".into(),
                text: Some("B".into()),
                on_click: None,
                style: parse_classes(&["h-[8]".into()]),
                bounds: Default::default(),
                children: vec![],
            },
        ],
    };
    layout_tree(&mut root, screen());
    assert_eq!(root.bounds.h, 64);
    assert!(root.children[0].bounds.y < root.children[1].bounds.y);
}

#[test]
fn template_vars_embedded_target() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "device");
    let tpl = r#"motionless
  if {is_embedded && crepus_target == "embedded"}
    div
      "Hello {name}""#;
    render(tpl, &ctx);
}

#[test]
fn screen_dimensions_injected() {
    let ctx = TemplateContext::new();
    let tpl = r#"motionless
  if {screen_width == 128 && screen_height == 64}
    div
      "ok""#;
    render(tpl, &ctx);
}

#[test]
fn colors_named_and_hex() {
    assert!(lookup_named_color("zinc-900").is_some());
    assert!(lookup_named_color("red-500").is_some());
    assert_eq!(parse_hex("#ff00aa").map(|c| c.to_u32()), Some(0xff00aa));
    let style = parse_classes(&["bg-zinc-900".into(), "text-green-500".into()]);
    assert!(style.bg.is_some());
    assert!(style.text.is_some());
}

#[test]
fn hit_test_deepest_id() {
    let doc = EmbeddedDocument::new(
        vec![EmbeddedNode {
            id: Some("root".into()),
            tag: "motion".into(),
            text: None,
            on_click: None,
            style: parse_classes(&[]),
            bounds: Rect::new(0, 0, 100, 100),
            children: vec![EmbeddedNode {
                id: Some("inner".into()),
                tag: "text".into(),
                text: Some("x".into()),
                on_click: None,
                style: parse_classes(&[]),
                bounds: Rect::new(10, 10, 20, 20),
                children: vec![],
            }],
        }],
        screen(),
    );
    assert_eq!(doc.hit_test(15, 15), Some("inner"));
    assert_eq!(doc.hit_test(1, 1), Some("root"));
}

#[test]
fn include_path_traversal_rejected() {
    let err = crepuscularity_core::resolve_include_path(None, "../secret.crepus").unwrap_err();
    assert!(
        err.to_string().contains("include path outside base dir"),
        "{err}"
    );
}

#[test]
fn include_absolute_path_rejected() {
    let err = crepuscularity_core::resolve_include_path(None, "/etc/passwd").unwrap_err();
    assert!(
        err.to_string().contains("include path outside base dir"),
        "{err}"
    );
}

#[test]
fn include_valid_file_renders() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/embedded-test-includes");
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join("child.crepus"), "motionless\n  \"included\"").unwrap();
    fs::write(
        dir.join("parent.crepus"),
        "motionless\ninclude child.crepus",
    )
    .unwrap();

    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.clone());
    let screen = screen();
    let mut fb = Rgb888Buffer::new(screen, crate::DEFAULT_BG);
    render_template_to_framebuffer(
        &fs::read_to_string(dir.join("parent.crepus")).unwrap(),
        &ctx,
        screen,
        &mut fb,
    )
    .expect("include render");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn framebuffer_snapshot_hash() {
    let tpl = r#"div flex flex-col w-full h-full bg-zinc-900 p-4 gap-2
  span text-green-400
    "OK""#;
    let (fb, _) = render(tpl, &TemplateContext::new());
    let bytes = fb.as_rgb888_bytes();
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, "9bc07418fa8b520aeb3692011d06354d75128b0ce5c1f4b5790adce3cf7c28a4",
        "update hash if render output intentionally changed"
    );
}

#[test]
fn framebuffer_writes_ppm() {
    let tpl = r#"motionless bg-zinc-900 w-full h-full
  "x""#;
    let (fb, _) = render(tpl, &TemplateContext::new());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/embedded-ppm-test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("snap.ppm");
    fb.write_ppm(&path).expect("ppm");
    let head = fs::read(&path).expect("read ppm");
    assert!(head.starts_with(b"P6\n128 64\n255\n"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn framebuffer_writes_ppm_error() {
    let tpl = r#"motionless bg-zinc-900 w-full h-full
  "x""#;
    let (fb, _) = render(tpl, &TemplateContext::new());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("non_existent_dir/snap.ppm");
    let err = crate::write_ppm(&path, &fb).unwrap_err();
    assert!(err.starts_with("create "));
}

#[test]
fn ui_one_shot_render() {
    let mut ui = Ui::new(
        64,
        32,
        r#"motion w-full h-full bg-zinc-900
  span #tag text-white
    "hi""#,
    );
    ui.render().expect("render");
    assert!(ui.rgb565().len() == 64 * 32 * 2);
    assert!(ui.document().unwrap().node_by_id("tag").is_some());
}

#[test]
fn parse_cache_skips_reparse_on_set() {
    let tpl = r#"div w-full h-full
  span
    "{n}""#;
    let mut ui = Ui::new(64, 32, tpl);
    ui.render().expect("first");
    ui.set("n", "b");
    ui.render().expect("second");
    assert_eq!(ui.template().context().get_str("n"), "b");
}

#[test]
fn panel_bgr_encode_swaps_bytes() {
    let mut scratch = Vec::new();
    let src = [0x12u8, 0x34];
    let out = crate::swap_rgb565_bytes_bgr(&src, &mut scratch);
    assert_eq!(out, [0x34, 0x12]);
}

#[test]
fn mock_display_flush() {
    struct Mock;
    impl crate::Rgb565Display for Mock {
        fn screen_size(&self) -> ScreenSize {
            ScreenSize::new(2, 2)
        }
        fn flush_rgb565_rect(
            &mut self,
            _x: u16,
            _y: u16,
            w: u16,
            h: u16,
            pixels: &[u8],
        ) -> Result<(), crate::DisplayError> {
            assert_eq!(pixels.len(), (w as usize) * (h as usize) * 2);
            Ok(())
        }
    }
    let mut ui = Ui::new(2, 2, "div w-full h-full bg-zinc-900").panel(PanelConfig::default());
    let mut mock = Mock;
    ui.flush(&mut mock).expect("flush");
}

#[test]
fn ui_macro_sets_vars() {
    let ui = crate::ui!(r#"motion w-full h-full"#, 32, 16, "n" => "x");
    assert_eq!(ui.template().context().get_str("n"), "x");
}

#[test]
fn template_draw_into_rgb565_view() {
    let screen = ScreenSize::new(64, 32);
    let mut ram = vec![0u16; screen.pixel_count()];
    let mut view = Rgb565View::new(screen, &mut ram).expect("view");
    let mut ui = Template::from_source(
        r#"div w-full h-full bg-zinc-900
  span #tag text-white
    "hi""#,
        screen,
    );
    let doc = ui.draw(&mut view).expect("draw");
    assert_eq!(doc.node_at(2, 2), Some("tag"));
    assert!(view.as_bytes().iter().any(|&b| b != 0));
}

#[test]
fn button_collects_on_click_id() {
    let tpl = r#"button #submit @click="handle_submit"
  "OK""#;
    let (_fb, doc) = render(tpl, &TemplateContext::new());
    let node = doc.node_by_id("submit").expect("submit id");
    assert_eq!(node.on_click.as_deref(), Some("handle_submit"));
}

#[test]
fn template_reload_error_paths() {
    // Test 1: Template created without path
    let mut tpl_no_path = Template::from_source("div", screen());
    let err = tpl_no_path.reload().unwrap_err();
    assert_eq!(err, "template has no path; reload requires `from_path`");

    // Test 2: Template with non-existent path
    let path = PathBuf::from("does_not_exist_xyz.crepus");
    let mut tpl_bad_path = Template::from_source_with_path("div", &path, screen());
    let err = tpl_bad_path.reload().unwrap_err();
    assert!(err.starts_with(&format!("template error: {:?}", path)));
}

#[test]
fn template_function_error_path() {
    let path = PathBuf::from("does_not_exist_abc.crepus");
    let res = crate::template(&path, screen());
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert!(err.starts_with(&format!("template error: {:?}", path)));
}
