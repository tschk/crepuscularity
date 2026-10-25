//! Built-in Markdown → HTML docs generator for `crepus web build`.
//!
//! Auto-detects `docs/` or `public/` relative to the site directory.
//! No external binary or config needed — framework handles it like Svelte/Next.js.

use std::io;
use std::path::Path;

use pulldown_cmark::{html, Options, Parser};

use crate::web::ThemeCss;

/// Sidebar section: (group_label, list of page_stems in display order).
const SECTIONS: &[(&str, &[&str])] = &[
    (
        "Getting Started",
        &["index", "dsl", "components", "cli", "changelog"],
    ),
    (
        "Targets",
        &[
            "gpui",
            "native",
            "tui",
            "embedded",
            "lvgl",
            "lite",
            "aurorality",
            "webext",
            "webext-policy",
        ],
    ),
    (
        "Reference",
        &[
            "view-ir-contract",
            "plugin-surface",
            "polyglot",
            "production",
            "observability",
            "timing-profile",
            "ide-extensions",
            "runtime",
            "react-native-adaptation",
            "tauri-compatibility",
        ],
    ),
];

pub(crate) fn generate_docs(
    src_dir: &Path,
    out_dir: &Path,
    theme: &ThemeCss,
    site_name: &str,
) -> io::Result<()> {
    if !src_dir.is_dir() {
        return Ok(());
    }

    // First pass: collect all pages
    let mut page_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut bodies: Vec<(String, String, String)> = Vec::new();

    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap_or_default();
        let content = std::fs::read_to_string(&path)?;
        let (title, body_html) = render_md(&content);
        let out_stem = if stem.eq_ignore_ascii_case("README") {
            "index".to_string()
        } else {
            stem.clone()
        };
        page_map.insert(out_stem.clone(), title.clone());
        bodies.push((out_stem, title, body_html));
    }

    // Second pass: generate HTML with full grouped nav
    std::fs::create_dir_all(out_dir)?;
    for (out_stem, title, body_html) in &bodies {
        let nav = render_nav(&page_map, out_stem);
        let html = render_shell(body_html, title, &nav, theme, site_name);
        std::fs::write(out_dir.join(format!("{out_stem}.html")), html)?;
    }

    if page_map.is_empty() {
        return Ok(());
    }

    // Search index
    let idx: Vec<serde_json::Value> = page_map
        .iter()
        .map(|(stem, title)| serde_json::json!({"path": format!("{stem}.html"), "title": title}))
        .collect();
    std::fs::write(
        out_dir.join("docs-search-index.json"),
        serde_json::to_string_pretty(&serde_json::json!(idx))?,
    )?;

    Ok(())
}

fn render_md(md: &str) -> (String, String) {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_GFM);

    let mut title = String::new();
    for line in md.lines() {
        if title.is_empty() && line.starts_with("# ") {
            title = line[2..].trim().to_string();
        }
    }
    let mut body = String::new();
    html::push_html(&mut body, Parser::new_ext(md, opts));
    // First-party Markdown can still embed raw HTML; strip scripts and unsafe
    // markup the same way the web renderer sanitizes `RawHtml`.
    let body = ammonia::clean(&body);
    if title.is_empty() {
        title = "Documentation".into();
    }
    (title, body)
}

fn render_nav(page_map: &std::collections::HashMap<String, String>, current: &str) -> String {
    let mut sections_html = String::new();

    for (group_label, stems) in SECTIONS {
        // Only include pages that exist
        let items: Vec<String> = stems
            .iter()
            .filter(|s| page_map.contains_key(**s))
            .map(|s| {
                let title = page_map.get(*s).map(String::as_str).unwrap_or(s);
                let active = if *s == current {
                    " class=\"active\""
                } else {
                    ""
                };
                format!("<li><a href=\"{s}.html\"{active}>{}</a></li>", esc(title))
            })
            .collect();

        if items.is_empty() {
            continue;
        }

        // Expand the section that contains the current page
        let expanded = stems.contains(&current);
        let open = if expanded { " open" } else { "" };

        sections_html.push_str(&format!(
            "<details class=\"nav-section\"{open}>\
<summary>{}</summary>\
<ul>{}</ul>\
</details>",
            esc(group_label),
            items.join("")
        ));
    }

    // Pages not in any section get an "Other" group
    let assigned: std::collections::HashSet<&str> = SECTIONS
        .iter()
        .flat_map(|(_, stems)| stems.iter().copied())
        .collect();
    let other_items: Vec<String> = page_map
        .keys()
        .filter(|s| !assigned.contains(s.as_str()))
        .map(|s| {
            let title = page_map.get(s.as_str()).map(String::as_str).unwrap_or(s);
            let active = if s.as_str() == current {
                " class=\"active\""
            } else {
                ""
            };
            format!("<li><a href=\"{s}.html\"{active}>{}</a></li>", esc(title))
        })
        .collect();

    if !other_items.is_empty() {
        sections_html.push_str(&format!(
            "<details class=\"nav-section\"><summary>Other</summary><ul>{}</ul></details>",
            other_items.join("")
        ));
    }

    format!(
        "<nav aria-label=\"Documentation\">\
<div class=\"doc-search\"><input type=\"text\" id=\"doc-search-input\" placeholder=\"Search docs…\" autocomplete=\"off\">\
<div id=\"doc-search-results\" class=\"doc-search-results\"></div></div>\
{sections_html}</nav>"
    )
}

fn render_shell(body: &str, title: &str, nav: &str, theme: &ThemeCss, site_name: &str) -> String {
    let ttl = esc(title);
    let site = esc(site_name);
    let a = esc(&theme.accent);
    let s = esc(&theme.surface);
    let t = esc(&theme.text);
    let m = esc(&theme.muted);
    let b = esc(&theme.border);

    let script = include_str!("docs_script.js");
    let mut style = include_str!("docs_style.css").to_string();
    style = style.replace("{a}", &a);
    style = style.replace("{s}", &s);
    style = style.replace("{t}", &t);
    style = style.replace("{m}", &m);
    style = style.replace("{b}", &b);

    let mut template = include_str!("docs_template.html").to_string();
    template = template.replace("{ttl}", &ttl);
    template = template.replace("{site}", &site);
    template = template.replace("{nav}", nav);
    template = template.replace("{body}", body);
    template = template.replace("{style}", &style);
    template = template.replace("{script}", script);
    template
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::ThemeCss;

    #[test]
    fn search_script_does_not_assign_inner_html() {
        let html = render_shell("body", "Title", "<nav></nav>", &ThemeCss::default(), "Site");
        assert!(
            !html.contains("innerHTML"),
            "docs search must not concatenate index fields into innerHTML"
        );
        assert!(html.contains("textContent"));
        assert!(html.contains("createElement('a')"));
    }

    #[test]
    fn generate_docs_escapes_markdown_titles_in_nav() {
        let src = tempfile::tempdir().expect("src");
        let out = tempfile::tempdir().expect("out");
        std::fs::write(
            src.path().join("xss.md"),
            "# Hello <script>alert(1)</script>\n\nsafe body\n",
        )
        .expect("write md");

        generate_docs(src.path(), out.path(), &ThemeCss::default(), "Site").expect("generate");

        let html = std::fs::read_to_string(out.path().join("xss.html")).expect("read html");
        assert!(
            html.contains("<title>Hello &lt;script&gt;alert(1)&lt;/script&gt; — Site</title>"),
            "page title must HTML-escape markdown headings"
        );
        assert!(
            html.contains("&lt;script&gt;alert(1)&lt;/script&gt;</a></li>"),
            "sidebar nav must HTML-escape markdown headings"
        );

        let idx = std::fs::read_to_string(out.path().join("docs-search-index.json")).expect("idx");
        assert!(idx.contains("Hello <script>alert(1)</script>"));
        assert!(
            html.contains("a.textContent=String(item.title||'')"),
            "search hits must use textContent so index titles cannot inject HTML"
        );
        assert!(
            !html.contains("<script>alert(1)</script>"),
            "markdown body HTML must be sanitized"
        );
    }

    #[test]
    fn generate_docs_strips_raw_html_script_from_body() {
        let src = tempfile::tempdir().expect("src");
        let out = tempfile::tempdir().expect("out");
        std::fs::write(
            src.path().join("index.md"),
            "# Safe\n\n<script>alert(1)</script>\n\nKeep **bold** and a [link](dsl.html).\n",
        )
        .expect("write md");

        generate_docs(src.path(), out.path(), &ThemeCss::default(), "Site").expect("generate");

        let html = std::fs::read_to_string(out.path().join("index.html")).expect("read html");
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("href=\"dsl.html\""));
    }
}
