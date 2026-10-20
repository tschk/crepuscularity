use std::{fs, path::Path, process::Command};

fn build(site: &Path, output: &Path, target: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", "App.astro", "--out-dir"])
        .arg(output)
        .output()
        .unwrap()
}

#[test]
fn authored_astro_dependencies_use_the_native_module_graph() {
    let site = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    fs::write(site.path().join("App.astro"), "---\nimport Card from './Card.astro';\nimport {greet} from './format';\ninterface Props { name: string }\nconst {name} = Astro.props;\nconst title = greet(name);\n---\n<main><Card name={title}/><p>{name}</p></main>").unwrap();
    fs::write(
        site.path().join("Card.astro"),
        "---\ninterface Props {name:string}\nconst {name}=Astro.props;\n---\n<b>{name}</b>",
    )
    .unwrap();
    fs::write(
        site.path().join("format.ts"),
        "export function greet(value:string){return 'hello '+value}",
    )
    .unwrap();
    fs::write(
        site.path().join("Unused.astro"),
        "---\nconst = broken\n---\n<Missing/>",
    )
    .unwrap();
    let result = build(site.path(), output.path(), "moonshine-ssr");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let module_dir = output.path().join("modules/moonshine-ssr");
    let source = fs::read_to_string(module_dir.join("App.astro.ts")).unwrap();
    assert!(source.contains("./Card.astro.ts"), "{source}");
    assert!(module_dir.join("Card.astro.ts").exists());
    assert!(module_dir.join("format.ts").exists());
    assert!(!module_dir.join("Unused.astro.ts").exists());
    assert!(!source.contains("astro/runtime"));
}

#[test]
fn unsupported_authored_astro_preserves_existing_artifacts() {
    for (target, source) in [
        (
            "moonshine-dom",
            "---\nconst message='server';\n---\n<p>{message}</p>",
        ),
        (
            "moonshine-ssr",
            "---\nconst message=await Promise.resolve('server');\n---\n<p>{message}</p>",
        ),
        (
            "moonshine-ssr",
            "---\nconst value=Astro.request;\n---\n<p>{value}</p>",
        ),
        (
            "moonshine-ssr",
            "---\nconst value=1;\n---\n<div client:load>{value}</div>",
        ),
    ] {
        let site = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        fs::write(site.path().join("App.astro"), source).unwrap();
        let facade = output.path().join(format!("CrepusEmit.{target}.ts"));
        fs::write(&facade, "// preserved\n").unwrap();
        let result = build(site.path(), output.path(), target);
        assert!(!result.status.success(), "{source}");
        assert_eq!(fs::read_to_string(facade).unwrap(), "// preserved\n");
    }
}

#[test]
fn scriptless_astro_entry_keeps_legacy_template_emission() {
    let site = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    fs::write(site.path().join("App.astro"), "<p>{message}</p>").unwrap();
    let result = build(site.path(), output.path(), "moonshine-dom");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = fs::read_to_string(output.path().join("CrepusEmit.moonshine-dom.ts")).unwrap();
    assert!(source.contains("scope().message"), "{source}");
}
