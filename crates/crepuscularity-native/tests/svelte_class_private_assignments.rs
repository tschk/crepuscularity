#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

fn compile(script: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(
        &format!("<script lang=\"ts\">{script}</script><p>{{model.read()}}</p>"),
        Path::new("WrappedPrivate.svelte"),
        target,
    )
    .map(|module| module.source)
}

#[test]
fn wrapped_deep_assignments_return_the_resolved_cell_value() {
    for target in [
        "(this.#value as any)",
        "(this.#value satisfies {n:number})",
        "this.#value!",
        "((this.#value! as {n:number}))",
    ] {
        let script = format!("class Model{{#value=$state({{n:0}});read(){{const raw={{n:1}};const assigned={target} = raw;return [assigned===raw,assigned===this.#value].join(',')}}}}const model=new Model();");
        let output = compile(&script, MoonshineModuleTarget::Dom).unwrap();
        // The proxy result, not the original raw RHS, is the assignment value.
        assert!(
            output.contains("WithCell(this.#value"),
            "{target}: {output}"
        );
        assert!(output.contains("Captured.value"), "{output}");
        assert!(output.contains("peek() as unknown) as typeof"), "{output}");
        assert!(!output.contains("this.#value.assign(raw)"), "{output}");
    }
}

#[test]
fn wrapped_constructor_compounds_keep_state_and_raw_live_reads() {
    let script = "class Model{#state=$state(1);#raw=$state.raw(1);constructor(){(this.#state as number)+=2;(this.#state satisfies number)&&=4;(this.#raw as number)+=2;this.#raw!??=4;(this as Model).#state+=1;}read(){return this.#state+this.#raw}}const model=new Model();";
    let output = compile(script, MoonshineModuleTarget::Dom).unwrap();
    assert!(output.contains("Captured.peek() as number"), "{output}");
    assert!(
        output.contains("Captured.peek() satisfies number"),
        "{output}"
    );
    assert!(output.contains("this.#raw.value as number"), "{output}");
    assert!(output.contains("this.#raw.peek() as number"), "{output}");
    assert!(output.contains("this.#raw.peek()!"), "{output}");
    assert!(output.contains("(this as Model).#state.assign"), "{output}");
}

#[test]
fn wrapped_constructor_updates_remain_native_value_lvalues() {
    let script = "class Model{#state=$state(1);#raw=$state.raw(1);constructor(){(this.#state as number)++;++this.#raw!;[this.#state as number]=[4];}read(){return this.#state+this.#raw}}const model=new Model();";
    let output = compile(script, MoonshineModuleTarget::Dom).unwrap();
    assert!(
        output.contains("(this.#state.value as number)++"),
        "{output}"
    );
    assert!(output.contains("++this.#raw.value!"), "{output}");
    assert!(
        output.contains("[this.#state.value as number] = [4]"),
        "{output}"
    );
    assert!(!output.contains("peek()++"), "{output}");
}

#[test]
fn wrapped_non_this_compounds_cannot_bypass_receiver_validation() {
    for rune in ["$state", "$state.raw"] {
        for operation in [
            "(other.#value as number)+=1",
            "other.#value!&&=2",
            "(other.#value satisfies number)??=3",
        ] {
            let script = format!("class Model{{#value={rune}(1);read(){{const other=this;return {operation}}}}}const model=new Model();");
            let error = compile(&script, MoonshineModuleTarget::Dom).unwrap_err();
            assert!(
                error.contains("unsupported non-this private rune compound/logical receiver"),
                "{operation}: {error}"
            );
        }
    }
}

#[test]
fn server_state_targets_still_use_raw_private_data() {
    let script = "class Model{#value=$state({n:0});read(){const raw={n:1};const assigned=(this.#value as any)=raw;return assigned===this.#value}}const model=new Model();";
    let output = compile(script, MoonshineModuleTarget::Ssr).unwrap();
    assert!(!output.contains("createPrivateState"), "{output}");
    assert!(!output.contains(".assign(raw)"), "{output}");
    assert!(output.contains("(this.#value as any) = raw"), "{output}");
}

#[test]
fn target_casts_and_assignment_result_typing_survive_type_escapes() {
    let script = "class Model{#value:number=$state(1);#raw=$state.raw(1);#derived=$derived(1);read(){const changed:string=(this.#value as any)=\"changed\";(this.#raw as any)=\"raw\";(this.#derived as any)=\"derived\";return changed}}const model=new Model();";
    let output = compile(script, MoonshineModuleTarget::Dom).unwrap();
    assert!(output.contains("Captured.value as any"), "{output}");
    assert!(output.contains("as unknown) as typeof"), "{output}");
    assert!(
        output.contains("(this.#raw.value as any) = \"raw\""),
        "{output}"
    );
    assert!(
        output.contains("(this.#derived.value as any) = \"derived\""),
        "{output}"
    );
}

#[test]
fn wrapped_state_assignments_explicitly_reject_suspending_rhs() {
    for method in [
        "async write(){(this.#value as any)=await Promise.resolve(2)}",
        "*write(){(this.#value as any)=yield 2}",
    ] {
        let script = format!("class Model{{#value=$state(1);{method}read(){{return this.#value}}}}const model=new Model();");
        let error = compile(&script, MoonshineModuleTarget::Dom).unwrap_err();
        assert!(
            error.contains("unsupported await/yield RHS in wrapped private state assignment"),
            "{error}"
        );
    }
    let script = "class Model{#value:any=$state(1);read(){return (this.#value as any)=async()=>await Promise.resolve(2)}}const model=new Model();";
    compile(script, MoonshineModuleTarget::Dom).expect("nested callback await is not moved");
}
