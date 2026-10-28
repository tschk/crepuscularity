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
        "context": context,
    })
    .to_string();
    let mut child = Command::new(bin)
        .args(["native", "ir", "--stdin-json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("crepus stdin unavailable")?
        .write_all(payload.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!(
            "crepus native ir failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
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
            callback(&event, self)?;
            self.handlers.insert(event.handler.clone(), callback);
        }
        self.render_ir()
    }
}

fn render_node(node: &ViewNode) -> String {
    match node {
        ViewNode::Text { content } => escape_html(content),
        ViewNode::Stack { axis, children } | ViewNode::Scroll { axis, children } => {
            format!(
                "<div data-crepus-kind=\"stack\" data-axis=\"{}\">{}</div>",
                escape_html(axis),
                children.iter().map(render_node).collect::<String>()
            )
        }
        ViewNode::Button { label, on_click } => {
            let attr = on_click
                .as_deref()
                .map(|handler| format!(" data-onclick=\"{}\"", escape_html(handler)))
                .unwrap_or_default();
            format!("<button{}>{}</button>", attr, escape_html(label))
        }
        ViewNode::Other => String::new(),
    }
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
