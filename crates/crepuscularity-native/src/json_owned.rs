use crate::{ViewIr, ViewNode};

enum Task {
    Node(Box<ViewNode>),
    Nodes(Vec<ViewNode>),
    Raw(String),
    Literal(&'static str),
}

/// Serialize an owned IR without recursive serialization of its child nodes.
/// Keeps Serde's field encoding and order, including omitted optional fields.
pub fn to_json_owned(ir: ViewIr) -> Result<String, serde_json::Error> {
    let mut json = format!("{{\"version\":{},\"root\":", ir.version);
    let mut tasks = vec![Task::Literal("}"), Task::Nodes(ir.root)];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Raw(raw) => json.push_str(&raw),
            Task::Literal(raw) => json.push_str(raw),
            Task::Nodes(nodes) => {
                json.push('[');
                tasks.push(Task::Literal("]"));
                for (index, node) in nodes.into_iter().enumerate().rev() {
                    tasks.push(Task::Node(Box::new(node)));
                    if index != 0 {
                        tasks.push(Task::Literal(","));
                    }
                }
            }
            Task::Node(node) => {
                let mut node = *node;
                let children = take_children(&mut node);
                let shell = serde_json::to_string(&node)?;
                let mut fields = Vec::new();
                let mut cursor = 0;
                for (key, children) in children {
                    // Structural field names are emitted by Serde; user quotes
                    // are escaped. Scan the original shell in field order so
                    // empty tab children remain distinct.
                    let field = format!("\"{key}\":[]");
                    let start = cursor
                        + shell[cursor..]
                            .find(&field)
                            .expect("detached child field must be serialized as an empty array");
                    let array_start = start + field.len() - 2;
                    fields.push(Task::Raw(shell[cursor..array_start].to_owned()));
                    fields.push(Task::Nodes(children));
                    cursor = array_start + 2;
                }
                fields.push(Task::Raw(shell[cursor..].to_owned()));
                tasks.extend(fields.into_iter().rev());
            }
        }
    }
    Ok(json)
}

fn take_children(node: &mut ViewNode) -> Vec<(&'static str, Vec<ViewNode>)> {
    match node {
        ViewNode::Stack { children, .. }
        | ViewNode::Scroll { children, .. }
        | ViewNode::Dropzone { children, .. }
        | ViewNode::Link { children, .. }
        | ViewNode::List { children, .. }
        | ViewNode::ListItem { children, .. } => vec![("children", std::mem::take(children))],
        ViewNode::ForEach { item_body, .. } => vec![("itemBody", std::mem::take(item_body))],
        ViewNode::If {
            then_children,
            else_children,
            ..
        } => {
            let mut children = vec![("thenChildren", std::mem::take(then_children))];
            if let Some(nodes) = else_children {
                children.push(("elseChildren", std::mem::take(nodes)));
            }
            children
        }
        ViewNode::Tabs { tabs, .. } => tabs
            .iter_mut()
            .map(|tab| ("children", std::mem::take(&mut tab.children)))
            .collect(),
        // Keep this exhaustive: adding a container must explicitly decide how
        // its children are detached before Serde runs.
        ViewNode::Text { .. }
        | ViewNode::Button { .. }
        | ViewNode::Toggle { .. }
        | ViewNode::Checkbox { .. }
        | ViewNode::Slider { .. }
        | ViewNode::Progress { .. }
        | ViewNode::Meter { .. }
        | ViewNode::Badge { .. }
        | ViewNode::Divider { .. }
        | ViewNode::Spacer { .. }
        | ViewNode::FilePicker { .. }
        | ViewNode::Image { .. }
        | ViewNode::WebView { .. }
        | ViewNode::SlotRotate { .. }
        | ViewNode::Input { .. }
        | ViewNode::Picker { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deep_ir_serializes_on_a_default_test_thread() {
        let source = format!("{}leaf{}", "<div>".repeat(250), "</div>".repeat(250));
        let ir = crate::render_template_to_ir(
            &source,
            &crepuscularity_core::context::TemplateContext::new(),
        )
        .unwrap();
        let json = to_json_owned(ir).unwrap();
        assert_eq!(json.matches("\"kind\":\"stack\"").count(), 250);
        assert!(json.contains("\"content\":\"leaf\""));
    }

    #[test]
    fn owned_encoding_matches_serde_for_every_recursive_container() {
        let leaf = json!({"kind":"text", "content":"\"children\":[] 雪\n"});
        let children = json!([leaf, {"kind":"text", "content":"Second child"}]);
        let cases = [
            json!({"kind":"stack", "axis":"column", "children":children}),
            json!({"kind":"scroll", "axis":"column", "children":children}),
            json!({"kind":"dropzone", "label":"Drop", "children":children}),
            json!({"kind":"link", "href":"https://example.com", "children":children}),
            json!({"kind":"list", "ordered":true, "children":children}),
            json!({"kind":"listItem", "children":children}),
            json!({"kind":"forEach", "bind":"items", "itemName":"item", "itemBody":children}),
            json!({"kind":"if", "condition":"active", "thenChildren":children}),
            json!({"kind":"if", "condition":"active", "thenChildren":[], "elseChildren":children}),
            json!({"kind":"if", "condition":"active", "thenChildren":children, "elseChildren":[]}),
            json!({"kind":"tabs", "bind":"tab", "tabs":[
                {"value":"a", "label":"Empty", "children":[]},
                {"value":"b", "label":"Full", "children":children},
                {"value":"c", "label":"Empty again", "children":[]}
            ]}),
        ];
        for case in cases {
            let node = serde_json::from_value(case).unwrap();
            let ir = ViewIr {
                version: crate::IR_VERSION,
                root: vec![
                    node,
                    ViewNode::Text {
                        content: "Second root".to_owned(),
                        bind: None,
                        style: None,
                    },
                ],
            };
            let expected = serde_json::to_string(&ir).unwrap();
            assert_eq!(to_json_owned(ir).unwrap(), expected);
        }
        let ir = ViewIr {
            version: crate::IR_VERSION,
            root: vec![],
        };
        assert_eq!(
            to_json_owned(ir.clone()).unwrap(),
            serde_json::to_string(&ir).unwrap()
        );
    }
}
