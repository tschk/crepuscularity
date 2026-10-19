use crepuscularity_core::parse_template_with_path;
use std::path::Path;

#[test]
fn nested_and_quoted_keys_do_not_corrupt_the_item_binding() {
    let expected = parse_template_with_path(
        "{#each items as item}<span>{item}</span>{/each}",
        Some(Path::new("view.svelte")),
    )
    .unwrap();
    for key in [
        "item.id",
        "getKey(item)",
        "(item.id)",
        "getKey(item, ')')",
        "'(' + item.id",
    ] {
        let source = format!("{{#each items as item ({key})}}<span>{{item}}</span>{{/each}}");
        assert_eq!(
            parse_template_with_path(&source, Some(Path::new("view.svelte"))).unwrap(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn keyed_each_rejects_empty_unclosed_or_trailing_key_syntax() {
    for binding in [
        "item ()",
        "item (   )",
        "item (item.id",
        "item (item.id) ignored",
        "item (item.id))",
        "item (item.id) (other)",
    ] {
        let source = format!("{{#each items as {binding}}}<span/>{{/each}}");
        assert!(
            parse_template_with_path(&source, Some(Path::new("view.svelte"))).is_err(),
            "accepted {source}"
        );
    }
}
