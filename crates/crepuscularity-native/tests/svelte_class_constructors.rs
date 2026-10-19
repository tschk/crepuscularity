#![cfg(feature = "web-modules")]
//! Constructor-first storage is initialized at the authored assignment, not a field initializer.
use crepuscularity_native::{
    compile_moonshine_module, compile_moonshine_svelte, MoonshineModuleTarget,
};
use std::path::Path;
fn compile(script: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(
        &format!("<script lang=\"ts\">{script}</script><p>{{JSON.stringify(model.read())}}</p>"),
        Path::new("Constructor.svelte"),
        target,
    )
    .map(|module| module.source)
}
fn without_type_assertions(source: &str) -> String {
    use oxc_allocator::{Allocator, CloneIn};
    use oxc_ast::ast::Expression;
    use oxc_ast_visit::{walk_mut, VisitMut};
    let allocator = Allocator::default();
    let mut parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty());
    struct Strip<'a>(&'a Allocator);
    impl<'a> VisitMut<'a> for Strip<'a> {
        fn visit_expression(&mut self, expression: &mut Expression<'a>) {
            match expression {
                Expression::TSAsExpression(inner) => {
                    *expression = inner.expression.clone_in(self.0)
                }
                Expression::TSSatisfiesExpression(inner) => {
                    *expression = inner.expression.clone_in(self.0)
                }
                _ => {}
            }
            walk_mut::walk_expression(self, expression);
        }
    }
    Strip(&allocator).visit_program(&mut parsed.program);
    oxc_codegen::Codegen::new().build(&parsed.program).code
}
#[test]
fn public_private_and_four_runes_initialize_in_constructor_order() {
    let source="class Model{value:number;#raw:{n:number};d:number;#by:number;constructor(n=1){this.value=$state(n);this.#raw=$state.raw({n});this.d=$derived(this.value*2);this.#by=$derived.by(()=>this.value+1)}read(){return [this.value,this.#raw.n,this.d,this.#by]}inc(){this.value++;this.#raw={n:2}}}const model=new Model();";
    let dom = without_type_assertions(
        &compile(source, MoonshineModuleTarget::Dom).expect("constructor first assignments"),
    );
    for helper in ["createState", "createPrivateRawState", "createDerived"] {
        assert!(dom.contains(helper), "{dom}")
    }
    assert!(dom.contains("#raw: "), "{dom}");
    assert!(dom.contains("this.#raw = "), "{dom}");
    let constructor = dom.find("constructor(").expect("constructor retained");
    assert!(
        constructor < dom.find("this.#raw = ").expect("raw assignment retained"),
        "{dom}"
    );
    assert!(dom.contains("this.#raw.value"), "{dom}");
    assert!(dom.contains("get value()"), "{dom}");
    let ssr =
        compile(source, MoonshineModuleTarget::Ssr).expect("server constructor first assignments");
    assert!(!ssr.contains("createState"), "{ssr}");
    assert!(ssr.contains("this.value = n"), "{ssr}");
    assert!(ssr.contains("this.#raw = {"), "{ssr}");
    assert!(ssr.contains("createServerDerived"), "{ssr}");
}
#[test]
fn private_brand_and_direct_constructor_reads_keep_their_context() {
    let source="class Model{before=#n in this;#n:number;#d:number;after=#n in this;constructor(n=1){this.#n=$state(n);const direct=this.#n;this.#d=$derived(this.#n*2);(()=>this.#n)();this.#n+=1;this.#n++;}read(){return [this.#n,this.#d]}}const model=new Model();";
    let dom = without_type_assertions(
        &compile(source, MoonshineModuleTarget::Dom).expect("physical private storage"),
    );
    assert!(
        dom.find("before = #n in this").unwrap() < dom.find("#n:").unwrap(),
        "{dom}"
    );
    assert!(
        dom.find("#n:").unwrap() < dom.find("after = #n in this").unwrap(),
        "{dom}"
    );
    assert!(dom.contains("const direct = this.#n.peek()"), "{dom}");
    assert!(dom.contains("=> this.#n.value * 2"), "{dom}");
    assert!(dom.contains("() => this.#n.value"), "{dom}");
    assert!(dom.contains("this.#n.assign(this.#n.peek() + 1)"), "{dom}");
    assert!(dom.contains("this.#n.value++"), "{dom}");
}
#[test]
fn inherited_super_and_undeclared_public_assignment_keep_runtime_order() {
    let source="class Base{value;constructor(n){this.value=$state(n)}}class Model extends Base{#n;constructor(){super(1);this.order=['first'];this.extra=$state.raw(2);this.#n=$state(3);this.order.push('last')}read(){return [this.value,this.extra,this.#n]}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile_moonshine_svelte(
            &format!("<script>{source}</script><p>{{JSON.stringify(model.read())}}</p>"),
            Path::new("ConstructorJS.svelte"),
            target,
        )
        .expect("base and derived JavaScript constructors")
        .source;
        assert!(
            out.find("super(1)").unwrap() < out.find("this.order =").unwrap(),
            "{out}"
        );
        assert!(
            out.find("this.order =").unwrap() < out.find("this.#n =").unwrap(),
            "{out}"
        );
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(
                !out.contains("extra;"),
                "undeclared public property must not acquire a field: {out}"
            );
            assert!(out.contains("this.extra = 2"), "{out}");
        }
    }
}
#[test]
fn annotations_zero_arguments_generics_and_comments_are_retained() {
    let source="class Model{/** retained field contract */value:string|number;#optional:number|undefined;constructor(){/* retained initialization point */this.value=$state<string|number>(1);this.#optional=$state.raw<number>()}read(){return [this.value,this.#optional]}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile(source, target).expect("typed constructor fields");
        assert!(out.contains("retained field contract"), "{out}");
        assert!(out.contains("retained initialization point"), "{out}");
        assert!(out.contains("number | undefined"), "{out}");
        assert!(out.contains("string | number"), "{out}");
    }
}
#[test]
fn skipped_initialization_and_later_ordinary_writes_remain_source_operations() {
    let source="class Model{#n:number;constructor(skip=false){if(skip)return;this.#n=$state(1);this.#n=2;this.#n+=1}read(){return this.#n}}const model=new Model();";
    let out = without_type_assertions(
        &compile(source, MoonshineModuleTarget::Dom).expect("early return and later writes"),
    );
    assert!(
        out.find("if (skip)").unwrap() < out.find("this.#n =").unwrap(),
        "{out}"
    );
    assert!(out.contains("this.#n.assign(2)"), "{out}");
    assert!(out.contains("this.#n.assign(this.#n.peek() + 1)"), "{out}");
}
#[test]
fn duplicate_prior_initialization_and_unimplemented_modifiers_reject() {
    for (source,expected) in [
        ("class Model{x;constructor(){this.x=0;this.x=$state(1)}read(){return this.x}}const model=new Model();","prior assignment"),
        ("class Model{x;constructor(){this.x=$state(1);this.x=$state(2)}read(){return this.x}}const model=new Model();","prior assignment"),
        ("class Model{x=0;constructor(){this.x=$state(1)}read(){return this.x}}const model=new Model();","already has an initializer"),
        ("class Model{x=$state(0);constructor(){this.x=$state(1)}read(){return this.x}}const model=new Model();","already has an initializer"),
        ("class Model{readonly x:number;constructor(){this.x=$state(1)}read(){return this.x}}const model=new Model();","unsupported constructor rune field modifiers"),
        ("class Model{#fn;constructor(){this.#fn=$state(()=>1);this.#fn()}read(){return this.#fn}}const model=new Model();","constructor private state/raw call"),
    ]{for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr]{let result=compile(source,target);if expected=="constructor private state/raw call"&&matches!(target,MoonshineModuleTarget::Ssr){result.expect("server raw receiver remains ordinary");}else{let error=result.expect_err(expected);assert!(error.contains(expected),"{error}");}}}
}
#[test]
fn shadowed_macro_and_ordinary_constructor_fields_are_not_reclassified() {
    let source="const $state=(n:number)=>n;class Model{x:number;constructor(){this.x=$state(1)}read(){return this.x}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile(source, target).expect("ordinary local function");
        assert!(!out.contains("createState"), "{out}");
        assert!(!out.contains("get x()"), "{out}");
    }
}

#[test]
fn prior_writes_follow_control_flow_and_updates_but_not_deferred_bodies() {
    for before in [
        "if(flag)this.x=1;",
        "this.x++;",
        "this['x']=1;",
        "for(let n=0;n<1;n++)this.x=1;",
        "flag&&(this.x=1);",
        "class Inner extends (this.x=1,Object){}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source=format!("class Model{{x:number;constructor(flag=false){{{before}this.x=$state(2)}}read(){{return this.x}}}}const model=new Model();");
            let error = compile(&source, target).expect_err("prior constructor write");
            assert!(error.contains("prior assignment"), "{error}");
        }
    }
    for before in [
        "const write=()=>this.x=1;",
        "(()=>this.x=1)();",
        "function write(){this.x=1;}",
        "({x:this.x}={x:1});",
        "class Inner{[this.x=1](){}}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source=format!("class Model{{x:number;constructor(){{{before}this.x=$state(2)}}read(){{return this.x}}}}const model=new Model();");
            compile(&source,target).expect("pinned placement accepts deferred/destructured writes; runtime failure is not inferred");
        }
    }
}

#[test]
fn undeclared_public_constructor_fields_use_original_language_not_synthetic_tsx() {
    let body="class Model{constructor(){this.extra=$state(1)}read(){return this.extra}}const model=new Model();";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile_moonshine_svelte(
            &format!("<script>{body}</script><p>{{model.read()}}</p>"),
            Path::new("JavaScript.svelte"),
            target,
        )
        .expect("JavaScript permits undeclared constructor properties");
        let error =
            compile(body, target).expect_err("do not invent an authored TypeScript property");
        assert!(
            error.contains("undeclared public constructor rune field in TypeScript: extra"),
            "{error}"
        );
        let companion =
            "export class Model{constructor(){this.extra=$state(1)}read(){return this.extra}}";
        compile_moonshine_module(companion, Path::new("model.svelte.js"), target)
            .expect("original companion JS language");
        let error = compile_moonshine_module(companion, Path::new("model.svelte.ts"), target)
            .expect_err("original companion TS language");
        assert!(
            error.contains("undeclared public constructor rune field in TypeScript"),
            "{error}"
        );
    }
}
#[test]
fn mixed_module_and_instance_language_slices_share_one_binding_scope() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for source in [
            "<script module>export class Model{constructor(){this.extra=$state(1)}read(){return this.extra}}</script><script lang='ts'>const model=new Model();</script><p>{model.read()}</p>",
            "<script module lang='ts'>export const shared:number=1;</script><script>class Model{constructor(){this.extra=$state(shared)}read(){return this.extra}}const model=new Model();</script><p>{model.read()}</p>",
        ] {compile_moonshine_svelte(source,Path::new("Mixed.svelte"),target).expect("only the class's original block language applies");}
        let source="<script module lang='ts'>export class Model{constructor(){this.extra=$state(1)}read(){return this.extra}}</script><script>const model=new Model();</script><p>{model.read()}</p>";
        assert!(
            compile_moonshine_svelte(source, Path::new("MixedTS.svelte"), target)
                .expect_err("module TypeScript property remains undeclared")
                .contains("undeclared public constructor rune field in TypeScript")
        );
        // This is an ordinary local function call. Preserve the existing emitted
        // TS error for its undeclared property rather than synthesizing a getter.
        let shadowed="<script module lang='ts'>const $state=(n:number)=>n;</script><script lang='ts'>class Model{constructor(){this.extra=$state(1)}read(){return this.extra}}const model=new Model();</script><p>{model.read()}</p>";
        let output = compile_moonshine_svelte(shadowed, Path::new("Shadowed.svelte"), target)
            .expect("combined module binding shadows the macro")
            .source;
        assert!(!output.contains("get extra()"), "{output}");
        assert!(!output.contains("createState"), "{output}");
    }
}

#[test]
fn server_state_generics_preserve_macro_return_type_in_initializers_and_constructors() {
    for source in [
        "class Model{value:number=$state<string|number>(1);raw:number=$state.raw<string|number>(1);#privateValue:number=$state<string|number>(1);#privateRaw:number=$state.raw<string|number>(1);read(){return this.value}}const model=new Model();",
        "class Model{value:number;raw:number;#privateValue:number;#privateRaw:number;constructor(){this.value=$state<string|number>(1);this.raw=$state.raw<string|number>(1);this.#privateValue=$state<string|number>(1);this.#privateRaw=$state.raw<string|number>(1)}read(){return this.value}}const model=new Model();",
    ] {
        let out=compile(source,MoonshineModuleTarget::Ssr).expect("source type checking remains a consumer responsibility");
        assert_eq!(out.matches("satisfies string | number) as string | number").count(),4,"{out}");
    }
}

#[test]
fn constructor_storage_preserves_original_alias_in_definite_initialization_type() {
    let source="type Maybe=number|undefined;class Model{value:Maybe;#raw:Maybe;constructor(skip=true){if(skip)return;this.value=$state<Maybe>(1);this.#raw=$state.raw<Maybe>(1)}read(){return this.#raw}}const model=new Model();";
    let out = compile(source, MoonshineModuleTarget::Dom)
        .expect("optional source fields need no definite-assignment assertion");
    assert_eq!(out.matches("} | Maybe;").count(), 2, "{out}");
    assert!(out.contains("satisfies {"), "{out}");
    assert!(!out.contains("#raw!:"), "{out}");
    assert!(out.contains("this.#raw as {"), "{out}");
}

#[test]
fn undeclared_public_accessors_precede_original_computed_members() {
    for member in ["[key](){return 9}", "get [key](){return 9}"] {
        let source=format!("const key='x';class Model{{{member};constructor(){{this.x=$state(1)}}read(){{return this.x}}}}const model=new Model();");
        let out = compile_moonshine_svelte(
            &format!("<script>{source}</script><p/>"),
            Path::new("ComputedJS.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .expect("computed member retains source precedence")
        .source;
        assert!(
            out.find("get x()").unwrap() < out.find("[key]()").unwrap(),
            "{out}"
        );
    }
    for static_member in ["static x;", "static x(){}", "static get x(){}"] {
        let source=format!("class Model{{{static_member}constructor(){{this.x=$state(1)}}}}const model=new Model();");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile_moonshine_svelte(
                &format!("<script>{source}</script><p/>"),
                Path::new("StaticJS.svelte"),
                target,
            )
            .expect("uninitialized static name does not initialize the instance rune");
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile_moonshine_svelte("<script>class Model{static x=9;#x;constructor(){this.#x=$state(1)}}const model=new Model();</script><p/>",Path::new("StaticPrivate.svelte"),target).expect("public and private names remain distinct");
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error=compile_moonshine_svelte("<script>class Model{static x=9;constructor(){this.x=$state(1)}}const model=new Model();</script><p/>",Path::new("StaticJS.svelte"),target).unwrap_err();
        assert!(error.contains("static initializer"), "{error}");
    }
}

#[test]
fn public_preinitialization_reads_keep_dispatch_and_typescript_flow_checks() {
    let source="class Model{value:number;before=this.value;constructor(){const direct=this['value'];const deferred=()=>this.value;this.value=$state(1)}read(){return this.value}}const model=new Model();";
    let out = compile(source, MoonshineModuleTarget::Dom)
        .expect("preinitialization reads remain source operations");
    assert!(out.contains(" in this && this.#"), "{out}");
    assert!(out.contains(", this.value)"), "{out}");
    assert!(out.contains(", this[\"value\"])"), "{out}");
    assert!(out.contains("const deferred = () => this.value"), "{out}");
    assert!(out.contains("return this.value;"), "{out}");
}

#[test]
fn initializer_accessors_also_precede_authored_computed_getters() {
    for source in [
        "const key='x';class Model{get[key](){return 9}x=$state(1)}const model=new Model();",
        "const key='x';class Model{get['x'](){return 9}x=$state(1)}const model=new Model();",
    ] {
        let out = compile_moonshine_svelte(
            &format!("<script>{source}</script><p/>"),
            Path::new("InitializerJS.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .expect("computed getter overrides generated accessor")
        .source;
        assert!(
            out.find("get x()").unwrap() < out.find("get [").unwrap(),
            "{out}"
        );
    }
}

#[test]
fn typed_companion_constructor_initializers_preserve_generic_arrow_syntax() {
    for callable in ["<T>(value:T)=>value", "function<T>(value:T){return value}"] {
        let source=format!("export class Model{{fn:<T>(value:T)=>T;constructor(){{this.fn=$state({callable})}}read<T>(value:T){{return this.fn(value)}}}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = compile_moonshine_module(&source, Path::new("factory.svelte.ts"), target)
                .expect("raw companion parses TypeScript before TSX disambiguation")
                .source;
            assert!(out.contains("<T>"), "{out}");
        }
    }
}
