use crepuscularity_core::parse_template;

#[test]
fn stray_attribute_delimiters_are_errors() {
    for source in [
        "<div / title='lost' />",
        "<div = title='lost' />",
        "<div title='kept' / / >text</div>",
        "<if / test={true}><div/></if>",
        "<for let='item' = in={items}><div/></for>",
    ] {
        assert!(parse_template(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn valid_self_closing_tags_and_delimiters_in_values_still_parse() {
    for source in [
        "<div/>",
        "<div title='a/b=c' />",
        "<div title=\"a/b=c\"></div>",
        "<if test={a == b}><div/></if>",
    ] {
        assert!(parse_template(source).is_ok(), "rejected {source:?}");
    }
}

#[test]
fn shared_markup_frontends_reject_stray_attribute_delimiters() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for filename in [
        "view.jsx",
        "view.vue",
        "view.svelte",
        "view.astro",
        "view.ng.html",
    ] {
        for markup in [
            "<div / title='lost' />",
            "<div = title='lost' />",
            "<div title='a/b=c' />",
        ] {
            let source = if filename.ends_with(".vue") {
                format!("<template>{markup}</template>")
            } else {
                markup.to_string()
            };
            assert_eq!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_ok(),
                markup.contains("a/b=c"),
                "{filename}: {source}"
            );
        }
    }
}

#[test]
fn missing_values_are_errors_but_explicit_empty_strings_are_valid() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for filename in [
        "view.jsx",
        "view.vue",
        "view.svelte",
        "view.astro",
        "view.ng.html",
    ] {
        for (markup, valid) in [
            ("<div title= />", false),
            ("<div title=></div>", false),
            ("<div title=\n />", false),
            ("<div title=", false),
            ("<div title='' />", true),
            ("<div title=\"\" />", true),
            ("<div hidden />", true),
            ("<div hidden=false />", true),
            ("<div title=hello />", true),
        ] {
            let source = if filename.ends_with(".vue") {
                format!("<template>{markup}</template>")
            } else {
                markup.to_string()
            };
            assert_eq!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_ok(),
                valid,
                "{filename}: {source}"
            );
        }
    }
}

#[test]
fn ordinary_attribute_expressions_must_be_nonempty() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for filename in [
        "view.jsx",
        "view.vue",
        "view.svelte",
        "view.astro",
        "view.ng.html",
    ] {
        for (markup, valid) in [
            ("<div title={} />", false),
            ("<input disabled={   } />", false),
            ("<div data-value={} />", false),
            ("<div title={''} />", true),
            ("<div title={null} />", true),
            ("<input disabled={false} />", true),
            ("<div title='' />", true),
        ] {
            let source = if filename.ends_with(".vue") {
                format!("<template>{markup}</template>")
            } else {
                markup.to_string()
            };
            assert_eq!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_ok(),
                valid,
                "{filename}: {source}"
            );
        }
    }
}
