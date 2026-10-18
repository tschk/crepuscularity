use crepuscularity_core::parser::{parse_svelte_component, parse_svelte_module};

#[test]
fn authored_counter_preserves_scripts_javascript_and_component_props() {
    let source = r#"<script lang="ts">
import Child from './Child.svelte';
let count = $state(0);
const doubled = $derived(count * 2);
</script>
<button class="counter" onclick={() => count++}>Count: {count}</button>
<Child value={doubled} {count} onClick={() => count += 2}/>
"#;
    let parsed = parse_svelte_module(source).unwrap();
    assert!(parsed.script.unwrap().contains("let count = $state(0)"));
    assert!(parsed.module_script.is_none());
    assert!(parsed.template_jsx.contains("className=\"counter\""));
    assert!(parsed.template_jsx.contains("onClick={() => count++}"));
    assert!(parsed
        .template_jsx
        .contains("<Child value={doubled} count={count}"));
    assert!(parsed
        .template_jsx
        .contains(r#"parts={["Count: ",()=> (count)]}"#));
    assert!(
        parse_svelte_component(source).is_err(),
        "template-only component rejection stays intact"
    );
}

#[test]
fn module_scope_if_chains_and_literal_text_are_preserved() {
    let parsed = parse_svelte_module(r#"<script module>export const name = 'Module';</script><script>let count = $state(0);</script>{#if count > 2}<p>  "quoted" &amp; 雪  </p>{:else if count > 0}<UI.Child title="&quot;literal&quot;"/>{:else}<><span>Zero</span></>{/if}"#).unwrap();
    assert!(parsed.module_script.unwrap().contains("export const name"));
    assert!(parsed.template_jsx.contains("(count > 2) ?"));
    assert!(parsed.template_jsx.contains("(count > 0) ?"));
    assert!(parsed.template_jsx.contains("&amp;"));
    assert!(parsed
        .template_jsx
        .contains(r#"literal parts={[{text:"  \"quoted\" &amp; 雪  "}]}"#));
    assert!(parsed
        .template_jsx
        .contains("UI.Child title=\"&quot;literal&quot;\""));
    assert!(!parsed.template_jsx.contains("{:else"));
}

#[test]
fn unsupported_constructs_and_malformed_markup_are_errors() {
    for source in [
        "<style>p {color:red}</style><p>Text</p>",
        "<script lang='coffee'></script><p/>",
        "<script src='./code.js'></script><p/>",
        "<script lang={kind}></script><p/>",
        "<script module={false}></script><p/>",
        "{#each items as item}<p>{item}</p>",
        "<p bind:value={value}/>",
        "<p on:click={handler}/>",
        "<p use:action|once/>",
        "<slot/>",
        "<svelte:component this={View}/>",
        "<p><span></p>",
        "{#if condition}<p/>",
        "{:else}<p/>",
        "{#if condition}<p/>{:else}<p/>{:else}<p/>{/if}",
        "<p>{value</p>",
        "<p>{}</p>",
        "<Child {true}/>",
        "<Child {item.value}/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "accepted {source}");
    }
}

#[test]
fn authored_class_directives_are_retained_for_native_lowering() {
    let parsed = parse_svelte_module(r#"<p class="base" class:active={active}/>"#).unwrap();
    let helper = parsed.class_directive_helper.as_deref().unwrap();
    assert!(
        parsed
            .template_jsx
            .contains(&format!("className={{{helper}(")),
        "{}",
        parsed.template_jsx
    );
    assert!(
        parsed.template_jsx.contains(r#"["active"]:(active"#),
        "{}",
        parsed.template_jsx
    );
    assert_eq!(parsed.template_jsx.matches("className=").count(), 1);
}

#[test]
fn authored_await_is_retained_for_native_lowering() {
    let parsed = parse_svelte_module("{#await promise}<p>Wait</p>{/await}").unwrap();
    let marker = parsed
        .await_block_component
        .expect("await needs a native marker");
    assert!(parsed.template_jsx.contains(&marker));
}

#[test]
fn legacy_script_attribute_policy_does_not_change() {
    let source = "<script data-custom='retained'>let value=1;</script><p/>";
    assert!(parse_svelte_component(source).is_ok());
    assert!(parse_svelte_module(source).is_err());
}

#[test]
fn authored_bom_and_keyboard_events_remain_structurally_valid() {
    let parsed = parse_svelte_module("\u{feff}<input onkeydown={(event) => event.key} />").unwrap();
    assert_eq!(
        parsed.template_jsx,
        format!(
            "<><input {}=\"http://www.w3.org/1999/xhtml\" onKeyDown={{(event) => event.key}} /></>",
            parsed.intrinsic_namespace_attribute.as_deref().unwrap()
        )
    );
}

#[test]
fn quoted_attributes_interpolate_while_encoded_braces_remain_literal() {
    let parsed = parse_svelte_module(r#"<button title="count {count}" disabled="{disabled}" onclick="{handler}" data-literal="&#123;count&#125;" data-mixed="&#123;literal&#125; &amp; {count}" data-plain="A & B {count};"/>"#).unwrap();
    let jsx = parsed.template_jsx;
    assert!(
        jsx.contains(r#"title={"" + "count " + __crepusAttributeText0((count))}"#),
        "{jsx}"
    );
    assert!(jsx.contains("disabled={disabled}"), "{jsx}");
    assert!(jsx.contains("onClick={handler}"), "{jsx}");
    assert!(jsx.contains(r#"data-literal="&#123;count&#125;""#), "{jsx}");
    assert!(
        jsx.contains(r#"data-mixed={"" + "{literal} & " + __crepusAttributeText0((count))}"#),
        "{jsx}"
    );
    assert!(
        jsx.contains(r#"data-plain={"" + "A & B " + __crepusAttributeText0((count)) + ";"}"#),
        "{jsx}"
    );
    for (authored, decoded) in [
        ("&copy;", "©"),
        ("&amp", "&"),
        ("&bogus;", "&bogus;"),
        ("&amp;copy;", "&copy;"),
    ] {
        let parsed = parse_svelte_module(&format!(r#"<p title="{authored} {{count}}"/>"#)).unwrap();
        let coerce = parsed.attribute_coercion_function.as_deref().unwrap();
        let literal = serde_json::to_string(&format!("{decoded} ")).unwrap();
        assert!(
            parsed
                .template_jsx
                .contains(&format!("title={{\"\" + {literal} + {coerce}((count))}}")),
            "{}",
            parsed.template_jsx
        );
    }
    for source in [
        r#"<p title="empty {}"/>"#,
        r#"<p title="unclosed {count"/>"#,
    ] {
        assert!(parse_svelte_module(source).is_err(), "accepted {source}");
    }
}

#[test]
fn authored_key_blocks_preserve_singleton_keys_and_nested_blocks() {
    let parsed = parse_svelte_module("<script>let __CrepusKeyBlock0 = 1;</script>{#key null}{#if show}<p>{value}</p>{:else}{#key undefined}empty{/key}{/if}{/key}").unwrap();
    let helper = parsed.key_block_component.as_deref().unwrap();
    assert_eq!(helper, "__CrepusKeyBlock1");
    assert!(parsed.template_jsx.contains("each={()=>[(null)]}"));
    assert!(parsed.template_jsx.contains("each={()=>[(undefined)]}"));
    assert!(parsed.template_jsx.contains("(show) ?"));
    assert_eq!(parsed.template_jsx.matches(helper).count(), 2);
    let empty = parse_svelte_module("{#key value}{/key}").unwrap();
    assert!(empty.template_jsx.contains("children={()=> <></>}"));
    assert!(parse_svelte_module("<p>plain</p>")
        .unwrap()
        .key_block_component
        .is_none());
}

#[test]
fn authored_key_block_boundaries_are_strict() {
    for source in [
        "{#key value}",
        "{#key value}{/if}",
        "{#if value}{/key}",
        "{#key value}{:else}{/key}",
        "{#key }{/key}",
        "<p>{#key value}</p>{/key}",
        "{#key value}<p>{/key}</p>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    let parsed = parse_svelte_module(
        "{#if ready}{#key ({ text: '/key', value: key })}<!-- {/key} -->ok{/key}{/if}",
    )
    .unwrap();
    assert!(parsed.template_jsx.contains("text: '/key'"));
    assert!(!parsed.template_jsx.contains("<!--"));
}

#[test]
fn authored_key_expression_comments_and_depth_limit() {
    let parsed = parse_svelte_module("{#key value /* key comment */}<p>body</p>{/key}").unwrap();
    assert!(parsed.template_jsx.contains("value /* key comment */"));
    let source = format!("{}body{}", "{#key value}".repeat(257), "{/key}".repeat(257));
    assert!(parse_svelte_module(&source).is_err());
}

#[test]
fn authored_checked_binding_markers_preserve_targets_and_attributes() {
    let parsed = parse_svelte_module(
        "<script>let __crepusCheckedBinding0;</script><input type=\"checkbox\" bind:checked={model.checked} id=\"flag\" /><input type=\"checkbox\" bind:checked />",
    ).unwrap();
    assert!(
        parse_svelte_module("<input type=\"checkbox\" defaultChecked={true} bind:checked />")
            .is_ok()
    );
    let marker = parsed.checked_binding_attribute.as_deref().unwrap();
    assert_eq!(marker, "__crepusCheckedBinding1");
    assert!(parsed
        .template_jsx
        .contains(&format!("{marker}={{model.checked}}")));
    assert!(parsed
        .template_jsx
        .contains(&format!("{marker}={{checked}}")));
    assert!(parsed.template_jsx.contains("id=\"flag\""));
    assert_eq!(parsed.template_jsx.matches("type=\"checkbox\"").count(), 2);
    assert!(parse_svelte_module("<input type=\"checkbox\" />")
        .unwrap()
        .checked_binding_attribute
        .is_none());
}

#[test]
fn authored_checked_binding_rejects_unsupported_targets_and_values() {
    for source in [
        "<input type=\"radio\" bind:checked />",
        "<input type=\"checkbox\" checked bind:checked />",
        "<input type=\"checkbox\" {checked} bind:checked />",
        "<input type=\"checkbox\" bind:checked { checked } />",
        "<input type=\"checkbox\" bind:checked checked={false} />",
        "<input type={kind} bind:checked />",
        "<input bind:checked />",
        "<input type=\"checkbox\" bind:checked=\"checked\" />",
        "<input type=\"checkbox\" bind:checked={} />",
        "<input type=\"checkbox\" bind:checked bind:checked={other} />",
        "<input type=\"checkbox\" bind:value={value} />",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    // The existing template-only checked-binding admission remains unchanged.
    assert!(crepuscularity_core::parse_template_with_path(
        "<input type=\"checkbox\" bind:checked={checked} />",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .is_ok());
}

#[test]
fn authored_value_binding_markers_preserve_supported_controls() {
    let parsed = parse_svelte_module("<script>let __crepusValueBinding0;</script><input bind:value /><input type=\"number\" bind:value={amount} defaultValue={3}/><textarea bind:value={text}></textarea><input type=\"checkbox\" bind:checked/>").unwrap();
    let marker = parsed.value_binding_attribute.as_deref().unwrap();
    assert_eq!(marker, "__crepusValueBinding1");
    for target in ["value", "amount", "text"] {
        assert!(parsed
            .template_jsx
            .contains(&format!("{marker}={{{target}}}")));
    }
    assert!(parsed.checked_binding_attribute.is_some());
    assert!(parsed.template_jsx.contains("defaultValue={3}"));
    for kind in [
        "text",
        "search",
        "tel",
        "url",
        "email",
        "password",
        "number",
        "range",
        "date",
        "datetime-local",
        "month",
        "week",
        "time",
        "color",
    ] {
        for (binding, target) in [("bind:value", "value"), ("bind:value={field}", "field")] {
            let parsed =
                parse_svelte_module(&format!("<input type=\"{kind}\" {binding}/>")).unwrap();
            let marker = parsed.value_binding_attribute.as_deref().unwrap();
            assert!(
                parsed
                    .template_jsx
                    .contains(&format!("{marker}={{{target}}}")),
                "{kind}: {binding}"
            );
            assert!(
                parsed.template_jsx.contains(&format!("type=\"{kind}\"")),
                "{kind}"
            );
        }
    }
    for multiple in ["", " multiple", " multiple=\"\""] {
        for (binding, target) in [("bind:value", "value"), ("bind:value={field}", "field")] {
            let source = format!("<select{multiple} {binding}/>");
            let parsed = parse_svelte_module(&source).unwrap();
            let marker = parsed.value_binding_attribute.as_deref().unwrap();
            assert!(
                parsed
                    .template_jsx
                    .contains(&format!("{marker}={{{target}}}")),
                "{source}: {}",
                parsed.template_jsx
            );
            assert!(
                parsed.template_jsx.contains("<select"),
                "{source}: {}",
                parsed.template_jsx
            );
            assert_eq!(
                parsed.template_jsx.contains(" multiple="),
                !multiple.is_empty(),
                "{source}"
            );
        }
    }
    assert!(parse_svelte_module("<input />")
        .unwrap()
        .value_binding_attribute
        .is_none());
}

#[test]
fn authored_value_binding_boundaries_are_explicit() {
    for source in [
        "<input type=\"file\" bind:value/>",
        "<input type=\"checkbox\" bind:value/>",
        "<input type={kind} bind:value/>",
        "<input value=\"a\" bind:value/>",
        "<input bind:value { value }/>",
        "<input {value} bind:value/>",
        "<input bind:value value={other}/>",
        "<input bind:value bind:value={other}/>",
        "<input bind:value=\"field\"/>",
        "<input bind:value={}/>",
        "<textarea bind:value>initial</textarea>",
        "<textarea bind:value> </textarea>",
        "<textarea bind:value>{initial}</textarea>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    assert!(parse_svelte_module("<textarea bind:value defaultValue=\"initial\"/>").is_ok());
    assert!(crepuscularity_core::parse_template_with_path(
        "<input bind:value={field}/>",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .is_ok());
}

#[test]
fn authored_component_binding_marker_preserves_component_targets() {
    let parsed = parse_svelte_module("<script>let __crepusComponentBinding0;</script><Child bind:this={child}><UI.Nested bind:this={refs.nested}/></Child>").unwrap();
    let marker = parsed.component_binding_attribute.as_deref().unwrap();
    assert_eq!(marker, "__crepusComponentBinding1");
    assert!(parsed
        .template_jsx
        .contains(&format!("<Child {marker}={{child}}>")));
    assert!(parsed
        .template_jsx
        .contains(&format!("<UI.Nested {marker}={{refs.nested}}")));
    assert!(parse_svelte_module("<Child/>")
        .unwrap()
        .component_binding_attribute
        .is_none());
}

#[test]
fn authored_component_binding_rejects_nonexpressions() {
    for source in [
        "<Child bind:this/>",
        "<Child bind:this=\"child\"/>",
        "<Child bind:this={}/>",
        "<Child bind:this={child} bind:this={other}/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    // The separate template parser keeps its established bind:this rejection.
    assert!(crepuscularity_core::parse_template_with_path(
        "<Child bind:this={child}/>",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .is_err());
}

#[test]
fn authored_root_snippets_preserve_parameters_and_forward_render_calls() {
    let parsed = parse_svelte_module("<script>let __CrepusRenderSnippet0;</script>{@render row(value)}{#snippet row(item: string, { label } = fallback)}<p>{item}{label}</p>{@render tail?.()}{/snippet}{#snippet tail()}done{/snippet}").unwrap();
    assert_eq!(
        parsed.render_component.as_deref(),
        Some("__CrepusRenderSnippet1")
    );
    assert_eq!(parsed.snippets.len(), 2);
    assert_eq!(parsed.snippets[0].name, "row");
    assert_eq!(
        parsed.snippets[0].parameters,
        "(item: string, { label } = fallback)"
    );
    assert!(parsed.snippets[0]
        .template_jsx
        .contains("parts={[()=> (item),()=> (label)]}"));
    assert!(parsed.snippets[0].template_jsx.contains("tail?.()"));
    assert!(parsed.template_jsx.contains("call={()=> (row(value))}"));
    assert!(!parsed.template_jsx.contains("<p>"));
    let empty = parse_svelte_module("{#snippet café()}ひ{/snippet}").unwrap();
    assert_eq!(empty.snippets[0].name, "café");
    assert!(empty.render_component.is_none());
}

#[test]
fn authored_snippet_boundaries_are_explicit() {
    for source in [
        "{#snippet row()}unclosed",
        "{/snippet}",
        "{#snippet row()}body{/if}",
        "{#snippet row()}body{/key}",
        "{#snippet row-name()}body{/snippet}",
        "{#snippet 1row()}body{/snippet}",
        "{#snippet row}body{/snippet}",
        "{#snippet row<T>()}body{/snippet}",
        "{#snippet row()}{#snippet nested()}{/snippet}{/snippet}",
        "{#if ok}{#snippet row()}{/snippet}{/if}",
        "{#key id}{#snippet row()}{/snippet}{/key}",
        "{#snippet row()}{/snippet}{#snippet row()}{/snippet}",
        "{@render}",
        "{@render }",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    assert!(parse_svelte_module("{#snippet empty()}{/snippet}").is_ok());
    assert!(crepuscularity_core::parse_template_with_path(
        "{#snippet row()}body{/snippet}",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .is_err());
}

#[test]
fn authored_raw_html_markers_preserve_expressions_and_scope() {
    let parsed = parse_svelte_module("<script>let __CrepusRawHTML0;</script>{@html root}<div>{@html nested}</div>{#snippet row(value)}{@html value}{/snippet}").unwrap();
    let marker = parsed.raw_html_component.as_deref().unwrap();
    assert_eq!(marker, "__CrepusRawHTML1");
    assert!(parsed
        .template_jsx
        .contains(&format!("<{marker} value={{root}}/>")));
    assert!(parsed.template_jsx.contains(&format!(
        "<div {}=\"http://www.w3.org/1999/xhtml\"><{marker} value={{nested}}/></div>",
        parsed.intrinsic_namespace_attribute.as_deref().unwrap()
    )));
    assert!(parsed.snippets[0]
        .template_jsx
        .contains(&format!("<{marker} value={{value}}/>")));
    assert!(parse_svelte_module("<p>plain</p>")
        .unwrap()
        .raw_html_component
        .is_none());
}

#[test]
fn authored_raw_html_requires_a_whitespace_separated_expression() {
    for source in [
        "{@html}",
        "{@html }",
        "{@html(value)}",
        "{@htmlx}",
        "{@html value",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    // Template-only parsing continues to use its own existing raw-HTML AST.
    let nodes = crepuscularity_core::parse_template_with_path(
        "{@html value}",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .unwrap();
    assert!(
        matches!(&nodes[..], [crepuscularity_core::ast::Node::RawHtml(value)] if value == "value")
    );
}

#[test]
fn authored_textarea_rejects_raw_html_placement() {
    for source in [
        "<textarea>{@html value}</textarea>",
        "<textarea>before{@html value}after</textarea>",
        "<textarea>{#if ready}{@html value}{/if}</textarea>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    assert!(parse_svelte_module("<textarea>{value}</textarea>").is_ok());
    assert!(parse_svelte_module("<div>{@html value}</div>").is_ok());
}

#[test]
fn authored_text_chunks_group_adjacent_parts_without_setup_reads() {
    let parsed = parse_svelte_module("<script>let __CrepusText0;</script><Child>a{first}{second}b<span/>{only}</Child>{#if ready}{inside}{/if}{#key id}{keyed}{/key}{#snippet row(item)}hi {item}{/snippet}").unwrap();
    assert_eq!(parsed.text_component.as_deref(), Some("__CrepusText1"));
    assert!(parsed
        .template_jsx
        .contains("parts={[\"a\",()=> (first),()=> (second),\"b\"]}"));
    for expression in ["only", "inside", "keyed"] {
        assert!(parsed
            .template_jsx
            .contains(&format!("parts={{[()=> ({expression})]}}")));
    }
    assert!(parsed.snippets[0]
        .template_jsx
        .contains(r#"parts={["hi ",()=> (item)]}"#));
    let literal = parse_svelte_module("<p>plain &amp; literal</p>").unwrap();
    let marker = literal.text_component.unwrap();
    assert_eq!(
        literal.template_jsx,
        format!("<><p {}=\"http://www.w3.org/1999/xhtml\"><{marker} literal parts={{[{{text:\"plain &amp; literal\"}}]}}/></p></>", literal.intrinsic_namespace_attribute.as_deref().unwrap())
    );
}

#[test]
fn authored_comments_preserve_separate_parts_within_one_text_run() {
    let parsed = parse_svelte_module("<p>a{x}<!-- ignored -->b{y}</p>").unwrap();
    assert!(parsed
        .template_jsx
        .contains(r#"parts={["a",()=> (x),"b",()=> (y)]}"#));
    assert!(!parsed.template_jsx.contains("<!--"));
}

#[test]
fn authored_mixed_text_entities_retain_raw_spelling_for_native_decoding() {
    let parsed =
        parse_svelte_module("<p>&copy; &unknown; &amp;{value}&#123;literal&#125;</p>").unwrap();
    assert!(parsed.template_jsx.contains(
        r#"parts={[{text:"&copy; &unknown; &amp;"},()=> (value),{text:"&#123;literal&#125;"}]}"#
    ));
}

#[test]
fn mixed_quoted_attribute_markers_are_hygienic_and_keep_sole_values_raw() {
    let parsed = parse_svelte_module(r#"<script>let __crepusAttributeText0;</script><p title="a{first()}b{second()}" data-raw="{value}" data-null="x{null}" data-symbol="x{symbol}"/>"#).unwrap();
    assert_eq!(
        parsed.attribute_coercion_function.as_deref(),
        Some("__crepusAttributeText1")
    );
    assert!(parsed.template_jsx.contains(r#"title={"" + "a" + __crepusAttributeText1((first())) + "b" + __crepusAttributeText1((second()))}"#));
    assert!(parsed.template_jsx.contains("data-raw={value}"));
    assert!(parsed
        .template_jsx
        .contains("__crepusAttributeText1((null))"));
    assert!(parsed
        .template_jsx
        .contains("__crepusAttributeText1((symbol))"));
    assert!(parse_svelte_module(r#"<p title="{value}"/>"#)
        .unwrap()
        .attribute_coercion_function
        .is_none());
    assert!(parse_svelte_module(r#"<p title="literal"/>"#)
        .unwrap()
        .attribute_coercion_function
        .is_none());
}

#[test]
fn mixed_quoted_attribute_comma_expression_remains_one_marker_argument() {
    let parsed = parse_svelte_module(r#"<p title="x{first(), second()}"/>"#).unwrap();
    assert!(parsed
        .template_jsx
        .contains(r#"title={"" + "x" + __crepusAttributeText0((first(), second()))}"#));
}

#[test]
fn bare_attribute_boolean_words_are_strings_not_binding_shorthand() {
    let parsed = parse_svelte_module("<Child value=true other=false braced={true} flag/>").unwrap();
    assert!(parsed
        .template_jsx
        .contains("value=\"true\" other=\"false\" braced={true} flag={true}"));
    for source in [
        "<Child bind:open=true/>",
        "<Child bind:open=false/>",
        "<input bind:value=true/>",
        "<input type=\"checkbox\" bind:checked=true/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
}

#[test]
fn authored_component_spreads_preserve_source_order_and_grouping() {
    let parsed = parse_svelte_module("<Child first={one} {...props} middle={two} {...(left, right)} last={three}/><UI.Child {...nested}/>").unwrap();
    assert!(parsed.template_jsx.contains(
        "<Child first={one} {...(props)} middle={two} {...((left, right))} last={three}"
    ));
    assert!(parsed.template_jsx.contains("<UI.Child {...(nested)}"));
}

#[test]
fn authored_spread_boundaries_remain_explicit() {
    for source in [
        "<Child {...}/>",
        "<Child {...props}=true/>",
        "<Child {...props/>",
        "<Child first={one} {...props} first={two}/>",
        "<script {...props}>let value=1;</script><Child/>",
        "<style {...props}>p{}</style><Child/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
    assert!(crepuscularity_core::parse_template_with_path(
        "<Child {...props}/>",
        Some(std::path::Path::new("fixture.svelte")),
    )
    .is_err());
}

#[test]
fn authored_component_value_bindings_use_a_separate_hygienic_marker() {
    let parsed = parse_svelte_module("<script>let __crepusComponentBindingProp0;</script><Field bind:value/><UI.Field bind:value={model.value}/><input bind:value={text}/>").unwrap();
    let marker = parsed.component_bindings[0].0.as_str();
    assert_eq!(marker, "__crepusComponentBindingProp1");
    assert!(parsed
        .template_jsx
        .contains(&format!("<Field {marker}={{(value\n)}}")));
    assert!(parsed
        .template_jsx
        .contains(&format!("<UI.Field {marker}={{(model.value\n)}}")));
    assert!(parsed.value_binding_attribute.is_some());
    assert_ne!(parsed.value_binding_attribute.as_deref(), Some(marker));
    assert!(parse_svelte_module("<input bind:value/>")
        .unwrap()
        .component_bindings
        .is_empty());
}

#[test]
fn authored_component_value_binding_conflicts_and_invalid_values_are_errors() {
    for source in [
        "<Field bind:value value={other}/>",
        "<Field value={other} bind:value/>",
        "<Field {value} bind:value/>",
        "<UI.Field bind:value { value }/>",
        "<Field bind:value bind:value={other}/>",
        r#"<Field bind:value="value"/>"#,
        "<Field bind:value={}/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
}

#[test]
fn authored_named_component_bindings_reuse_markers_in_first_property_order() {
    let checked = parse_svelte_module(r#"<Check type="checkbox" bind:checked />"#).unwrap();
    assert_eq!(checked.component_bindings[0].1, "checked");
    assert!(checked.checked_binding_attribute.is_none());
    let parsed = parse_svelte_module("<Field bind:open bind:checked={flag}/><UI.Field bind:open={other} bind:custom={model.value}/>").unwrap();
    assert_eq!(
        parsed
            .component_bindings
            .iter()
            .map(|(_, property)| property.as_str())
            .collect::<Vec<_>>(),
        ["open", "checked", "custom"]
    );
    let open = &parsed.component_bindings[0].0;
    assert!(parsed
        .template_jsx
        .contains(&format!("{open}={{(open\n)}}")));
    assert!(parsed
        .template_jsx
        .contains(&format!("{open}={{(other\n)}}")));
    assert!(parsed.checked_binding_attribute.is_none());
    for source in [
        "<Field open={value} bind:open/>",
        "<Field bind:open {open}/>",
        "<Field bind:open bind:open={other}/>",
        "<Field bind:custom=\"value\"/>",
        "<Field bind:custom={}/>",
        "<Field bind:={target}/>",
        "<Field bind:bad:name={target}/>",
        "<Field bind:aria-label/>",
        "<Field bind:bad-name/>",
        "<input bind:custom={value}/>",
    ] {
        assert!(parse_svelte_module(source).is_err(), "{source}");
    }
}

#[test]
fn authored_component_binding_names_do_not_inherit_intrinsic_attribute_bans() {
    for property in [
        "ref",
        "children",
        "innerHTML",
        "outerHTML",
        "dangerouslySetInnerHTML",
        "__proto__",
        "aria-label",
    ] {
        let parsed = parse_svelte_module(&format!("<Field bind:{property}={{target}}/>")).unwrap();
        assert_eq!(parsed.component_bindings[0].1, property);
    }
}

#[test]
fn authored_component_binding_targets_have_exactly_one_transport_parenthesis() {
    let parsed =
        parse_svelte_module("<Field bind:value={()=>model.value, next=>model.value=next}/>")
            .unwrap();
    let marker = &parsed.component_bindings[0].0;
    assert!(parsed.template_jsx.contains(&format!(
        "{marker}={{(()=>model.value, next=>model.value=next\n)}}"
    )));
    // Preserve authored parentheses so native validation can distinguish invalid
    // parenthesized pairs from the single generated transport layer.
    let parsed = parse_svelte_module("<Field bind:value={(get,set)}/>").unwrap();
    assert!(parsed.template_jsx.contains("={((get,set)\n)}"));
    let parsed =
        parse_svelte_module("<Field bind:value={model.value // trailing comment\n}/>").unwrap();
    assert!(parsed
        .template_jsx
        .contains("={(model.value // trailing comment\n\n)}"));
}

#[test]
fn authored_text_markers_retain_raw_formatting_for_native_fragment_cleanup() {
    let parsed = parse_svelte_module("\n  <p>\n\tfirst {value}\n last \t</p>\n").unwrap();
    let marker = parsed.text_component.unwrap();
    assert!(
        parsed
            .template_jsx
            .starts_with(&format!("<><{marker} literal parts={{[\"\\n  \"]}}/>")),
        "{}",
        parsed.template_jsx
    );
    assert!(
        parsed
            .template_jsx
            .contains(r#"parts={["\n\tfirst ",()=> (value),"\n last \t"]}"#),
        "{}",
        parsed.template_jsx
    );
    assert!(
        parsed
            .template_jsx
            .ends_with(&format!("<{marker} literal parts={{[\"\\n\"]}}/></>")),
        "{}",
        parsed.template_jsx
    );
}

#[test]
fn authored_static_text_uses_literal_markers_without_collapsing_or_decoding() {
    let parsed = parse_svelte_module("<p>  plain\n\t</p><p>&#10; &#32;literal</p>").unwrap();
    assert!(parsed
        .template_jsx
        .contains(r#"literal parts={["  plain\n\t"]}"#));
    assert!(parsed
        .template_jsx
        .contains(r#"literal parts={[{text:"&#10; &#32;literal"}]}"#));
    assert!(!parsed.template_jsx.contains("{\"  plain"));
}

#[test]
fn authored_intrinsic_references_keep_namespace_and_target_metadata() {
    for tag in ["div", "input"] {
        let parsed = parse_svelte_module(&format!("<{tag} bind:this={{element}}/>")).unwrap();
        let reference = parsed.intrinsic_binding_attribute.as_deref().unwrap();
        let namespace = parsed.intrinsic_namespace_attribute.as_deref().unwrap();
        assert_eq!(parsed.template_jsx, format!("<><{tag} {namespace}=\"http://www.w3.org/1999/xhtml\" {reference}={{(element)}} /></>"));
        assert!(parsed.component_binding_attribute.is_none());
    }
}

#[test]
fn authored_intrinsic_snippets_stay_local_to_the_element_children() {
    let parsed =
        parse_svelte_module("<div>{#snippet row()}inside{/snippet}{@render row()}</div>").unwrap();
    assert!(parsed.snippets.is_empty());
    assert_eq!(parsed.inline_snippet_names.len(), 1);
    let name = &parsed.inline_snippet_names[0];
    let jsx = &parsed.template_jsx;
    assert!(jsx.contains(&format!("function {name}()")), "{jsx}");
    assert!(
        jsx.contains(&format!("const row={name};return <>")),
        "{jsx}"
    );
    assert!(jsx.contains("call={()=> (row())}"), "{jsx}");
    assert!(!jsx.contains(" row={row}"), "{jsx}");
}

#[test]
fn authored_intrinsic_spreads_preserve_admitted_template_layers() {
    for tag in ["p", "div", "input"] {
        let source = format!("<{tag} title=\"before\" {{...props}} data-tail=\"after\"/>");
        let parsed = parse_svelte_module(&source).unwrap();
        let output = &parsed.template_jsx;
        assert!(output.contains("{...(props)}"), "{output}");
        assert!(
            output.find("before").unwrap() < output.find("{...(props)}").unwrap(),
            "{output}"
        );
        assert!(
            output.find("{...(props)}").unwrap() < output.find("after").unwrap(),
            "{output}"
        );
        // Admission belongs to the authored compiler, not extraction-only shared AST.
        assert!(crepuscularity_core::parse_template_with_path(
            &source,
            Some(std::path::Path::new("fixture.svelte"))
        )
        .is_err());
    }
}
