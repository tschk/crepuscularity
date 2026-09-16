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

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{ttl} — {site}</title>
<style>
  *{{box-sizing:border-box}}
  body{{margin:0;min-height:100vh;background:{s};color:{t};font-family:Inter,system-ui,sans-serif;-webkit-font-smoothing:antialiased;line-height:1.6}}
  a{{color:color-mix(in srgb,{t} 88%,transparent);text-decoration:none}}
  a:hover{{color:{t};text-decoration:underline;text-underline-offset:3px}}
  .doc-shell{{display:grid;grid-template-columns:minmax(220px,280px) 1fr;min-height:100vh}}
  aside{{position:sticky;top:0;height:100vh;overflow-y:auto;padding:1.25rem;border-right:1px solid {b};background:color-mix(in srgb,{s} 92%,white 8%)}}
  .brand{{font-weight:700;font-size:.95rem;color:{t};display:block;margin-bottom:1rem}}
  .brand:hover{{opacity:.85;text-decoration:none}}

  /* Search */
  .doc-search{{margin-bottom:1rem;position:relative}}
  .doc-search input{{width:100%;padding:.5rem .75rem;border:1px solid {b};border-radius:6px;background:color-mix(in srgb,{t} 6%,{s});color:{t};font-size:.85rem;outline:none;transition:border-color .15s}}
  .doc-search input:focus{{border-color:{a}}}
  .doc-search input::placeholder{{color:{m};opacity:.7}}
  .doc-search-results{{position:absolute;z-index:10;left:0;right:0;max-height:260px;overflow-y:auto;border:1px solid {b};border-radius:6px;background:{s};display:none;box-shadow:0 4px 16px rgba(0,0,0,.35)}}
  .doc-search-results.visible{{display:block}}
  .doc-search-results a{{display:block;padding:.5rem .75rem;font-size:.85rem;color:{m};border-bottom:1px solid color-mix(in srgb,{b} 30%,transparent)}}
  .doc-search-results a:last-child{{border-bottom:none}}
  .doc-search-results a:hover,.doc-search-results a.focused{{background:color-mix(in srgb,{a} 14%,transparent);color:{t};text-decoration:none}}
  .doc-search-results .no-match{{padding:.75rem;color:{m};font-size:.8rem;font-style:italic}}

  /* Sidebar sections */
  .nav-section{{margin-bottom:.25rem}}
  .nav-section summary{{cursor:pointer;font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.08em;color:{m};padding:.45rem .5rem;border-radius:5px;list-style:none;display:flex;align-items:center;gap:.4rem;transition:color .12s,background .12s;user-select:none}}
  .nav-section summary::-webkit-details-marker{{display:none}}
  .nav-section summary::before{{content:"▸";font-size:.6rem;transition:transform .15s;opacity:.5}}
  .nav-section[open] summary::before{{transform:rotate(90deg)}}
  .nav-section summary:hover{{color:{t};background:color-mix(in srgb,{t} 5%,transparent)}}
  .nav-section ul{{list-style:none;padding:0 0 0 .35rem;margin:.15rem 0 .5rem}}
  .nav-section li{{margin:.15rem 0}}
  .nav-section a{{display:block;padding:.25rem .5rem;border-radius:4px;font-size:.85rem;color:{m};transition:background .12s,color .12s}}
  .nav-section a.active{{color:{a};font-weight:600;background:color-mix(in srgb,{a} 12%,transparent)}}
  .nav-section a:hover{{color:{t};background:color-mix(in srgb,{t} 6%,transparent);text-decoration:none}}

  /* Content */
  article{{max-width:45rem;margin:0 auto;padding:1.5rem 2rem 3rem}}
  article h1,h2,h3{{color:{t}}}
  article h1{{font-size:1.75rem}}
  article h2{{font-size:1.3rem;border-bottom:1px solid {b};padding-bottom:.35rem}}
  article p,li{{color:{m}}}
  article code{{font-family:monospace;font-size:.85em;background:color-mix(in srgb,{t} 10%,transparent);padding:.12rem .35rem;border-radius:4px}}
  article pre{{background:color-mix(in srgb,{t} 6%,{s});border:1px solid {b};border-radius:8px;padding:1rem;overflow-x:auto;font-size:.82rem}}
  article pre code{{background:none;padding:0}}
  article blockquote{{margin:1rem 0;padding:.5rem 1rem;border-left:3px solid {a};background:color-mix(in srgb,{a} 8%,transparent);border-radius:0 8px 8px 0}}
  article table{{width:100%;border-collapse:collapse;margin:1rem 0;font-size:.875rem}}
  article th,td{{padding:.5rem .75rem;border:1px solid {b}}}
  article th{{background:color-mix(in srgb,{t} 6%,transparent);color:{t}}}

  @media(max-width:640px){{
    .doc-shell{{grid-template-columns:1fr}}
    aside{{position:static;height:auto;border-right:none;border-bottom:1px solid {b}}}
    .doc-search-results{{right:1rem}}
  }}
</style>
</head>
<body>
<div class="doc-shell">
<aside><a class="brand" href="../index.html">{site}</a>{nav}</aside>
<article>{body}</article>
</div>
<script>
(function(){{
  var idx=[];
  fetch('../docs/docs-search-index.json').then(function(r){{return r.json()}}).then(function(d){{idx=d}}).catch(function(){{}});
  var input=document.getElementById('doc-search-input');
  var results=document.getElementById('doc-search-results');
  var focusIdx=-1;
  if(!input||!results)return;
  function fuzzy(q,s){{
    q=q.toLowerCase();s=s.toLowerCase();
    if(s.indexOf(q)!==-1)return 1;
    var qi=0;
    for(var si=0;si<s.length&&qi<q.length;si++){{
      if(s[si]===q[qi])qi++;
    }}
    return qi===q.length?0.5:0;
  }}
  function showAll(){{
    var sections=document.querySelectorAll('.nav-section');
    for(var i=0;i<sections.length;i++){{
      var lis=sections[i].querySelectorAll('li');
      for(var j=0;j<lis.length;j++)lis[j].style.display='';
    }}
  }}
  function render(q){{
    if(!q.trim()){{
      results.className='doc-search-results';
      results.replaceChildren();
      showAll();
      return;
    }}
    var scored=[];
    for(var i=0;i<idx.length;i++){{
      var s=fuzzy(q,idx[i].title);
      if(s>0)scored.push({{item:idx[i],score:s}});
    }}
    scored.sort(function(a,b){{return b.score-a.score}});
    /* filter sidebar */
    var matchPaths=scored.map(function(s){{return s.item.path}});
    var sections=document.querySelectorAll('.nav-section');
    for(var i=0;i<sections.length;i++){{
      var lis=sections[i].querySelectorAll('li');
      var anyVisible=false;
      for(var j=0;j<lis.length;j++){{
        var a=lis[j].querySelector('a');
        var href=a?a.getAttribute('href'):'';
        var show=matchPaths.indexOf(href)!==-1;
        lis[j].style.display=show?'':'none';
        if(show)anyVisible=true;
      }}
      sections[i].open=anyVisible;
    }}
    results.replaceChildren();
    if(scored.length===0){{
      var empty=document.createElement('div');
      empty.className='no-match';
      empty.textContent='No results';
      results.appendChild(empty);
    }}else{{
      for(var i=0;i<scored.length;i++){{
        var item=scored[i].item;
        var path=String(item&&item.path?item.path:'');
        if(!path||path.indexOf(':')!==-1||path.indexOf('//')!==-1||path.charAt(0)==='/'||path.indexOf('\')!==-1||path.indexOf('..')!==-1)continue;
        var a=document.createElement('a');
        a.setAttribute('href',path);
        a.textContent=String(item.title||'');
        results.appendChild(a);
      }}
    }}
    results.className='doc-search-results visible';
    focusIdx=-1;
  }}
  input.addEventListener('input',function(){{render(this.value)}});
  input.addEventListener('keydown',function(e){{
    var links=results.querySelectorAll('a');
    if(e.key==='ArrowDown'){{e.preventDefault();focusIdx=Math.min(focusIdx+1,links.length-1);updateFocus(links)}}
    else if(e.key==='ArrowUp'){{e.preventDefault();focusIdx=Math.max(focusIdx-1,-1);updateFocus(links)}}
    else if(e.key==='Enter'&&focusIdx>=0&&links[focusIdx]){{e.preventDefault();window.location.href=links[focusIdx].href}}
    else if(e.key==='Escape'){{input.value='';render('');input.blur()}}
  }});
  function updateFocus(links){{
    for(var i=0;i<links.length;i++)links[i].className=i===focusIdx?'focused':'';
  }}
  document.addEventListener('click',function(e){{
    if(!e.target.closest('.doc-search')){{render('')}}
  }});
}})();
</script>
</body>
</html>"##
    )
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
