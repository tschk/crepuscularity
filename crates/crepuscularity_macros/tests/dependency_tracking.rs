use std::path::{Path, PathBuf};
use std::process::Command;

fn embedded_fixture(label: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "crepus-embedded-template-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("Cargo.toml"), format!(
        "[package]\nname='macro-dependency-fixture'\nversion='0.0.0'\nedition='2021'\n[dependencies]\ncrepuscularity_macros={{path={:?}}}\n", env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    Fixture(root)
}

#[test]
fn embedded_template_paths_preserve_source_dependencies_and_validation() {
    let fixture = embedded_fixture("valid");
    let source = "span \"Snow 雪\"\n";
    let template = fixture.0.join("valid file.crepus");
    std::fs::write(&template, source).unwrap();
    std::fs::write(fixture.0.join("src/main.rs"), format!(
        "fn main(){{let relative=crepuscularity_macros::embedded_template!(\"valid file.crepus\");let absolute=crepuscularity_macros::embedded_template!({:?});assert_eq!(relative,absolute);print!(\"{{}}\",absolute);}}", template.to_str().unwrap()
    )).unwrap();
    assert_eq!(build_and_run(&fixture.0), source);
    let updated = "span \"Changed template-only content\"\n";
    std::fs::write(&template, updated).unwrap();
    assert_eq!(build_and_run(&fixture.0), updated);
    std::fs::write(&template, "if").unwrap();
    std::fs::write(
        fixture.0.join("src/main.rs"),
        format!(
            "fn main(){{let _=crepuscularity_macros::embedded_template!({:?});}}",
            template.to_str().unwrap()
        ),
    )
    .unwrap();
    let output = build(&fixture.0);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("embedded_template!: invalid .crepus"),
        "{error}"
    );
    assert!(error.contains("requires arguments"), "{error}");
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO"))
        .args(["build", "--offline", "--quiet"])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap()
}

fn build_and_run(root: &Path) -> String {
    let build = build(root);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let executable = root.join("target/debug").join(format!(
        "macro-dependency-fixture{}",
        std::env::consts::EXE_SUFFIX
    ));
    let run = Command::new(executable).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8(run.stdout).unwrap()
}

#[test]
fn cargo_rebuilds_views_and_accessors_after_template_only_edits() {
    let root = std::env::temp_dir().join(format!(
        "crepus-macro-dependencies-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    let root = &fixture.0;
    std::fs::create_dir(root.join("src")).unwrap();
    let manifest = format!("[package]\nname = \"macro-dependency-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[dependencies]\ncrepuscularity_macros = {{ path = {:?} }}\n", env!("CARGO_MANIFEST_DIR"));
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    std::fs::write(root.join("src/main.rs"), r#"
extern crate self as crepuscularity_web;
extern crate self as crepuscularity_tui;
pub mod dom {
    pub struct StaticElementRef { pub id: &'static str }
    impl StaticElementRef { pub const fn new(id: &'static str) -> Self { Self { id } } }
}
pub struct ElementRef { pub content: String }
impl ElementRef { pub fn new(id: &str) -> Self { Self { content: id.into() } } }
pub type TemplateValue = String;
pub struct Template { context: TemplateContext }
pub struct TemplateContext { pub base_dir: Option<std::path::PathBuf> }
impl Template {
    pub fn from_source_with_path(_: &str, _: &str) -> Self { Self { context: TemplateContext { base_dir: None } } }
    pub fn context_mut(&mut self) -> &mut TemplateContext { &mut self.context }
    pub fn set(&mut self, _: impl Into<String>, _: impl Into<TemplateValue>) {}
    pub fn draw(&self, _: &mut ratatui::Frame, _: ratatui::layout::Rect) -> Result<(), String> { Ok(()) }
    pub fn draw_full(&self, _: &mut ratatui::Frame) -> Result<(), String> { Ok(()) }
}
pub mod ratatui { pub struct Frame; pub mod layout { pub struct Rect; } }
fn main() {
    let view = crepuscularity_macros::view_file!("entry.crepus");
    let web = crepuscularity_macros::crepus_refs!("refs.crepus");
    let tui = crepuscularity_macros::template_refs!("refs.crepus").unwrap();
    println!("{}|{}|{}", view, std::mem::size_of_val(&web), std::mem::size_of_val(&tui));
}
"#).unwrap();
    std::fs::write(root.join("entry.crepus"), "include child.crepus").unwrap();
    std::fs::write(root.join("child.crepus"), "include leaf.crepus").unwrap();
    std::fs::write(root.join("leaf.crepus"), "\"leaf-A\"").unwrap();
    std::fs::write(root.join("refs.crepus"), "include refs-child.crepus#Card").unwrap();
    #[cfg(unix)]
    {
        std::fs::write(root.join("refs-target.crepus"), "--- Card\nspan #first").unwrap();
        std::os::unix::fs::symlink("refs-target.crepus", root.join("refs-child.crepus")).unwrap();
    }
    #[cfg(not(unix))]
    std::fs::write(root.join("refs-child.crepus"), "--- Card\nspan #first").unwrap();
    let original_main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    let initial = build_and_run(root);
    assert!(initial.starts_with("leaf-A|"), "{initial}");

    // No Rust source is edited after the initial consumer build.
    std::fs::write(root.join("entry.crepus"), "\"entry-B\"").unwrap();
    assert!(build_and_run(root).starts_with("entry-B|"));
    std::fs::write(root.join("entry.crepus"), "include child.crepus").unwrap();
    assert_eq!(build_and_run(root), initial);
    std::fs::write(root.join("leaf.crepus"), "\"leaf-C\"").unwrap();
    assert!(build_and_run(root).starts_with("leaf-C|"));
    std::fs::write(root.join("child.crepus"), "\"child-D\"").unwrap();
    assert!(build_and_run(root).starts_with("child-D|"));

    std::fs::write(
        root.join("refs-child.crepus"),
        "--- Card\nspan #first\nspan #second",
    )
    .unwrap();
    let updated = build_and_run(root);
    let initial: Vec<_> = initial.trim().split('|').collect();
    let updated: Vec<_> = updated.trim().split('|').collect();
    assert!(updated[1].parse::<usize>().unwrap() > initial[1].parse::<usize>().unwrap());
    assert!(updated[2].parse::<usize>().unwrap() > initial[2].parse::<usize>().unwrap());

    // Cargo canonicalizes dependencies, so retargeting a symlink alone requires a forced rebuild.

    // A conflicting caller binding must never silently replace a supplied prop.
    std::fs::write(
        root.join("src/main.rs"),
        r#"
fn main() {
    let title = "caller-title";
    let view = crepuscularity_macros::view_file!("entry.crepus");
    println!("{}", view);
}
"#,
    )
    .unwrap();
    for (entry, child, source) in [
        (
            "include child.crepus title=\"prop-title\"",
            "\"{title}\"",
            "entry.crepus",
        ),
        ("include wrapper.crepus", "\"{title}\"", "wrapper.crepus"),
        (
            "include child.crepus#Card title={title}",
            "--- Card\n\"{title}\"",
            "entry.crepus",
        ),
    ] {
        std::fs::write(root.join("entry.crepus"), entry).unwrap();
        std::fs::write(root.join("child.crepus"), child).unwrap();
        std::fs::write(
            root.join("wrapper.crepus"),
            "include child.crepus title={title}",
        )
        .unwrap();
        let output = build(root);
        assert!(
            !output.status.success(),
            "unsupported props compiled successfully"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("view_file! does not support include props"),
            "{error}"
        );
        assert!(error.contains(source), "{error}");
        assert!(error.contains("child.crepus"), "{error}");
    }
    let defaults = "+++\n[Card.defaults]\ntitle = \"default-title\"\n+++\n--- Card\n\"{title}\"\n--- Plain\n\"{title}\"";
    std::fs::write(root.join("child.crepus"), defaults).unwrap();
    std::fs::write(root.join("entry.crepus"), "include child.crepus#Card").unwrap();
    for path in ["entry.crepus", "child.crepus#Card"] {
        let main = format!(
            r#"fn main() {{
    let title = "caller-title";
    let view = crepuscularity_macros::view_file!({path:?});
    println!("{{}}", view);
}}"#
        );
        std::fs::write(root.join("src/main.rs"), main).unwrap();
        let output = build(root);
        assert!(
            !output.status.success(),
            "unsupported defaults compiled successfully"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("view_file! does not support defaults for component `Card`"),
            "{error}"
        );
    }
    // Unselected defaults do not affect a prop-free component using caller scope.
    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    let title = "caller-title";
    println!("{}", crepuscularity_macros::view_file!("child.crepus#Plain"));
}"#,
    )
    .unwrap();
    assert_eq!(build_and_run(root).trim(), "caller-title");
    // Accessor macros leave props/default evaluation to their runtime renderer.
    std::fs::write(root.join("src/main.rs"), original_main).unwrap();
    std::fs::write(root.join("entry.crepus"), "\"prop-free\"").unwrap();
    std::fs::write(
        root.join("refs.crepus"),
        "include refs-child.crepus#Card title=\"prop-title\"\n  span #provided",
    )
    .unwrap();
    std::fs::write(
        root.join("refs-child.crepus"),
        "+++\n[Card.defaults]\ntitle = \"default-title\"\n+++\n--- Card\nspan #first",
    )
    .unwrap();
    assert!(build_and_run(root).starts_with("prop-free|"));
    for (entry, child, source) in [
        (
            "include child.crepus\n  \"provided\"",
            "\"fallback\"",
            "entry.crepus",
        ),
        ("include wrapper.crepus", "\"fallback\"", "wrapper.crepus"),
        (
            "include child.crepus#Card\n  \"provided\"",
            "--- Card\n\"fallback\"",
            "entry.crepus",
        ),
    ] {
        std::fs::write(root.join("entry.crepus"), entry).unwrap();
        std::fs::write(root.join("child.crepus"), child).unwrap();
        std::fs::write(
            root.join("wrapper.crepus"),
            "include child.crepus\n  # comment\n\n  \"provided\"",
        )
        .unwrap();
        let output = build(root);
        assert!(
            !output.status.success(),
            "unsupported include slot compiled successfully"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("view_file! does not support authored include slots"),
            "{error}"
        );
        assert!(error.contains(source), "{error}");
    }
}
