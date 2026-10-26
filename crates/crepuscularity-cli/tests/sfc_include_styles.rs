use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn build(site: &Path, target: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", "Entry.crepus"])
        .output()
        .unwrap()
}

#[test]
fn reachable_sfc_styles_fail_before_publishing_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["vue", "svelte"] {
            for style in [
                "<style>p { color: red }</style>",
                "<style scoped>p { color: red }</style>",
                "<style></style>",
            ] {
                for existing_output in [false, true] {
                    let site = tempfile::tempdir().unwrap();
                    fs::create_dir_all(site.path().join("parts/nested")).unwrap();
                    fs::write(
                        site.path().join("Entry.crepus"),
                        "div\n  include parts/middle.crepus",
                    )
                    .unwrap();
                    fs::write(
                        site.path().join("parts/middle.crepus"),
                        format!("div\n  include nested/Child.{extension}"),
                    )
                    .unwrap();
                    let body = if extension == "vue" {
                        "<template><p>Styled child</p></template>"
                    } else {
                        "<p>Styled child</p>"
                    };
                    fs::write(
                        site.path().join(format!("parts/nested/Child.{extension}")),
                        format!("{body}{style}"),
                    )
                    .unwrap();
                    let dist = site.path().join("dist");
                    let sentinel = dist.join("previous-output.txt");
                    if existing_output {
                        fs::create_dir(&dist).unwrap();
                        fs::write(&sentinel, b"previous output\0unchanged").unwrap();
                    }

                    let result = build(site.path(), target);
                    let diagnostic = String::from_utf8_lossy(&result.stderr);
                    assert!(
                        !result.status.success(),
                        "{target} accepted reachable {extension} {style}"
                    );
                    assert!(
                        diagnostic.contains(&format!("Child.{extension}")),
                        "{diagnostic}"
                    );
                    assert!(diagnostic.contains("styles"), "{diagnostic}");
                    assert!(diagnostic.contains("unsupported"), "{diagnostic}");
                    if existing_output {
                        assert_eq!(fs::read(&sentinel).unwrap(), b"previous output\0unchanged");
                        assert_eq!(fs::read_dir(&dist).unwrap().count(), 1);
                    } else {
                        assert!(!dist.exists(), "partial outputs: {diagnostic}");
                    }
                }
            }
        }
    }
}

#[test]
fn unused_styles_and_style_looking_content_do_not_block_includes() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir(site.path().join("parts")).unwrap();
        fs::write(
            site.path().join("Entry.crepus"),
            "div\n  include parts/Child.vue\n  include parts/Child.svelte",
        )
        .unwrap();
        fs::write(
            site.path().join("parts/Child.vue"),
            "<!-- <style scoped>p{color:red}</style> --><template><p title='<style>literal</style>'>Vue child</p></template>",
        )
        .unwrap();
        fs::write(
            site.path().join("parts/Child.svelte"),
            "<!-- <style>p{color:red}</style> --><p title='<style>literal</style>'>Svelte child</p>",
        )
        .unwrap();
        fs::write(
            site.path().join("Unused.vue"),
            "<template><p>Unused</p></template><style scoped>p{color:red}</style>",
        )
        .unwrap();
        fs::write(
            site.path().join("Unused.svelte"),
            "<p>Unused</p><style>p{color:red}</style>",
        )
        .unwrap();

        let result = build(site.path(), target);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        assert!(dist.join(format!("CrepusEmit.{target}.ts")).is_file());
        let ir = fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap();
        assert!(ir.contains("Vue child"), "{ir}");
        assert!(ir.contains("Svelte child"), "{ir}");
        assert!(!dist.join("crepus-module-manifest.json").exists());
    }
}
