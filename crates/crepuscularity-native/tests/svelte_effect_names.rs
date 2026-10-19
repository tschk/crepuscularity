#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn effect_imports_avoid_unicode_escaped_local_bindings() {
    for (local, rune, imported) in [
        ("SvelteEffect", "$effect(() => {})", "createRuneEffect"),
        (
            "SveltePreEffect",
            "$effect.pre(() => {})",
            "createRunePreEffect",
        ),
        (
            "SvelteEffectRoot",
            "$effect.root(() => {})",
            "createRuneRoot",
        ),
        ("SvelteTracking", "$effect.tracking()", "runeTracking"),
    ] {
        let source = format!(
            r#"<script lang="ts">const \u005f_crepus{local}=(...args:any[])=>{{}};{rune};</script><p>ok</p>"#
        );
        let output = compile_moonshine_svelte(
            &source,
            Path::new("EffectNames.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .expect("escaped ordinary locals must not capture lowered effect calls");
        let generated = compact(&output.source);
        assert!(
            generated.contains(&format!("{imported}as__crepus{local}_")),
            "{}",
            output.source
        );
        assert!(
            generated.contains(&format!("__crepus{local}_(")),
            "{}",
            output.source
        );
    }
}

#[test]
fn server_effect_erasure_preserves_escaped_authored_locals() {
    let source = r#"<script lang="ts">
const \u005f_crepusSvelteEffect = 'ordinary';
const \u005f_crepusSveltePreEffect = 'pre';
const \u005f_crepusSvelteEffectRoot = 'root';
const \u005f_crepusSvelteTracking = 'tracking';
function factory(){throw new Error('factory must be erased')}
$effect(factory());
$effect.pre(factory());
const stop=$effect.root(factory());
const tracking=$effect.tracking();
</script><p>{\u005f_crepusSvelteEffect}:{\u005f_crepusSveltePreEffect}:{\u005f_crepusSvelteEffectRoot}:{\u005f_crepusSvelteTracking}</p>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("ServerEffectNames.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .expect("SSR erasure must not introduce helper/local collisions");
    let generated = compact(&output.source);
    for name in [
        "SvelteEffect",
        "SveltePreEffect",
        "SvelteEffectRoot",
        "SvelteTracking",
    ] {
        assert!(
            generated.contains(&format!("const__crepus{name}=")),
            "{}",
            output.source
        );
    }
    assert_eq!(
        generated.matches("factory()").count(),
        1,
        "only the authored factory declaration remains: {}",
        output.source
    );
    assert!(!generated.contains("createRune"), "{}", output.source);
    assert!(!generated.contains("runeTracking"), "{}", output.source);
    assert!(generated.contains("tracking=false"), "{}", output.source);
    assert!(
        generated.contains("conststop=(()=>{})"),
        "{}",
        output.source
    );
}
