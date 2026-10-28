//! AST-validated Svelte stylesheet transforms.
#[path = "svelte_css/scoped.rs"]
pub(crate) mod scoped;
use raffia::{ast::*, Parser, Syntax};

const UNSUPPORTED: &str =
    "authored Svelte styles are unsupported unless every rule is explicitly global CSS";
const COMPLEXITY_LIMIT: usize = 256;

#[cfg(test)]
pub(crate) fn transform(source: &str) -> Result<Option<String>, String> {
    let sheet = parse_stylesheet(source)?;
    transform_global(source, &sheet)
}
pub(super) fn validate_source(source: &str) -> Result<(), String> {
    // Raffia has no recursion limit. Conservatively bound parser work before it
    // enters recursive selector/value grammar, including delimiters in literals.
    if source
        .bytes()
        .filter(|byte| matches!(byte, b'{' | b'(' | b'['))
        .count()
        > COMPLEXITY_LIMIT
    {
        return Err("native Svelte global CSS exceeds the bounded parser complexity limit".into());
    }
    if source.contains('\\')
        || source.to_ascii_lowercase().contains("url(")
        || source.to_ascii_lowercase().contains("image-set(")
    {
        return Err(
            "native Svelte global CSS escapes and external references are unsupported".into(),
        );
    }
    Ok(())
}
/// Classification follows one validated parse; diagnostics never select a fallback.
#[derive(Debug)]
pub(crate) enum ClassifiedStyle<'s> {
    Global(Option<String>),
    Local(Stylesheet<'s>),
}
pub(crate) fn classify(source: &str) -> Result<ClassifiedStyle<'_>, String> {
    let sheet = parse_stylesheet(source)?;
    let global_only = sheet.statements.iter().all(|statement| match statement {
        Statement::QualifiedRule(rule) => rule
            .selector
            .selectors
            .iter()
            .all(|selector| global_shape(selector).is_some()),
        _ => false,
    });
    if global_only {
        Ok(ClassifiedStyle::Global(transform_global(source, &sheet)?))
    } else {
        // The initial local slice owns flat qualified rules only. Validate its
        // declaration boundary before markup analysis, including unused rules.
        for statement in &sheet.statements {
            let Statement::QualifiedRule(rule) = statement else {
                return Err("unsupported scoped CSS at-rule (analysis pending)".into());
            };
            declarations(&rule.block)?;
        }
        Ok(ClassifiedStyle::Local(sheet))
    }
}
fn parse_stylesheet(source: &str) -> Result<Stylesheet<'_>, String> {
    validate_source(source)?;
    stacker::grow(8 * 1024 * 1024, || {
        let mut parser = Parser::new(source, Syntax::Css);
        let sheet = parser
            .parse::<Stylesheet>()
            .map_err(|error| format!("invalid native Svelte CSS: {error:?}"))?;
        if !parser.recoverable_errors().is_empty() {
            return Err(format!(
                "invalid native Svelte CSS: {:?}",
                parser.recoverable_errors()
            ));
        }
        Ok(sheet)
    })
}
fn transform_global(source: &str, sheet: &Stylesheet<'_>) -> Result<Option<String>, String> {
    let mut edits = Vec::new();
    for statement in &sheet.statements {
        let Statement::QualifiedRule(rule) = statement else {
            return Err(UNSUPPORTED.into());
        };
        let globals = rule
            .selector
            .selectors
            .iter()
            .map(global)
            .collect::<Result<Vec<_>, _>>()?;
        if globals.len() == 1 && globals[0].arg.is_none() {
            for statement in &rule.block.statements {
                let Statement::QualifiedRule(child) = statement else {
                    return Err(UNSUPPORTED.into());
                };
                ordinary_selectors(&child.selector)?;
                declarations(&child.block)?;
            }
            edits.push((rule.selector.span.start, rule.block.span.start + 1));
            edits.push((rule.block.span.end - 1, rule.span.end));
        } else {
            for global in globals {
                let Some(arg) = &global.arg else {
                    return Err(UNSUPPORTED.into());
                };
                let PseudoClassSelectorArgKind::SelectorList(selectors) = &arg.kind else {
                    return Err(UNSUPPORTED.into());
                };
                if selectors.selectors.len() != 1 {
                    return Err(UNSUPPORTED.into());
                }
                ordinary_selectors(selectors)?;
                edits.push((global.span.start, arg.l_paren.end));
                edits.push((arg.r_paren.start, global.span.end));
            }
            declarations(&rule.block)?;
        }
    }
    let mut output = source.to_owned();
    edits.sort_unstable();
    for (start, end) in edits.into_iter().rev() {
        if output.get(start..end).is_none() {
            return Err("invalid CSS edit span".into());
        }
        output.replace_range(start..end, "");
    }
    Ok((!sheet.statements.is_empty()).then_some(output))
}
fn global_shape<'a, 's>(selector: &'a ComplexSelector<'s>) -> Option<&'a PseudoClassSelector<'s>> {
    let [ComplexSelectorChild::CompoundSelector(compound)] = selector.children.as_slice() else {
        return None;
    };
    let [SimpleSelector::PseudoClass(pseudo)] = compound.children.as_slice() else {
        return None;
    };
    matches!(&pseudo.name,InterpolableIdent::Literal(name) if name.name=="global").then_some(pseudo)
}
fn global<'a, 's>(
    selector: &'a ComplexSelector<'s>,
) -> Result<&'a PseudoClassSelector<'s>, String> {
    global_shape(selector).ok_or_else(|| UNSUPPORTED.into())
}
fn ordinary_selectors(list: &SelectorList<'_>) -> Result<(), String> {
    for complex in &list.selectors {
        for child in &complex.children {
            let ComplexSelectorChild::CompoundSelector(compound) = child else {
                continue;
            };
            for simple in &compound.children {
                match simple {
                    SimpleSelector::Nesting(_) => return Err(UNSUPPORTED.into()),
                    SimpleSelector::PseudoClass(pseudo) => {
                        if matches!(&pseudo.name, InterpolableIdent::Literal(name) if name.name == "global")
                        {
                            return Err(UNSUPPORTED.into());
                        }
                        if let Some(arg) = &pseudo.arg {
                            match &arg.kind {
                                PseudoClassSelectorArgKind::SelectorList(list) => {
                                    ordinary_selectors(list)?
                                }
                                // Other functional pseudo classes need a separate
                                // acceptance slice before nesting is admitted.
                                _ => return Err(UNSUPPORTED.into()),
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
fn declarations(block: &SimpleBlock<'_>) -> Result<(), String> {
    for statement in &block.statements {
        let Statement::Declaration(declaration) = statement else {
            return Err(UNSUPPORTED.into());
        };
        for value in &declaration.value {
            local_value(value)?;
        }
    }
    Ok(())
}
fn local_value(value: &ComponentValue<'_>) -> Result<(), String> {
    match value {
        ComponentValue::Url(_) => {
            return Err("native Svelte global CSS external references are unsupported".into())
        }
        ComponentValue::Function(function) => {
            if matches!(&function.name, FunctionName::Ident(InterpolableIdent::Literal(name)) if name.name.eq_ignore_ascii_case("url") || name.name.eq_ignore_ascii_case("image-set") || name.name.eq_ignore_ascii_case("-webkit-image-set") || name.name.eq_ignore_ascii_case("image"))
            {
                return Err("native Svelte global CSS external references are unsupported".into());
            }
            for value in &function.args {
                local_value(value)?;
            }
        }
        ComponentValue::Calc(calc) => {
            local_value(&calc.left)?;
            local_value(&calc.right)?;
        }
        ComponentValue::BracketBlock(block) => {
            for value in &block.value {
                local_value(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_global_selector_preserves_declarations_comments_and_unicode() {
        assert_eq!(
            transform("/* :global(fake) */ :global(.rule) { content: 'héllo'; color:red; }")
                .unwrap(),
            Some("/* :global(fake) */ .rule { content: 'héllo'; color:red; }".into())
        );
    }
    #[test]
    fn explicit_global_block_unwraps_parsed_boundaries() {
        assert_eq!(
            transform(":global { .rule {content:'}';color:red} }").unwrap(),
            Some(" .rule {content:'}';color:red} ".into())
        );
    }
    #[test]
    fn separately_wrapped_selector_list_retains_order() {
        assert_eq!(
            transform(":global(.a), :global(.b:hover) {color:red}").unwrap(),
            Some(".a, .b:hover {color:red}".into())
        );
    }
    #[test]
    fn local_mixed_nested_and_external_styles_reject() {
        for source in [
            ".local{color:red}",
            ":global(.a) .local{color:red}",
            ":global(.a),.local{color:red}",
            ":global(.a,.b){color:red}",
            ":global(:global(.a)){color:red}",
            ":global{color:red}",
            "@media all{:global(.a){color:red}}",
            ":global(.a){.b{color:red}}",
            ":global(.a){background:url(x)}",
            ":global(.a){content:'\\61'}",
        ] {
            assert!(transform(source).is_err(), "{source}");
        }
    }
    #[test]
    fn invalid_and_recoverable_css_reject() {
        for source in [
            ":global(.a){color:",
            ":global(.a){color:red",
            ":global(.a){color red}",
        ] {
            assert!(transform(source).is_err(), "{source}");
        }
    }
    #[test]
    fn comments_only_and_empty_styles_emit_no_asset() {
        for source in ["", "  ", "/* comment */"] {
            assert_eq!(transform(source).unwrap(), None);
        }
    }
    #[test]
    fn deeply_nested_selector_rejects_before_recursive_parser() {
        assert!(transform(&format!(
            ":global({}.a{}){{color:red}}",
            ":is(".repeat(500),
            ")".repeat(500)
        ))
        .unwrap_err()
        .contains("complexity"));
    }
    #[test]
    fn bounded_nested_selectors_work_on_small_test_thread() {
        assert!(transform(&format!(
            ":global({}.a{}){{color:red}}",
            ":is(".repeat(200),
            ")".repeat(200)
        ))
        .is_ok());
    }
}

#[cfg(test)]
mod external_tests {
    use super::*;
    #[test]
    fn external_images_and_obfuscated_urls_reject_before_assets() {
        for source in [
            r#":global(.a){background:image-set("https://example.invalid/a.png" 1x)}"#,
            r#":global{.a{background:-webkit-image-set("https://example.invalid/a.png" 1x)}}"#,
            ":global(.a){background:url(data:image/png;base64,AA)}",
            r#":global(.a){background:u\72l(https://example.invalid/a.png)}"#,
            ":global(.a){background:var(--fallback,image-set('https://example.invalid/a.png' 1x))}",
            ":global{.a{color:red} .mixed :global(.b){color:blue}}",
        ] {
            assert!(transform(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod pseudo_element_reference_tests {
    use super::*;
    #[test]
    fn functional_pseudo_elements_keep_official_global_wrapper_semantics() {
        for selector in ["::part(foo)", "::slotted(:global(.x))"] {
            assert_eq!(
                transform(&format!(":global({selector}){{color:red}}")).unwrap(),
                Some(format!("{selector}{{color:red}}"))
            );
        }
    }
}

#[cfg(test)]
mod classification_tests {
    use super::*;
    #[test]
    fn global_classification_keeps_existing_goldens() {
        for source in [
            ":global(.a){color:red}",
            ":global{.a{color:red}}",
            ":global(::part(foo)){color:red}",
        ] {
            let ClassifiedStyle::Global(output) = classify(source).unwrap() else {
                panic!("global style misclassified")
            };
            assert_eq!(output, transform(source).unwrap());
        }
        assert!(matches!(
            classify(".a{color:red}").unwrap(),
            ClassifiedStyle::Local(_)
        ));
    }
    #[test]
    fn invalid_and_security_errors_are_not_local_dispatch() {
        for source in [
            ".a{color red}",
            ".a{background:url(x)}",
            ".a{background:image('x')}",
            ":global(.a){background:image('x')}",
        ] {
            assert!(classify(source).is_err(), "{source}");
        }
        let error = classify(&format!(".a{{{}", "(".repeat(257))).unwrap_err();
        assert!(error.contains("complexity"), "{error}");
    }
}
