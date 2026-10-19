use crepuscularity_core::parse_template_with_path;
use std::path::Path;

#[test]
fn unsupported_spread_attributes_are_errors_across_markup_frontends() {
    for filename in [
        "view.jsx",
        "view.vue",
        "view.svelte",
        "view.astro",
        "view.ng.html",
    ] {
        for markup in [
            "<div {...props}/>",
            "<div { ...props }></div>",
            "<div {... props}/>",
        ] {
            let source = if filename.ends_with(".vue") {
                format!("<template>{markup}</template>")
            } else {
                markup.to_string()
            };
            let error = parse_template_with_path(&source, Some(Path::new(filename)))
                .expect_err(&format!("accepted {filename}: {source}"));
            assert!(
                error.to_string().contains("spread attributes"),
                "{filename}: {error}"
            );
        }
    }
}

#[test]
fn spread_is_rejected_on_jsx_control_tags_and_branches() {
    for source in [
        "<if test={true} {...props}><div/></if>",
        "<if test={false}><else-if test={true} {...props}><div/></else-if></if>",
        "<match on={value}><case pattern={1} {...props}><div/></case></match>",
    ] {
        let error = parse_template_with_path(source, Some(Path::new("view.tsx"))).unwrap_err();
        assert!(error.to_string().contains("spread attributes"), "{error}");
    }
}
