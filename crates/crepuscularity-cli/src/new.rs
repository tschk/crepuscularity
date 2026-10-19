/// `crepus new NAME` — scaffold a new GPUI application.
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::ui;

pub fn run(name: &str) {
    let t0 = Instant::now();
    crate::scaffold::validate_project_name(name).unwrap_or_else(|e| ui::error(e));
    let dir = Path::new(name);

    if dir.exists() {
        ui::error(&format!("'{}' already exists", name));
    }

    crate::scaffold::create_project_dir(dir).unwrap_or_else(|e| ui::error(&e));

    fs::create_dir_all(dir.join("src")).unwrap_or_else(|e| {
        ui::error(&format!("could not create directories: {e}"));
    });

    fs::write(dir.join("Cargo.toml"), cargo_toml(name)).unwrap_or_else(|e| {
        ui::error(&format!("write Cargo.toml: {e}"));
    });
    fs::write(dir.join("src").join("main.rs"), main_rs(name)).unwrap_or_else(|e| {
        ui::error(&format!("write main.rs: {e}"));
    });
    fs::write(dir.join(".gitignore"), "/target\n").unwrap_or_else(|e| {
        ui::error(&format!("write .gitignore: {e}"));
    });

    use console::style;
    eprintln!("\n{} created {}", ui::ok(), style(name).cyan().bold());
    eprintln!();
    eprintln!("{}", style("Next steps:").dim());
    eprintln!("  cd {name}");
    eprintln!("  crepus dev");
    ui::done_in(t0.elapsed());
}

fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
gpui = {{ package = "gpui-ce", version = "0.2.2", default-features = false, features = ["font-kit"] }}
gpui_platform = {{ package = "gpui_ce_platform", version = "0.1.0", features = ["font-kit"] }}
crepuscularity-gpui = {{ version = "0.5.10" }}
"#
    )
}

fn main_rs(name: &str) -> String {
    let pascal = to_pascal_case(name);
    format!(
        r##"use crepuscularity_gpui::prelude::*;
use gpui::{{App, WindowOptions}};

struct {pascal}View {{
    count: i32,
}}

impl {pascal}View {{
    fn new(_cx: &mut Context<Self>) -> Self {{
        Self {{ count: 0 }}
    }}

    fn increment(&mut self, _: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut Context<Self>) {{
        self.count += 1;
        cx.notify();
    }}
}}

impl Render for {pascal}View {{
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {{
        let count = self.count;
        view! {{r#"
            div w-full h-full bg-zinc-950 text-white flex flex-col items-center justify-center gap-6
                div text-8xl font-bold leading-none
                    "{{count}}"
                button bg-white text-black font-semibold px-6 py-2 rounded-lg @click=increment
                    "increment"
        "#}}
    }}
}}

fn main() {{
    crepuscularity_gpui::application().run(|cx: &mut App| {{
        use gpui::prelude::*;
        match cx.open_window(WindowOptions::default(), |_win, cx| {{
            cx.new({pascal}View::new)
        }}) {{
            Ok(_) => {{}},
            Err(e) => eprintln!("failed to open window: {{e:?}}"),
        }}
    }});
}}
"##,
        pascal = pascal,
    )
}

pub(crate) fn to_pascal_case(s: &str) -> String {
    let mut name: String = s
        .split(&['-', '_', ' '][..])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect();
    if name
        .chars()
        .next()
        .is_some_and(|c| !unicode_ident::is_xid_start(c))
    {
        name.insert(0, '_');
    }
    name
}

#[cfg(test)]
mod scaffold_dependency_tests {
    use super::*;
    #[test]
    fn generated_gpui_source_matches_backend_registry_dependencies() {
        let generated: toml::Value = toml::from_str(&cargo_toml("fixture")).unwrap();
        let backend: toml::Value =
            toml::from_str(include_str!("../../crepuscularity-gpui/Cargo.toml")).unwrap();
        for dependency in ["gpui", "gpui_platform"] {
            let emitted = &generated["dependencies"][dependency];
            let shipped = &backend["dependencies"][dependency];
            for field in ["package", "version"] {
                assert_eq!(emitted[field], shipped[field], "{dependency}.{field}");
            }
            for field in ["git", "rev", "path"] {
                assert!(emitted.get(field).is_none(), "{dependency}.{field}");
            }
        }
        assert_eq!(
            generated["dependencies"]["gpui"]["default-features"],
            backend["dependencies"]["gpui"]["default-features"]
        );
        assert!(generated["dependencies"]["gpui_platform"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature.as_str() == Some("font-kit")));
        for feature in ["gpui/font-kit", "gpui_platform/font-kit"] {
            assert!(backend["features"]["font-kit"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry.as_str() == Some(feature)));
        }
    }
}
