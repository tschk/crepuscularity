//! HTML void elements — must not emit a closing tag (e.g. `<br></br>` doubles line breaks).

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub fn is_void_html_tag(tag: &str) -> bool {
    VOID_TAGS.iter().any(|name| tag.eq_ignore_ascii_case(name))
}

/// Void elements cannot own children; never silently discard or evaluate them.
pub(crate) fn validate_children(
    el: &crepuscularity_core::ast::Element,
) -> Result<(), crepuscularity_core::CrepusError> {
    if is_void_html_tag(&el.tag) && !el.children.is_empty() {
        return Err(crepuscularity_core::CrepusError::render(format!(
            "HTML void element <{}> does not support children",
            el.tag
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_is_void() {
        assert!(is_void_html_tag("br"));
        assert!(!is_void_html_tag("pre"));
    }
}
