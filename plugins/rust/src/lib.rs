use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct ViewIr {
    pub version: u32,
    pub root: Vec<ViewNode>,
}

pub struct Event {
    pub handler: String,
    pub payload: Option<Value>,
}

pub type EventHandler =
    Box<dyn FnMut(&Event, &mut ViewSession) -> Result<(), Box<dyn std::error::Error>>>;

pub struct ViewSession {
    path: String,
    context: HashMap<String, Value>,
    handlers: HashMap<String, EventHandler>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ViewNode {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "stack")]
    Stack {
        axis: String,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "scroll")]
    Scroll {
        axis: String,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "button")]
    Button {
        label: String,
        #[serde(rename = "onClick")]
        on_click: Option<String>,
    },
    #[serde(other)]
    Other,
}

pub fn render_ir(path: &str) -> Result<ViewIr, Box<dyn std::error::Error>> {
    render_ir_with_context(path, &HashMap::new())
}

pub fn render_ir_with_context(
    path: &str,
    context: &HashMap<String, Value>,
) -> Result<ViewIr, Box<dyn std::error::Error>> {
    let bin = std::env::var("CREPUS_BIN").unwrap_or_else(|_| {
        let local =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/crepus");
        if local.exists() {
            local.display().to_string()
        } else {
            "crepus".to_string()
        }
    });
    let template = std::fs::read_to_string(path)?;
    let payload = serde_json::json!({
        "template": template,
        "filename": path,
        "baseDir": std::path::Path::new(path).parent(),
        "context": context,
    })
    .to_string();
    let mut child = Command::new(bin)
        .args(["native", "ir", "--stdin-json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("crepus stdin unavailable")?;
    // Drain output while writing so early diagnostics cannot block a large request.
    let writer = std::thread::spawn(move || stdin.write_all(payload.as_bytes()));
    let out = child.wait_with_output();
    let written = writer.join();
    let out = out?;
    if !out.status.success() {
        return Err(format!(
            "crepus native ir failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    written.map_err(|_| "crepus stdin writer panicked")??;
    Ok(serde_json::from_slice(&out.stdout)?)
}

pub fn render_html(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(render_ir(path)?.root.iter().map(render_node).collect())
}

impl ViewSession {
    pub fn new(path: impl Into<String>, context: HashMap<String, Value>) -> Self {
        Self {
            path: path.into(),
            context,
            handlers: HashMap::new(),
        }
    }

    pub fn on<F>(&mut self, handler: impl Into<String>, callback: F) -> &mut Self
    where
        F: FnMut(&Event, &mut ViewSession) -> Result<(), Box<dyn std::error::Error>> + 'static,
    {
        self.handlers.insert(handler.into(), Box::new(callback));
        self
    }

    pub fn render_ir(&self) -> Result<ViewIr, Box<dyn std::error::Error>> {
        render_ir_with_context(&self.path, &self.context)
    }

    pub fn render_html(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.render_ir()?.root.iter().map(render_node).collect())
    }

    pub fn dispatch(&mut self, event: Event) -> Result<ViewIr, Box<dyn std::error::Error>> {
        if let Some(rest) = event.handler.strip_prefix("bind:") {
            if let Some((key, value)) = rest.split_once(':') {
                self.context
                    .insert(key.to_string(), Value::String(value.to_string()));
            }
        }
        if let Some(mut callback) = self.handlers.remove(&event.handler) {
            let result = callback(&event, self);
            self.handlers.insert(event.handler.clone(), callback);
            result?;
        }
        self.render_ir()
    }
}

fn render_node(node: &ViewNode) -> String {
    match node {
        ViewNode::Text { content } => escape_html(content),
        ViewNode::Stack { axis, children } | ViewNode::Scroll { axis, children } => {
            let kind = if matches!(node, ViewNode::Scroll { .. }) {
                "scroll"
            } else {
                "stack"
            };
            format!(
                "<div data-crepus-kind=\"{kind}\" data-axis=\"{}\">{}</div>",
                escape_attr(axis),
                children.iter().map(render_node).collect::<String>()
            )
        }
        ViewNode::Button { label, on_click } => {
            let attr = on_click
                .as_deref()
                .map(|handler| format!(" data-onclick=\"{}\"", escape_attr(handler)))
                .unwrap_or_default();
            format!("<button{}>{}</button>", attr, escape_html(label))
        }
        ViewNode::Other => String::new(),
    }
}

fn escape_attr(value: &str) -> String {
    escape_html(value).replace('\r', "&#13;")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    #[test]
    fn stacks_and_scrolls_keep_distinct_html_markers() {
        let stack = crate::ViewNode::Stack {
            axis: "row".into(),
            children: vec![crate::ViewNode::Scroll {
                axis: "column".into(),
                children: vec![crate::ViewNode::Text {
                    content: "Child <&雪".into(),
                }],
            }],
        };
        assert_eq!(crate::render_node(&stack), "<div data-crepus-kind=\"stack\" data-axis=\"row\"><div data-crepus-kind=\"scroll\" data-axis=\"column\">Child &lt;&amp;雪</div></div>");
        let scroll = crate::ViewNode::Scroll {
            axis: "row".into(),
            children: vec![],
        };
        assert_eq!(
            crate::render_node(&scroll),
            "<div data-crepus-kind=\"scroll\" data-axis=\"row\"></div>"
        );
    }

    #[test]
    fn attributes_preserve_carriage_returns_after_html_escaping() {
        let value = "雪\rline\r\n&#13;\"&<>";
        let escaped = "雪&#13;line&#13;\n&amp;#13;&quot;&amp;&lt;&gt;";
        let button = crate::ViewNode::Button {
            label: value.into(),
            on_click: Some(value.into()),
        };
        assert_eq!(
            crate::render_node(&button),
            format!(
                "<button data-onclick=\"{escaped}\">{}</button>",
                crate::escape_html(value)
            )
        );
        for node in [
            crate::ViewNode::Stack {
                axis: value.into(),
                children: vec![],
            },
            crate::ViewNode::Scroll {
                axis: value.into(),
                children: vec![],
            },
        ] {
            assert!(crate::render_node(&node).contains(&format!(" data-axis=\"{escaped}\"")));
        }
        assert!(crate::escape_html(value).contains('\r'));
    }

    #[test]
    fn callback_errors_retain_registration_for_later_dispatches() {
        use std::cell::Cell;
        use std::rc::Rc;

        let calls = Rc::new(Cell::new(0));
        let observed = Rc::clone(&calls);
        let mut session = crate::ViewSession::new("unused.crepus", Default::default());
        session.on("bind:count:2", move |_, session| {
            observed.set(observed.get() + 1);
            assert_eq!(
                session.context["count"],
                serde_json::Value::String("2".into())
            );
            Err("handler marker".into())
        });
        for expected in [1, 2] {
            let error = session
                .dispatch(crate::Event {
                    handler: "bind:count:2".into(),
                    payload: None,
                })
                .unwrap_err();
            assert_eq!(error.to_string(), "handler marker");
            assert_eq!(calls.get(), expected);
        }
    }

    #[test]
    fn context_render_resolves_relative_includes() {
        let ir = crate::render_ir("../fixtures/relative-include/main.crepus").unwrap();
        assert!(serde_json::to_string(&ir.root)
            .unwrap()
            .contains("Relative include content"));
    }

    #[test]
    fn context_render_preserves_frontend_filename() {
        let ir = crate::render_ir("../fixtures/frontend.svelte").unwrap();
        let text = serde_json::to_string(&ir.root).unwrap();
        assert!(text.contains("Frontend content"));
        assert!(!text.contains("ignored"));
    }

    #[test]
    fn renders_html() {
        let html = crate::render_html("../fixtures/hello.crepus").expect("render html");
        assert!(html.contains("data-crepus-kind=\"stack\""));
    }

    #[test]
    fn session_dispatch_rerenders() {
        let mut context = std::collections::HashMap::new();
        context.insert(
            "count".to_string(),
            serde_json::Value::String("1".to_string()),
        );
        let mut session = crate::ViewSession::new("../fixtures/interactive.crepus", context);
        assert!(session
            .render_html()
            .expect("failed to render HTML")
            .contains("Count 1"));
        let ir = session
            .dispatch(crate::Event {
                handler: "bind:count:2".to_string(),
                payload: None,
            })
            .expect("dispatch event failed");
        let rendered = serde_json::to_string(&ir.root).expect("failed to serialize IR root");
        assert!(rendered.contains("Count 2"));
    }
}
