//! Named `@click` handlers for runtime-rendered (and hot-reloaded) templates.
//!
//! Templates name a handler; the host registers Rust callbacks under those names:
//!
//! ```ignore
//! let handlers = EventHandlers::new().on_click("increment", move |_, _, cx| { /* … */ });
//! HotReloadState::new(path, context, cx).with_event_handlers(handlers)
//! ```
//!
//! - `@click=increment` calls `increment` with no arguments.
//! - `@click={select(item.id, "x")}` evaluates each argument in the element's scope.
//! - A name that is not registered but is a string variable (for example a component prop
//!   `action="increment"`) resolves through that variable, so components can take handlers.
//!
//! Lookups happen on every render, so handlers keep working after a template reload.
//! Without an installed registry (plain `render_nodes`, `crepus preview`) clicks are inert;
//! with one, an unknown name renders an in-window error instead of being dropped.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crepuscularity_core::context::{TemplateContext, TemplateValue};

pub type ClickHandler = Rc<dyn Fn(&[TemplateValue], &mut gpui::Window, &mut gpui::App)>;

/// Host callbacks addressed by template handler names.
#[derive(Clone, Default)]
pub struct EventHandlers {
    click: HashMap<String, ClickHandler>,
}

impl EventHandlers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_click(
        mut self,
        name: impl Into<String>,
        handler: impl Fn(&[TemplateValue], &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.click.insert(name.into(), Rc::new(handler));
        self
    }
}

thread_local! {
    static CURRENT: RefCell<Option<EventHandlers>> = const { RefCell::new(None) };
}

/// Install `handlers` for runtime renders inside `render` (GPUI renders on one thread).
pub fn with_event_handlers<R>(handlers: &EventHandlers, render: impl FnOnce() -> R) -> R {
    struct Restore(Option<EventHandlers>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|current| *current.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CURRENT.with(|current| current.borrow_mut().replace(handlers.clone())));
    render()
}

/// `Ok(None)` when no registry is installed.
pub(crate) fn resolve_click(
    handler: &str,
    ctx: &TemplateContext,
) -> Result<Option<(ClickHandler, Vec<TemplateValue>)>, String> {
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(handlers) = current.as_ref() else {
            return Ok(None);
        };
        let (name, args) = split_call(handler)?;
        let (callback, args) = match handlers.click.get(name) {
            Some(callback) => (callback, args),
            None => {
                // One level of indirection: a string prop naming the real handler.
                let target = match ctx.vars.get(name) {
                    Some(TemplateValue::Str(target)) if args.is_empty() => target.as_str(),
                    _ => return Err(format!("no GPUI @click handler registered for `{name}`")),
                };
                let (target_name, target_args) = split_call(target)?;
                let callback = handlers.click.get(target_name).ok_or_else(|| {
                    format!("no GPUI @click handler registered for `{target_name}` (via `{name}`)")
                })?;
                (callback, target_args)
            }
        };
        let args = args
            .iter()
            .map(|arg| {
                crepuscularity_core::eval::eval_expr(arg, ctx)
                    .map_err(|error| format!("@click argument `{arg}`: {error}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Some((Rc::clone(callback), args)))
    })
}

/// `name` or `name(arg, …)`, with optional surrounding braces.
fn split_call(handler: &str) -> Result<(&str, Vec<&str>), String> {
    let text = handler.trim();
    let text = text
        .strip_prefix('{')
        .and_then(|t| t.strip_suffix('}'))
        .unwrap_or(text)
        .trim();
    let (name, args) = match text.split_once('(') {
        Some((name, rest)) => {
            let inner = rest
                .trim_end()
                .strip_suffix(')')
                .ok_or_else(|| format!("@click `{handler}` is missing `)`"))?;
            (name.trim(), split_args(inner))
        }
        None => (text, Vec::new()),
    };
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(format!(
            "@click `{handler}` must be a handler name or `name(args)` call"
        ));
    }
    Ok((name, args))
}

fn split_args(inner: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let (mut depth, mut quote, mut start) = (0usize, None, 0);
    for (i, c) in inner.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() || !args.is_empty() {
        args.push(last);
    }
    args
}
