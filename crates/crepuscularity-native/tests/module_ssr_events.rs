#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_module, compile_moonshine_svelte, MoonshineModuleTarget,
};
use std::path::Path;

fn compile(source: &str, target: MoonshineModuleTarget) -> crepuscularity_native::MoonshineModule {
    compile_moonshine_svelte(source, Path::new("events.svelte"), target).unwrap()
}
#[test]
fn server_intrinsic_handlers_do_not_evaluate_factories_but_dom_retains_them() {
    let source = "<button onclick={makeClick()} oninput={makeInput()}>go</button>";
    let server = compile(source, MoonshineModuleTarget::Ssr);
    assert!(!server.source.contains("makeClick"));
    assert!(!server.source.contains("makeInput"));
    assert!(!server.source.contains("onClick"));
    let dom = compile(source, MoonshineModuleTarget::Dom);
    assert!(dom.source.contains("makeClick()"));
    assert!(dom.source.contains("makeInput()"));
}
#[test]
fn discarded_nested_jsx_and_conditional_helpers_leave_no_overlapping_edits() {
    let source="<script>import Child from './Child';</script><main><button onclick={flag?<Child child={<button onClick={nestedFactory()}/>}/>:<span>{text}</span>}>go</button><input onchange={otherFactory()}/></main>";
    let server = compile(source, MoonshineModuleTarget::Ssr);
    assert!(!server.source.contains("nestedFactory"));
    assert!(!server.source.contains("otherFactory"));
    assert!(!server.source.contains("__crepusChoose"));
    assert!(!server.source.contains("flag"));
    assert!(server.imports.contains(&"./Child".to_owned()));
    let dom = compile(source, MoonshineModuleTarget::Dom);
    assert!(dom.source.contains("nestedFactory()"));
    assert!(dom.source.contains("otherFactory()"));
}
#[test]
fn component_event_props_and_original_dependency_metadata_are_preserved() {
    let source="<script>import Child from './Child';</script><button onclick={import('./handler').then(makeHandler)}/><Child onClick={componentFactory()}/>";
    let server = compile(source, MoonshineModuleTarget::Ssr);
    assert!(server.source.contains("componentFactory()"));
    assert!(!server.source.contains("makeHandler"));
    assert_eq!(
        server.imports,
        vec![
            "./Child",
            "./handler",
            "@tschk/moonshine-svelte/server-mount"
        ]
    );
}
#[test]
fn discarded_handlers_still_receive_full_original_validation() {
    for source in [
        "export default()=> <button onUnknown={factory()}/>",
        "export default()=> <button onClick='text'/>",
        "export default()=> <button onClick/>",
        "export default()=> <button onClick={}/>",
        "export default()=> <button onClick={factory()} onClick={other()}/>",
        "export default()=> <button onClick={<iframe/>}/>",
        "export default()=> <button onClick={<span ref={capture}/>}/>",
        "export default()=> <button onClick={import('solid-js/store')}/>",
        "export default()=> <button onClick={import(path)}/>",
        "export default()=> <button onClick={()=>{const value=1;let value=2}}/>",
    ] {
        assert!(
            compile_moonshine_module(source, Path::new("bad.tsx"), MoonshineModuleTarget::Ssr)
                .is_err(),
            "{source}"
        );
        let template = source.strip_prefix("export default()=> ").unwrap();
        // The authored frontend normalizes boolean attributes to expression values.
        if source == "export default()=> <button onClick/>" {
            continue;
        }
        assert!(
            compile_moonshine_svelte(
                template,
                Path::new("bad.svelte"),
                MoonshineModuleTarget::Ssr
            )
            .is_err(),
            "{template}"
        );
    }
}

#[test]
fn generic_jsx_server_components_keep_ordinary_javascript_argument_evaluation() {
    let out = compile_moonshine_module(
        "export default()=> <button onClick={factory()}/>",
        Path::new("generic.tsx"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(out.source.contains("factory()"));
}
