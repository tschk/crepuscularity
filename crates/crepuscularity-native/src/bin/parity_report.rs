//! Diff Tailwind-like **static** class literals between GPUI `styler.rs::apply_static` and the
//! native [`crepuscularity_native::style`] subset.
//!
//! Also loads `data/class_registry.toml` (intended coverage) when present.
//!
//! Usage: `parity-report [--json path]` (default: print summary to stdout).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ClassRegistryFile {
    classes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ParityTiersFile {
    #[serde(default)]
    tiers: Vec<ParityTier>,
}

#[derive(Debug, Deserialize)]
struct ParityTier {
    id: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    classes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    source_styler: String,
    source_native_style: String,
    source_native_render: String,
    styler_apply_static_literals: usize,
    native_style_literals: usize,
    in_both: Vec<String>,
    only_in_styler: Vec<String>,
    only_in_native: Vec<String>,
    registry_classes: Vec<String>,
    registry_not_in_native: Vec<String>,
    registry_not_in_styler_static: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tiers: Vec<BTreeMap<String, serde_json::Value>>,
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut json_out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--json" || a == "-j" {
            json_out = args.next().map(PathBuf::from);
        }
    }

    let styler_path = manifest_dir.join("../crepuscularity-runtime/src/styler.rs");
    let style_path = manifest_dir.join("src/style.rs");
    let render_path = manifest_dir.join("src/render.rs");

    let styler_src = std::fs::read_to_string(&styler_path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", styler_path.display());
        std::process::exit(1);
    });
    let style_src = std::fs::read_to_string(&style_path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", style_path.display());
        std::process::exit(1);
    });
    let render_src = std::fs::read_to_string(&render_path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", render_path.display());
        std::process::exit(1);
    });

    // `styler.rs::apply_static` has no `{` char literals; brace matching is reliable.
    let styler_body = extract_fn_body_brace(&styler_src, "apply_static").unwrap_or_else(|| {
        eprintln!("parity-report: could not find apply_static in styler.rs");
        std::process::exit(1);
    });

    let mut native_literals = BTreeSet::new();
    for name in [
        "apply_layout_class",
        "apply_text_class",
        "apply_color_and_radius",
    ] {
        if let Some(body) = extract_fn_until_next_fn(&style_src, name) {
            native_literals.extend(extract_match_arm_classes_from_body(body));
            native_literals.extend(extract_quoted_tailwind_tokens(body));
        }
    }
    if let Some(body) = extract_fn_until_next_fn(&style_src, "is_scroll_container") {
        native_literals.extend(extract_quoted_tailwind_tokens(body));
    }
    for name in ["stack_axis", "parse_gap_spacing"] {
        if let Some(body) = extract_fn_until_next_fn(&render_src, name) {
            native_literals.extend(extract_quoted_tailwind_tokens(body));
        }
    }

    let styler_literals: BTreeSet<String> = extract_match_arm_classes_from_body(styler_body);

    let registry_path = manifest_dir.join("data/class_registry.toml");
    let registry: BTreeSet<String> = std::fs::read_to_string(&registry_path)
        .ok()
        .and_then(|s| toml::from_str::<ClassRegistryFile>(&s).ok())
        .map(|r| r.classes.into_iter().collect())
        .unwrap_or_default();

    let tiers_path = manifest_dir.join("data/parity_tiers.json");
    let tiers_file: ParityTiersFile = std::fs::read_to_string(&tiers_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(ParityTiersFile { tiers: vec![] });

    let only_in_styler: Vec<String> = styler_literals
        .difference(&native_literals)
        .cloned()
        .collect();
    let only_in_native: Vec<String> = native_literals
        .difference(&styler_literals)
        .cloned()
        .collect();
    let in_both: Vec<String> = styler_literals
        .intersection(&native_literals)
        .cloned()
        .collect();

    let registry_not_in_native: Vec<String> = registry
        .iter()
        .filter(|c| !native_covers_class(c.as_str(), &native_literals))
        .cloned()
        .collect();
    let registry_not_in_styler_static: Vec<String> =
        registry.difference(&styler_literals).cloned().collect();

    let styler_refs: BTreeSet<&str> = styler_literals.iter().map(String::as_str).collect();

    let tier_rows: Vec<BTreeMap<String, serde_json::Value>> = tiers_file
        .tiers
        .iter()
        .map(|t| {
            let set: BTreeSet<&str> = t.classes.iter().map(|s| s.as_str()).collect();
            let n_styler = set.intersection(&styler_refs).count();
            let n_native = t
                .classes
                .iter()
                .filter(|c| native_covers_class(c.as_str(), &native_literals))
                .count();
            let mut m = BTreeMap::new();
            m.insert("id".into(), serde_json::json!(t.id));
            m.insert("description".into(), serde_json::json!(t.description));
            m.insert("classes_total".into(), serde_json::json!(t.classes.len()));
            m.insert("in_styler_static".into(), serde_json::json!(n_styler));
            m.insert("in_native_style".into(), serde_json::json!(n_native));
            m
        })
        .collect();

    let report = Report {
        source_styler: styler_path.display().to_string(),
        source_native_style: style_path.display().to_string(),
        source_native_render: render_path.display().to_string(),
        styler_apply_static_literals: styler_literals.len(),
        native_style_literals: native_literals.len(),
        in_both,
        only_in_styler,
        only_in_native,
        registry_classes: registry.iter().cloned().collect(),
        registry_not_in_native,
        registry_not_in_styler_static,
        tiers: tier_rows,
    };

    println!(
        "GPUI styler apply_static: {} class literals",
        report.styler_apply_static_literals
    );
    println!(
        "Native style.rs match arms: {} class literals",
        report.native_style_literals
    );
    println!("Intersection: {}", report.in_both.len());
    println!("Only styler: {}", report.only_in_styler.len());
    println!("Only native: {}", report.only_in_native.len());
    if !report.registry_classes.is_empty() {
        println!(
            "Registry: {} classes (not in native: {}, not in styler static: {})",
            report.registry_classes.len(),
            report.registry_not_in_native.len(),
            report.registry_not_in_styler_static.len()
        );
    }

    let json = serde_json::to_string_pretty(&report).expect("serde_json::to_string_pretty(report)");
    if let Some(p) = json_out {
        std::fs::write(&p, &json).unwrap_or_else(|e| {
            eprintln!("write {}: {e}", p.display());
            std::process::exit(1);
        });
        eprintln!("wrote {}", p.display());
    }
}

/// Slice from `fn name` through the line before the next top-level `fn ` (for `style.rs`).
fn extract_fn_until_next_fn<'a>(src: &'a str, fn_name: &str) -> Option<&'a str> {
    let needle = format!("fn {fn_name}");
    let start = src.find(&needle)?;
    let rest = &src[start..];
    let end = rest[1..]
        .find("\nfn ")
        .map(|i| start + 1 + i)
        .unwrap_or_else(|| src.len());
    Some(&src[start..end])
}

/// Opening brace of `fn name` until the matching closing brace (fails if `{` appears in literals).
fn extract_fn_body_brace<'a>(src: &'a str, fn_name: &str) -> Option<&'a str> {
    let needle = format!("fn {fn_name}");
    let start = src.find(&needle)?;
    let brace = src[start..].find('{')?;
    let open = start + brace;
    let mut depth = 0i32;
    for (i, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = open + i + ch.len_utf8();
                    return Some(&src[open..end]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract all double-quoted string contents from `s`.
fn extract_quoted_strings(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            i = start;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i < bytes.len() {
                out.push(&s[start..i]);
            }
        }
        i += 1;
    }
    out
}

/// Quoted tokens in `matches!(…)` helpers (no `=>` arms).
fn extract_quoted_tailwind_tokens(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in extract_quoted_strings(body) {
        if is_class_literal(s) {
            out.insert(s.to_string());
        }
    }
    out
}

fn extract_match_arm_classes_from_body(body: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = BTreeSet::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        let starts_arm =
            t.starts_with('"') || t.starts_with('|') || (t.starts_with('_') && line.contains("=>"));
        if starts_arm {
            let start_line = i;
            let mut chunk = String::new();
            let mut found_arrow = false;
            while i < lines.len() {
                chunk.push_str(lines[i]);
                chunk.push('\n');
                if lines[i].contains("=>") {
                    found_arrow = true;
                    i += 1;
                    break;
                }
                i += 1;
            }
            if found_arrow {
                let left = chunk.split_once("=>").map(|(l, _)| l).unwrap_or("");
                for s in extract_quoted_strings(left) {
                    if is_class_literal(s) {
                        out.insert(s.to_string());
                    }
                }
            } else {
                i = start_line + 1;
                continue;
            }
            continue;
        }
        i += 1;
    }
    out
}

/// `gap-*` is implemented via a `strip_prefix("gap-")` hook, not per-literal match arms.
fn native_covers_class(class: &str, native: &BTreeSet<String>) -> bool {
    if native.contains(class) {
        return true;
    }
    if native.contains("gap-")
        && class.starts_with("gap-")
        && class.len() > 4
        && class[4..].chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    if native.contains("bg-") && class.strip_prefix("bg-").is_some_and(native_palette_or_hex) {
        return true;
    }
    if native.contains("text-")
        && class
            .strip_prefix("text-")
            .is_some_and(|rest| native_text_size(rest) || native_palette_or_hex(rest))
    {
        return true;
    }
    false
}

fn native_text_size(value: &str) -> bool {
    matches!(
        value,
        "xs" | "sm"
            | "base"
            | "lg"
            | "xl"
            | "2xl"
            | "3xl"
            | "4xl"
            | "5xl"
            | "6xl"
            | "7xl"
            | "8xl"
            | "9xl"
    )
}

fn native_palette_or_hex(value: &str) -> bool {
    matches!(value, "black" | "white" | "transparent" | "clear")
        || value.starts_with("[#")
        || value.starts_with("zinc-")
        || value.starts_with("slate-")
        || value.starts_with("gray-")
        || value.starts_with("neutral-")
        || value.starts_with("stone-")
        || value.starts_with("red-")
        || value.starts_with("orange-")
        || value.starts_with("amber-")
        || value.starts_with("yellow-")
        || value.starts_with("lime-")
        || value.starts_with("green-")
        || value.starts_with("emerald-")
        || value.starts_with("teal-")
        || value.starts_with("cyan-")
        || value.starts_with("sky-")
        || value.starts_with("blue-")
        || value.starts_with("indigo-")
        || value.starts_with("violet-")
        || value.starts_with("purple-")
        || value.starts_with("fuchsia-")
        || value.starts_with("pink-")
        || value.starts_with("rose-")
}

fn is_class_literal(s: &str) -> bool {
    if s.is_empty() || s.len() > 96 {
        return false;
    }
    if s == "_" || s.starts_with("//") {
        return false;
    }
    if s.contains('{') || s.contains('}') {
        return false;
    }
    if s.contains(' ') {
        return false;
    }
    if s.starts_with('#') && s.len() == 7 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fn_body_brace() {
        let src = "fn apply_static() { hello { } }";
        assert_eq!(
            extract_fn_body_brace(src, "apply_static"),
            Some("{ hello { } }")
        );

        let src_unbalanced = "fn unbalanced() { hello { } ";
        assert_eq!(extract_fn_body_brace(src_unbalanced, "unbalanced"), None);

        let src_nested = "fn nested() { { { } } }";
        assert_eq!(
            extract_fn_body_brace(src_nested, "nested"),
            Some("{ { { } } }")
        );

        let src_empty = "fn empty() {}";
        assert_eq!(extract_fn_body_brace(src_empty, "empty"), Some("{}"));

        let src_no_fn = "pub const X: i32 = 42;";
        assert_eq!(extract_fn_body_brace(src_no_fn, "missing"), None);

        let src_no_brace = "fn no_brace()";
        assert_eq!(extract_fn_body_brace(src_no_brace, "no_brace"), None);
    }
}
