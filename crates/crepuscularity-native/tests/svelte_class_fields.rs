#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(script: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(&format!("<script lang=\"ts\">{script}</script><button onclick={{()=>model.n++}}>{{model.n}}/{{model.double}}</button>"),Path::new("Class.svelte"),target).map(|module|module.source)
}
#[test]
fn public_class_fields_use_independent_backing_cells_and_server_fields() {
    let source="class Model{n:number=$state(1);raw=$state.raw({x:2});double=$derived(this.n*2);by=$derived.by(()=>this.n+1);inc(){this.n++}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile(source, target).expect("public rune class compiles");
        assert!(!out.contains("$state"), "{out}");
        assert!(!out.contains("$derived"), "{out}");
        assert!(out.contains("get double()"), "{out}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(out.contains("get n()"), "{out}");
            assert!(out.contains("createRawState"), "{out}");
        } else {
            assert!(!out.contains("get n()"), "{out}");
            assert!(out.contains("n: number = 1"), "{out}");
        }
    }
}
#[test]
fn unsupported_class_fields_remain_explicit() {
    for field in [
        "static #n=$state(1)",
        "static n=$state(1)",
        "['n']=$state(1)",
        "readonly n=$state(1)",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(
                &format!("class Model{{{field}}}const model={{n:1,double:2}}"),
                target,
            )
            .expect_err("unsupported class rune");
            assert!(
                error.contains("unsupported Svelte class rune field"),
                "{error}"
            );
        }
    }
}

#[test]
fn zero_arguments_collisions_comments_and_shared_imports() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out=compile("// preserve class note\nclass Model{/* preserve field note */n:number|undefined=$state();second=$state(2);raw=$state.raw();double=$derived(this.n??0)}const model=new Model()",target).expect("zero argument state fields");
        assert!(out.contains("preserve class note"), "{out}");
        assert!(out.contains("preserve field note"), "{out}");
        assert!(out.matches("createState as").count() <= 1, "{out}");
        for script in [
            "class Model{n=$state(1);n=2}",
            "class Model{n=$state(1);get n(){return 2}}",
        ] {
            let error = compile(script, target).expect_err("field collision");
            assert!(
                error.contains("collision")
                    || (error.contains("invalid Svelte instance") && error.contains("scope")),
                "{error}"
            );
        }
    }
}

#[test]
fn macro_type_arguments_and_field_annotations_are_independent() {
    let source="class Model{n:number=$state(1);empty=$state<number>();raw=$state.raw<string>();wide=$state<string|number>(1);double=$derived(this.n*2)}const model=new Model()";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile(source, target).expect("generic state fields");
        assert!(out.contains("number | undefined"), "{out}");
        assert!(out.contains("string | undefined"), "{out}");
        assert!(out.contains("string | number"), "{out}");
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(out.contains("satisfies"), "{out}");
        }
    }
}

#[test]
fn zero_argument_fields_do_not_capture_undefined_bindings() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out=compile("function factory(undefined:number){return class Model{n=$state();double=$derived(2)}}const model=new (factory(9))()",target).expect("shadowed undefined parameter");
        assert!(out.contains("void 0"), "{out}");
    }
}

#[test]
fn escaped_binding_reference_and_private_names_are_reserved() {
    let source = r"const \u005f\u005fcrepusClassCell0=9;class Model{#\u005f\u005fcrepusClass1=8;n=$state(1);double=$derived(this.n*2);read(){return [this.#\u005f\u005fcrepusClass1,\u005f\u005fcrepusClassCell0,\u005f\u005fcrepusClassUnknown]}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target).expect("decoded names remain distinct");
        assert!(output.contains("__crepusClass_Cell0"), "{output}");
        assert!(output.contains("__crepusClassUnknown"), "{output}");
    }
}
