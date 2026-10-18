use crepuscularity_core::{diagnose_crepus_source_with_path, parse_template_with_path};
use std::path::Path;

#[test]
fn malformed_unicode_sources_never_panic_at_frontend_boundaries() {
    let tokens = [
        "<",
        ">",
        "/",
        "=\"",
        "'",
        "{",
        "}",
        ":",
        "(",
        ")",
        "[",
        "]",
        "\\",
        "\n",
        " ",
        "é",
        "雪",
        "😀",
        "\0",
        "<!--",
        "-->",
        "<script>",
        "</script>",
        "<style>",
        "</style>",
        "if",
        "for",
        "div",
        "...",
        "&amp;",
        "{@",
        "@",
        "#",
        "`",
        "\r",
        "\u{ffff}",
        "\u{2028}",
    ];
    let paths = [
        "probe.crepus",
        "probe.jsx",
        "probe.svelte",
        "probe.vue",
        "probe.astro",
        "probe.ng",
    ];
    // Fixed seed and bounded short inputs make failures reproducible without a fuzzing dependency.
    let mut state = 0xb3d1_3279_1195_75c5u64;
    for case in 0..2_000 {
        let mut source = String::new();
        for _ in 0..(8 + case % 25) {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            source.push_str(tokens[(state >> 32) as usize % tokens.len()]);
        }
        for filename in paths {
            let path = Path::new(filename);
            let result = std::panic::catch_unwind(|| {
                let _ = parse_template_with_path(&source, Some(path))
                    .map_err(|error| error.to_string());
                let _ = diagnose_crepus_source_with_path(&source, Some(path));
            });
            assert!(
                result.is_ok(),
                "case={case} path={filename} source={source:?}"
            );
        }
    }
}
