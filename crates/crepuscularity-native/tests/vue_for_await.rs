#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn top_level_for_await_is_rejected_before_synchronous_component_emission() {
    for body in [
        "for await (const item of []) {}",
        "{ for await (const item of []) {} }",
        "import {ref} from 'vue';for await (const item of []) {}",
        "import './setup';for await (const item of []) {}",
    ] {
        let source =
            format!("<script setup lang=\"ts\">const props=defineProps<{{label?:string}}>();{body}</script><template><p>Ready</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source, Path::new("AsyncSetup.vue"), target)
                .expect_err("awaited loops must not leak into a synchronous generated component");
            assert!(
                error.contains("async setup")
                    || (error.contains("Vue setup parse failed") && error.contains("await")),
                "{body}: {error}"
            );
            if body.starts_with("import ") {
                assert!(error.contains("async setup unsupported"), "{body}: {error}");
            }
        }
    }
}

#[test]
fn for_await_inside_async_functions_remains_supported() {
    let source = r#"<script setup lang="ts">
const props=defineProps<{label?:string}>();
async function consume() { for await (const item of [1, 2]) { void item; } }
const consumeArrow = async () => { for await (const item of [3]) { void item; } };
void consume; void consumeArrow;
</script><template><p>Ready</p></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(source, Path::new("AsyncFunction.vue"), target)
            .expect("nested async functions do not make setup asynchronous");
        assert!(
            output.source.contains("async function consume"),
            "{}",
            output.source
        );
        assert!(output.source.contains("for await"), "{}", output.source);
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &output.source, oxc_span::SourceType::tsx())
                .parse();
        assert!(
            parsed.errors.is_empty(),
            "{:?}\n{}",
            parsed.errors,
            output.source
        );
    }
}
