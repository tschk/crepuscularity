//! Attribute-only element merging.

use super::super::RawParseError;
use crate::ast::*;

use super::element::parse_element_line;

pub(crate) fn merge_attr_only_children(mut el: Element) -> Result<Element, RawParseError> {
    let mut kept = Vec::new();
    for child in std::mem::take(&mut el.children) {
        if let Node::Element(c) = child {
            if is_attr_only_element(&c) {
                hoist_attr_element(&mut el, &c)?;
                continue;
            }
            kept.push(Node::Element(merge_attr_only_children(c)?));
        } else {
            kept.push(child);
        }
    }
    el.children = kept;
    Ok(el)
}

fn is_attr_only_element(el: &Element) -> bool {
    el.tag.contains('=') && el.children.is_empty()
}

fn hoist_attr_element(parent: &mut Element, attr_el: &Element) -> Result<(), RawParseError> {
    let mut fake = parse_element_line(&format!("div {}", attr_el.tag), vec![])?;
    let id_count = [parent as &Element, &fake, attr_el]
        .into_iter()
        .map(|element| {
            usize::from(element.id.is_some())
                + element
                    .bindings
                    .iter()
                    .filter(|binding| binding.prop == "id")
                    .count()
        })
        .sum::<usize>();
    if id_count > 1 {
        return Err(RawParseError {
            message: "an element may have only one id attribute".into(),
            byte_offset: None,
        });
    }
    fake.id = fake.id.or_else(|| attr_el.id.clone());
    fake.bindings.extend(attr_el.bindings.iter().cloned());
    fake.classes.extend(attr_el.classes.iter().cloned());
    fake.conditional_classes
        .extend(attr_el.conditional_classes.iter().cloned());
    fake.event_handlers
        .extend(attr_el.event_handlers.iter().cloned());
    fake.animations.extend(attr_el.animations.iter().cloned());
    parent.bindings.extend(fake.bindings);
    parent.conditional_classes.extend(fake.conditional_classes);
    parent.event_handlers.extend(fake.event_handlers);
    parent.animations.extend(fake.animations);
    if let Some(id) = fake.id {
        parent.id = Some(id);
    }
    parent.classes.extend(fake.classes);
    Ok(())
}
