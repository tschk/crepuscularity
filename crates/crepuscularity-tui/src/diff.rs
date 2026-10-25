use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use crepuscularity_core::ast::Node;

use crepuscularity_core::{TemplateContext, TemplateValue};

fn hash_value<H: Hasher>(value: &TemplateValue, state: &mut H) {
    std::mem::discriminant(value).hash(state);
    match value {
        TemplateValue::Str(s) => s.hash(state),
        TemplateValue::Int(n) => n.hash(state),
        TemplateValue::Float(f) => f.to_bits().hash(state),
        TemplateValue::Bool(b) => b.hash(state),
        TemplateValue::Null => {}
        TemplateValue::List(items) => {
            items.len().hash(state);
            for item in items {
                hash_context(item, state);
            }
        }
        TemplateValue::Scope(ctx) => hash_context(ctx, state),
    }
}

fn hash_context<H: Hasher>(ctx: &TemplateContext, state: &mut H) {
    let mut keys: Vec<&String> = ctx.vars.keys().collect();
    keys.sort();
    keys.len().hash(state);
    for key in keys {
        key.hash(state);
        hash_value(&ctx.vars[key], state);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderSnapshot {
    fingerprints: HashMap<Arc<str>, u64>,
    base_dir: Option<PathBuf>,
    virtual_files: Arc<HashMap<String, String>>,
    slot: Option<(Vec<Node>, Box<RenderSnapshot>)>,
}

impl RenderSnapshot {
    pub fn from_context(ctx: &TemplateContext) -> Self {
        let mut fingerprints = HashMap::with_capacity(ctx.vars.len());
        for (key, value) in &ctx.vars {
            let mut hasher = rustc_hash::FxHasher::default();
            hash_value(value, &mut hasher);
            fingerprints.insert(Arc::from(key.as_str()), hasher.finish());
        }
        Self {
            fingerprints,
            base_dir: ctx.base_dir.clone(),
            virtual_files: Arc::clone(&ctx.virtual_files),
            slot: Self::snapshot_slot(ctx),
        }
    }

    fn snapshot_slot(ctx: &TemplateContext) -> Option<(Vec<Node>, Box<Self>)> {
        ctx.slot
            .as_ref()
            .map(|(nodes, parent)| (nodes.clone(), Box::new(Self::from_context(parent))))
    }

    fn has_changed(&self, ctx: &TemplateContext) -> bool {
        if self.base_dir != ctx.base_dir
            || (!Arc::ptr_eq(&self.virtual_files, &ctx.virtual_files)
                && self.virtual_files != ctx.virtual_files)
        {
            return true;
        }
        let slot_changed = match (&self.slot, &ctx.slot) {
            (None, None) => false,
            (Some((previous_nodes, previous_parent)), Some((nodes, parent))) => {
                previous_nodes != nodes || previous_parent.has_changed(parent)
            }
            _ => true,
        };
        if slot_changed || self.fingerprints.len() != ctx.vars.len() {
            return true;
        }
        ctx.vars.iter().any(|(key, value)| {
            let mut hasher = rustc_hash::FxHasher::default();
            hash_value(value, &mut hasher);
            self.fingerprints.get(key.as_str()) != Some(&hasher.finish())
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiffTracker {
    last: Option<RenderSnapshot>,
}

impl DiffTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_changed(&self, ctx: &TemplateContext) -> bool {
        match &self.last {
            None => true,
            Some(snapshot) => snapshot.has_changed(ctx),
        }
    }

    pub fn update(&mut self, ctx: &TemplateContext) {
        if let Some(snapshot) = &mut self.last {
            snapshot.base_dir.clone_from(&ctx.base_dir);
            snapshot.virtual_files = Arc::clone(&ctx.virtual_files);
            snapshot.slot = RenderSnapshot::snapshot_slot(ctx);
            snapshot
                .fingerprints
                .retain(|k, _| ctx.vars.contains_key(k.as_ref()));
            for (key, value) in &ctx.vars {
                let mut hasher = rustc_hash::FxHasher::default();
                hash_value(value, &mut hasher);
                let fp = hasher.finish();
                if let Some(v) = snapshot.fingerprints.get_mut(key.as_str()) {
                    *v = fp;
                } else {
                    snapshot.fingerprints.insert(Arc::from(key.as_str()), fp);
                }
            }
        } else {
            self.last = Some(RenderSnapshot::from_context(ctx));
        }
    }

    /// Changed variable names; resource-only changes are reported by `has_changed`.
    pub fn changed_keys(&self, ctx: &TemplateContext) -> Vec<String> {
        match &self.last {
            None => ctx.vars.keys().cloned().collect(),
            Some(snapshot) => {
                let mut changed = Vec::new();
                for (key, value) in &ctx.vars {
                    let mut hasher = rustc_hash::FxHasher::default();
                    hash_value(value, &mut hasher);
                    let fingerprint = hasher.finish();
                    match snapshot.fingerprints.get(key.as_str()) {
                        Some(prev) if *prev == fingerprint => {}
                        _ => changed.push(key.clone()),
                    }
                }
                for key in snapshot.fingerprints.keys() {
                    if !ctx.vars.contains_key(key.as_ref()) {
                        changed.push(key.to_string());
                    }
                }
                changed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_tracker_detects_new_variables() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        tracker.update(&ctx);

        ctx.set("b", "2");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_detects_changed_variable_values() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        tracker.update(&ctx);

        ctx.set("a", "2");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_detects_removed_variables() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        tracker.update(&ctx);

        ctx.vars.remove("b");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_returns_false_when_nothing_changed() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        tracker.update(&ctx);

        assert!(!tracker.has_changed(&ctx));
    }

    #[test]
    fn changed_keys_returns_correct_list() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        ctx.set("c", "3");
        tracker.update(&ctx);

        ctx.set("a", "changed");
        ctx.vars.remove("b");
        ctx.set("d", "new");

        let mut changed = tracker.changed_keys(&ctx);
        changed.sort();
        assert_eq!(changed, vec!["a", "b", "d"]);
    }
}
