use serde::{Deserialize, Serialize};

use crate::ir::{ViewIr, ViewNode, ViewStyle};

/// Renderer-agnostic IR operations, inspired by Dioxus' mutation sink model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum IrMutation {
    ReplaceRoot {
        root: Vec<ViewNode>,
    },
    ReplaceNode {
        path: Vec<usize>,
        node: ViewNode,
    },
    InsertNode {
        parent_path: Vec<usize>,
        index: usize,
        node: ViewNode,
    },
    RemoveNode {
        path: Vec<usize>,
    },
    UpdateText {
        path: Vec<usize>,
        content: String,
    },
    UpdateStyle {
        path: Vec<usize>,
        style: Option<ViewStyle>,
    },
}

/// Compute a path-based patch from one IR tree to another.
/// Patches do not carry the document version. For differing versions, use a full
/// document reload; the returned root replacement alone does not migrate it.
pub fn diff_ir(old: &ViewIr, new: &ViewIr) -> Vec<IrMutation> {
    if old.version != new.version {
        return vec![IrMutation::ReplaceRoot {
            root: new.root.clone(),
        }];
    }

    let mut out = Vec::new();
    diff_node_lists(&old.root, &new.root, &[], &mut out);
    out
}

/// Apply an ordered patch atomically; an error leaves the original [`ViewIr`] unchanged.
pub fn apply_mutations(ir: &mut ViewIr, ops: &[IrMutation]) -> Result<(), String> {
    if ops.is_empty() {
        return Ok(());
    }
    let mut next = ir.clone();
    apply_mutations_in_place(&mut next, ops)?;
    *ir = next;
    Ok(())
}

fn apply_mutations_in_place(ir: &mut ViewIr, ops: &[IrMutation]) -> Result<(), String> {
    for op in ops {
        match op {
            IrMutation::ReplaceRoot { root } => ir.root = root.clone(),
            IrMutation::ReplaceNode { path, node } => {
                let target = node_mut(ir, path)?;
                *target = node.clone();
            }
            IrMutation::InsertNode {
                parent_path,
                index,
                node,
            } => {
                let children = children_mut(ir, parent_path)?;
                if *index > children.len() {
                    return Err(format!(
                        "insert index {} out of bounds for parent path {:?}",
                        index, parent_path
                    ));
                }
                children.insert(*index, node.clone());
            }
            IrMutation::RemoveNode { path } => {
                if path.is_empty() {
                    return Err("cannot remove root list directly; use ReplaceRoot".to_string());
                }
                let parent = &path[..path.len() - 1];
                let idx = *path
                    .last()
                    .ok_or_else(|| "missing remove index".to_string())?;
                let children = children_mut(ir, parent)?;
                if idx >= children.len() {
                    return Err(format!(
                        "remove index {} out of bounds for parent path {:?}",
                        idx, parent
                    ));
                }
                children.remove(idx);
            }
            IrMutation::UpdateText { path, content } => match node_mut(ir, path)? {
                ViewNode::Text {
                    content: current, ..
                } => *current = content.clone(),
                other => {
                    return Err(format!(
                        "UpdateText expects text node at {:?}, got {other:?}",
                        path
                    ))
                }
            },
            IrMutation::UpdateStyle { path, style } => {
                *node_mut(ir, path)?.style_mut() = style.clone();
            }
        }
    }
    Ok(())
}

fn diff_node_lists(
    old: &[ViewNode],
    new: &[ViewNode],
    parent: &[usize],
    out: &mut Vec<IrMutation>,
) {
    let common = old.len().min(new.len());
    for i in 0..common {
        let mut path = parent.to_vec();
        path.push(i);
        diff_node(&old[i], &new[i], &path, out);
    }

    if old.len() > new.len() {
        for i in (new.len()..old.len()).rev() {
            let mut path = parent.to_vec();
            path.push(i);
            out.push(IrMutation::RemoveNode { path });
        }
    } else if new.len() > old.len() {
        for (i, node) in new.iter().enumerate().skip(old.len()) {
            out.push(IrMutation::InsertNode {
                parent_path: parent.to_vec(),
                index: i,
                node: node.clone(),
            });
        }
    }
}

fn diff_node(old: &ViewNode, new: &ViewNode, path: &[usize], out: &mut Vec<IrMutation>) {
    match (old, new) {
        (
            ViewNode::Text {
                content: oc,
                bind: ob,
                style: os,
            },
            ViewNode::Text {
                content: nc,
                bind: nb,
                style: ns,
            },
        ) => {
            if ob != nb {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
            } else if oc != nc {
                out.push(IrMutation::UpdateText {
                    path: path.to_vec(),
                    content: nc.clone(),
                });
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        (
            ViewNode::Stack {
                on_long_press: olp,
                axis: oa,
                spacing: og,
                align_items: oai,
                justify_content: ojc,
                style: os,
                children: och,
                ..
            },
            ViewNode::Stack {
                on_long_press: nlp,
                axis: na,
                spacing: ng,
                align_items: nai,
                justify_content: njc,
                style: ns,
                children: nch,
                ..
            },
        ) => {
            if oa != na || og != ng || oai != nai || ojc != njc || olp != nlp {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
            diff_node_lists(och, nch, path, out);
        }
        (
            ViewNode::Scroll {
                axis: oa,
                on_long_press: ol,
                style: os,
                children: och,
            },
            ViewNode::Scroll {
                axis: na,
                on_long_press: nl,
                style: ns,
                children: nch,
            },
        ) => {
            if oa != na || ol != nl {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
            diff_node_lists(och, nch, path, out);
        }
        (
            ViewNode::Button {
                on_long_press: olp,
                label: ol,
                label_parts: op,
                on_click: oo,
                style: os,
                ..
            },
            ViewNode::Button {
                on_long_press: nlp,
                label: nl,
                label_parts: np,
                on_click: no,
                style: ns,
                ..
            },
        ) => {
            if ol != nl || op != np || oo != no || olp != nlp {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        (
            ViewNode::Image {
                on_long_press: olp,
                src: osrc,
                src_bind: osrc_bind,
                alt: oalt,
                placeholder: oph,
                style: os,
                ..
            },
            ViewNode::Image {
                on_long_press: nlp,
                src: nsrc,
                src_bind: nsrc_bind,
                alt: nalt,
                placeholder: nph,
                style: ns,
                ..
            },
        ) => {
            if osrc != nsrc || osrc_bind != nsrc_bind || oalt != nalt || oph != nph || olp != nlp {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        (
            ViewNode::SlotRotate {
                phrases: op,
                interval_ms: oi,
                style: os,
            },
            ViewNode::SlotRotate {
                phrases: np,
                interval_ms: ni,
                style: ns,
            },
        ) => {
            if op != np || oi != ni {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        (
            ViewNode::Input {
                placeholder: op,
                bind: ob,
                multiline: om,
                secure: osq,
                on_change: oc,
                on_input: oi,
                style: os,
            },
            ViewNode::Input {
                placeholder: np,
                bind: nb,
                multiline: nm,
                secure: nsq,
                on_change: nc,
                on_input: ni,
                style: ns,
            },
        ) => {
            if op != np || ob != nb || om != nm || osq != nsq || oc != nc || oi != ni {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        (
            ViewNode::Picker {
                bind: ob,
                options: oo,
                on_change: oc,
                style: os,
            },
            ViewNode::Picker {
                bind: nb,
                options: no,
                on_change: nc,
                style: ns,
            },
        ) => {
            if ob != nb || oo != no || oc != nc {
                out.push(IrMutation::ReplaceNode {
                    path: path.to_vec(),
                    node: new.clone(),
                });
                return;
            }
            if os != ns {
                out.push(IrMutation::UpdateStyle {
                    path: path.to_vec(),
                    style: ns.clone(),
                });
            }
        }
        _ if old == new => {}
        _ => out.push(IrMutation::ReplaceNode {
            path: path.to_vec(),
            node: new.clone(),
        }),
    }
}

fn node_mut<'a>(ir: &'a mut ViewIr, path: &[usize]) -> Result<&'a mut ViewNode, String> {
    if path.is_empty() {
        return Err("node path must not be empty".to_string());
    }
    node_mut_in_list(ir.root.as_mut_slice(), path)
        .ok_or_else(|| format!("invalid node path {path:?} for current IR"))
}

fn node_mut_in_list<'a>(nodes: &'a mut [ViewNode], path: &[usize]) -> Option<&'a mut ViewNode> {
    let (idx, rest) = path.split_first()?;
    let node = nodes.get_mut(*idx)?;
    if rest.is_empty() {
        return Some(node);
    }
    let children = match node {
        ViewNode::Stack { children, .. }
        | ViewNode::Scroll { children, .. }
        | ViewNode::Dropzone { children, .. }
        | ViewNode::List { children, .. }
        | ViewNode::Link { children, .. }
        | ViewNode::ListItem { children, .. } => children,
        _ => return None,
    };
    node_mut_in_list(children.as_mut_slice(), rest)
}

fn children_mut<'a>(
    ir: &'a mut ViewIr,
    parent_path: &[usize],
) -> Result<&'a mut Vec<ViewNode>, String> {
    if parent_path.is_empty() {
        return Ok(&mut ir.root);
    }

    match node_mut(ir, parent_path)? {
        ViewNode::Stack { children, .. }
        | ViewNode::Scroll { children, .. }
        | ViewNode::Dropzone { children, .. }
        | ViewNode::List { children, .. }
        | ViewNode::Link { children, .. }
        | ViewNode::ListItem { children, .. } => Ok(children),
        other => Err(format!(
            "node at {:?} cannot contain children: {other:?}",
            parent_path
        )),
    }
}

#[cfg(test)]
mod long_press_tests {
    use super::*;

    #[test]
    fn failed_patch_keeps_the_original_document() {
        let original = ViewIr {
            version: 1,
            root: vec![],
        };
        let mut document = original.clone();
        let patch = [
            IrMutation::ReplaceRoot {
                root: vec![ViewNode::Button {
                    label: "Hold".into(),
                    label_parts: vec![],
                    on_click: None,
                    on_long_press: Some("hold".into()),
                    style: None,
                }],
            },
            IrMutation::RemoveNode { path: vec![99] },
        ];
        assert!(apply_mutations(&mut document, &patch).is_err());
        assert_eq!(document, original);
    }

    #[test]
    fn button_long_press_changes_replace_the_node() {
        let button = |handler: Option<&str>| ViewNode::Button {
            label: "Hold".into(),
            label_parts: vec![],
            on_click: None,
            on_long_press: handler.map(str::to_owned),
            style: None,
        };
        for (old, new) in [
            (None, Some("hold")),
            (Some("hold"), Some("other")),
            (Some("hold"), None),
        ] {
            let new = button(new);
            let mut changes = vec![];
            diff_node_lists(
                &[button(old)],
                std::slice::from_ref(&new),
                &[],
                &mut changes,
            );
            assert_eq!(
                changes,
                vec![IrMutation::ReplaceNode {
                    path: vec![0],
                    node: new
                }]
            );
        }
        let same = button(Some("hold"));
        let mut changes = vec![];
        diff_node_lists(
            std::slice::from_ref(&same),
            std::slice::from_ref(&same),
            &[],
            &mut changes,
        );
        assert!(changes.is_empty());
    }
}
