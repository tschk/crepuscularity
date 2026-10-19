#![cfg(feature = "web-modules")]
//! Private initializer, lexical ownership and fail-closed callable regressions.
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(script: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(
        &format!("<script lang=\"ts\">{script}</script><p>{{model.read()}}</p>"),
        Path::new("Private.svelte"),
        target,
    )
    .map(|module| module.source)
}
#[test]
fn private_storage_brand_order_and_server_raw_fields() {
    let script="class Model{before=#n in this;#n:number=$state(1);#raw=$state.raw({x:1});#d=$derived(this.#n*2);after=#n in this;read(){return this.#n+this.#d}write(n:number){return this.#n=n}}const model=new Model();";
    let dom = compile(script, MoonshineModuleTarget::Dom).expect("private initializers");
    assert!(dom.contains("createPrivateState"), "{dom}");
    assert!(dom.contains("createPrivateRawState"), "{dom}");
    assert!(dom.contains("#n in this"), "{dom}");
    assert!(dom.contains("this.#n.assign(n)"), "{dom}");
    assert!(!dom.contains("get n()"), "{dom}");
    let ssr = compile(script, MoonshineModuleTarget::Ssr).expect("server private fields");
    assert!(!ssr.contains("createPrivateState"), "{ssr}");
    assert!(ssr.contains("#n: number = 1"), "{ssr}");
    assert!(ssr.contains("this.#n = n"), "{ssr}");
}
#[test]
fn constructor_reads_and_compounds_use_peek_but_updates_track() {
    let script="class Model{#n=$state(1);constructor(){const x=this.#n;(()=>this.#n)();this.#n+=1;this.#n&&=2;this.#n++;}read(){return this.#n}}const model=new Model();";
    let out = compile(script, MoonshineModuleTarget::Dom).expect("constructor references");
    assert!(out.contains("this.#n.peek()"), "{out}");
    assert!(out.contains("() => this.#n.value"), "{out}");
    assert!(out.contains("this.#n.value++"), "{out}");
    assert!(out.contains("this.#n.assign(this.#n.peek() + 1)"), "{out}");
}
#[test]
fn outer_lexical_private_is_explicitly_unsupported_until_contract_is_resolved() {
    let script="class Model{#n=$state(1);read(){const outer=this;class Inner{read(){return outer.#n}}return new Inner().read()}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile(script, target)
            .expect_err("outer private host-box divergence")
            .contains("unsupported outer lexical private rune reference"));
    }
}
#[test]
fn nested_own_private_constructor_context_and_ordinary_shadowing_are_separate() {
    let script="class Model{#n=$state(1);constructor(){const direct=this.#n;class Inner{#n=$state(2);constructor(){const direct=this.#n;(()=>this.#n)()}read(){return this.#n}}class Shadow{#n=3;read(){return this.#n}}new Inner();new Shadow()}read(){return this.#n}}const model=new Model();";
    let out = compile(script, MoonshineModuleTarget::Dom).expect("separate own private scopes");
    assert!(out.matches("this.#n.peek()").count() >= 2, "{out}");
    assert!(out.contains("() => this.#n.value"), "{out}");
    assert!(out.contains("return this.#n;"), "{out}");
}
#[test]
fn assignment_patterns_and_updates_remain_native_lvalues() {
    let script="class Model{#n=$state(1);read(){[this.#n]=[3];({x:this.#n}={x:4});return ++this.#n}}const model=new Model();";
    let out = compile(script, MoonshineModuleTarget::Dom).expect("private lvalues");
    assert!(out.contains("++this.#n.value"), "{out}");
    assert!(!out.contains("this.#n.peek()"), "{out}");
}
#[test]
fn private_inferred_callables_use_identity_and_preserve_call_arguments() {
    let script =
        "class Model{#fn=$state(<T>(x:T)=>x);read(){return this.#fn(1)}}const model=new Model();";
    let out = compile(script, MoonshineModuleTarget::Dom).expect("inferred generic arrow");
    assert!(out.contains("Identity<T,>(value: T): T"), "{out}");
    assert!(out.contains("Identity(this.#fn.value)(1)"), "{out}");
}
#[test]
fn unsupported_receivers_and_callable_types_fail_explicitly() {
    for script in [
        "class Model{#n=$state(1);read(){let other=this;return other.#n+=1}}const model=new Model();",
        "class Model{#n=$state(1);read(other:Model|null){return other?.#n}}const model=new Model();",
        "class Model{#fn=$state(()=>1);constructor(){this.#fn()}read(){return 1}}const model=new Model();",
        "class Model{#fn=$state(function(this:Model){return 1});read(){return this.#fn()}}const model=new Model();",
        "type Fn=(x:number)=>number;class Model{#fn:Fn=$state(x=>x);read(){return this.#fn(1)}}const model=new Model();",
    ] {
        assert!(compile(script,MoonshineModuleTarget::Dom).expect_err("unsupported private case").contains("unsupported"));
    }
}
