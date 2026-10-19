pub(crate) fn strip_trailing_inline_css(
    lines: &[&str],
    start: usize,
    mut end: usize,
) -> (usize, String) {
    if end <= start {
        return (end, String::new());
    }

    // Explicit trailing <style>...</style> block.
    let mut cursor = end;
    while cursor > start && lines[cursor - 1].trim().is_empty() {
        cursor -= 1;
    }
    if cursor > start && lines[cursor - 1].trim() == "</style>" {
        let mut open = cursor - 1;
        while open > start {
            open -= 1;
            if lines[open].trim() == "<style>" {
                let css = lines[(open + 1)..(cursor - 1)]
                    .join("\n")
                    .trim()
                    .to_string();
                return (open, css);
            }
        }
    }

    // Trailing raw CSS without `<style>` wrappers.
    //
    // The body and the CSS tail are not separated by a blank line in many
    // templates, so walking back through "CSS-shaped" lines alone is not
    // enough — `.crepus` element lines like `div bind:href={url}` and bare
    // expressions like `{score}` also end with `}`. We require an
    // **unambiguous CSS opener** at the top of the candidate trailing block
    // (`@`-rule, comment, or a selector line ending with `{`).
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    if end <= start {
        return (end, String::new());
    }
    if !lines[end - 1].trim().ends_with('}') {
        return (end, String::new());
    }

    let mut css_start = end;
    while css_start > start {
        let t = lines[css_start - 1].trim();
        if t.is_empty() {
            if css_start > start + 1 && looks_like_css_line(lines[css_start - 2].trim()) {
                css_start -= 1;
                continue;
            }
            break;
        }
        if !looks_like_css_line(t) {
            break;
        }
        css_start -= 1;
    }
    if css_start >= end {
        return (end, String::new());
    }

    // If the first line at css_start doesn't look like a CSS opener, try
    // the next line — the walk may have landed on a template-remnant line
    // (e.g. `div` placeholder) when the file is CSS-only.
    let mut actual_start = css_start;
    while actual_start < end {
        let candidate = lines[actual_start].trim();
        if candidate.starts_with('@')
            || candidate.starts_with("/*")
            || candidate.ends_with('{')
            || (candidate.ends_with('}') && candidate.contains('{') && candidate.contains(':'))
        {
            break;
        }
        actual_start += 1;
    }
    if actual_start >= end {
        return (end, String::new());
    }
    let css = lines[actual_start..end].join("\n").trim().to_string();
    (actual_start, css)
}

fn looks_like_css_line(line: &str) -> bool {
    // Attribute expressions can contain Rust paths and semicolons, just like CSS.
    if line
        .split_once('{')
        .is_some_and(|(prefix, _)| prefix.trim_end().ends_with('='))
    {
        return false;
    }
    if line.starts_with('@') || line.starts_with("/*") || line.starts_with('}') {
        return true;
    }
    if line.ends_with('{') || line.ends_with(',') {
        return true;
    }
    if line.ends_with(';') && line.contains(':') {
        return true;
    }
    if line.ends_with('}') && line.contains('{') && line.contains(':') && line.contains(';') {
        return true;
    }
    if line.contains(':') && line.contains(';') {
        return true;
    }
    false
}
