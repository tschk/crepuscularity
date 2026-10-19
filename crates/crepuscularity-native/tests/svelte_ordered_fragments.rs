#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn original_context_fragments_compile_without_runtime_const_markers() {
    for (name, source) in [
        (
            "App.svelte",
            include_str!("fixtures/ordered-fragments/context/App.svelte"),
        ),
        (
            "Consumer.svelte",
            include_str!("fixtures/ordered-fragments/context/Consumer.svelte"),
        ),
        (
            "Read.svelte",
            include_str!("fixtures/ordered-fragments/context/Read.svelte"),
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_svelte(source, Path::new(name), target)
                .unwrap()
                .source;
            assert!(!output.contains("__CrepusConstScope"), "{output}");
            if name == "App.svelte" && matches!(target, MoonshineModuleTarget::Dom) {
                assert!(output.contains("getRuneConsumer as"), "{output}");
                assert!(output.contains("withComponentConsumer as"), "{output}");
                assert!(output.contains("readUpdates"), "{output}");
            }
            if matches!(target, MoonshineModuleTarget::Ssr) {
                assert!(!output.contains("getRuneConsumer as"), "{output}");
            }
        }
    }
}

#[test]
fn const_body_group_keeps_namespace_construction_lazy_and_original_types() {
    let source = r#"<script lang="ts">let values=$state([1]);</script><svg><g>{#each values as value}{@const text:number=value}<a><title>{text}</title></a>{/each}</g><foreignObject><div><a>html</a></div></foreignObject></svg>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Namespaces.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(output.contains("readUpdates"), "{output}");
    assert!(output.contains("__crepusOnce"), "{output}");
    assert!(!output.contains("__CrepusConstScope"), "{output}");
}

#[test]
fn namespace_admission_is_authored_dom_only_and_keeps_executable_guards() {
    use crepuscularity_native::compile_moonshine_module;
    assert!(compile_moonshine_module(
        "export default function C(){return <svg><g/></svg>}",
        Path::new("Generic.tsx"),
        MoonshineModuleTarget::Dom
    )
    .is_err());
    assert!(compile_moonshine_svelte(
        "<svg><script/></svg>",
        Path::new("Unsafe.svelte"),
        MoonshineModuleTarget::Dom
    )
    .is_err());
    let foreign = compile_moonshine_svelte(
        "<svg><iframe/></svg>",
        Path::new("Foreign.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    let compact: String = foreign.source.split_whitespace().collect();
    assert!(
        compact.contains("\"http://www.w3.org/2000/svg\",\"iframe\""),
        "{}",
        foreign.source
    );
    let svg = compile_moonshine_svelte(
        "<svg><g/></svg>",
        Path::new("Svg.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, &svg.source, oxc_span::SourceType::tsx()).parse();
    assert!(
        parsed.errors.is_empty(),
        "{:?}\n{}",
        parsed.errors,
        svg.source
    );
    assert!(
        svg.source.contains("hSource as __crepusSourceIntrinsic"),
        "{}",
        svg.source
    );
    assert!(
        !svg.source.contains("__crepusIntrinsicNamespace"),
        "{}",
        svg.source
    );
    assert!(compile_moonshine_module(
        "export default function C(){return <svg><g/></svg>}",
        Path::new("Generic.tsx"),
        MoonshineModuleTarget::Ssr,
    )
    .is_err());
}

#[test]
fn nonconst_dom_fragments_use_ordered_groups_without_directives() {
    let source = r#"<script lang="ts">let label=$state('one');function read(){return label;}</script><section title={read()}><p>{read()}</p></section>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Plain.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(dom.contains("getRuneConsumer as"), "{dom}");
    assert!(
        dom.contains("Authored fragment setup did not complete"),
        "{dom}"
    );
    assert!(dom.contains("readUpdates"), "{dom}");
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Plain.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("getRuneConsumer as"), "{ssr}");
    assert!(
        !ssr.contains("Authored fragment setup did not complete"),
        "{ssr}"
    );
}

#[test]
fn ordered_javascript_keeps_language_and_jsdoc_public_metadata() {
    use crepuscularity_native::MoonshineModuleLanguage;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use oxc_span::SourceType;
    let source = r#"<script>/** @typedef {{label:string}} Props */
/** @type {Props} */let{label}=$props();
/** @param {number} step @returns {number} */export function increment(step){return step+1;}</script><section title={label}><p>{label}</p></section>"#;
    let output =
        compile_moonshine_svelte(source, Path::new("Docs.svelte"), MoonshineModuleTarget::Dom)
            .unwrap();
    assert_eq!(output.language, MoonshineModuleLanguage::JavaScript);
    assert!(
        output.source.contains("@typedef {{label:string}} Props"),
        "{}",
        output.source
    );
    assert!(
        output.source.contains("ComponentView<typeof"),
        "{}",
        output.source
    );
    assert!(
        output.source.contains("getRuneConsumer as"),
        "{}",
        output.source
    );
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &output.source, SourceType::mjs()).parse();
    assert!(
        parsed.errors.is_empty(),
        "{:?}\n{}",
        parsed.errors,
        output.source
    );
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        semantic.errors.is_empty(),
        "{:?}\n{}",
        semantic.errors,
        output.source
    );
}

#[test]
fn ordered_await_const_directives_keep_all_three_arm_markers_consumed() {
    let source = r#"<script lang="ts">let input:Promise<number>=$state(Promise.resolve(1));function action(node:HTMLElement){return{destroy(){}}}</script>{#await input}<p use:action>pending</p>{:then value}{@const result:number=value}<p use:action>{result}</p>{:catch error}<p use:action>error</p>{/await}"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Await.svelte"), target)
            .unwrap()
            .source;
        assert!(!output.contains("__crepusDirective0"), "{output}");
        assert!(!output.contains("__CrepusConstScope"), "{output}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.contains("getRuneConsumer as"), "{output}");
            assert!(output.contains("installAction as"), "{output}");
        } else {
            assert!(!output.contains("installAction as"), "{output}");
        }
    }
}

#[test]
fn ordered_plain_intrinsic_children_have_a_bound_once_helper() {
    let source = "<script lang=\"ts\">let {label}:{label:string}=$props();function read(){return label;}</script><p>{read()}</p>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("Plain.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(output.contains("function __crepusOnce"), "{output}");
    assert!(output.contains("__crepusOnce0(() =>"), "{output}");
    assert!(!output.contains("__CrepusDirectiveTemplate"), "{output}");
}
