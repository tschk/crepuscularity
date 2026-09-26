use std::collections::HashMap;
use std::hash::{Hash, Hasher};

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
    fingerprints: HashMap<String, u64>,
}

impl RenderSnapshot {
    pub fn from_context(ctx: &TemplateContext) -> Self {
        let mut fingerprints = HashMap::with_capacity(ctx.vars.len());
        for (key, value) in &ctx.vars {
            let mut hasher = rustc_hash::FxHasher::default();
            hash_value(value, &mut hasher);
            fingerprints.insert(key.clone(), hasher.finish());
        }
        Self { fingerprints }
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
            Some(snapshot) => {
                if snapshot.fingerprints.len() != ctx.vars.len() {
                    return true;
                }
                for (key, value) in &ctx.vars {
                    let mut hasher = rustc_hash::FxHasher::default();
                    hash_value(value, &mut hasher);
                    let fingerprint = hasher.finish();
                    match snapshot.fingerprints.get(key) {
                        Some(prev) if *prev == fingerprint => {}
                        _ => return true,
                    }
                }
                false
            }
        }
    }

    pub fn update(&mut self, ctx: &TemplateContext) {
        if let Some(snapshot) = &mut self.last {
            snapshot
                .fingerprints
                .retain(|k, _| ctx.vars.contains_key(k));
            for (key, value) in &ctx.vars {
                let mut hasher = rustc_hash::FxHasher::default();
                hash_value(value, &mut hasher);
                let fp = hasher.finish();
                if let Some(v) = snapshot.fingerprints.get_mut(key) {
                    *v = fp;
                } else {
                    snapshot.fingerprints.insert(key.clone(), fp);
                }
            }
        } else {
            self.last = Some(RenderSnapshot::from_context(ctx));
        }
    }

    pub fn changed_keys(&self, ctx: &TemplateContext) -> Vec<String> {
        match &self.last {
            None => ctx.vars.keys().cloned().collect(),
            Some(snapshot) => {
                let mut changed = Vec::new();
                for (key, value) in &ctx.vars {
                    let mut hasher = rustc_hash::FxHasher::default();
                    hash_value(value, &mut hasher);
                    let fingerprint = hasher.finish();
                    match snapshot.fingerprints.get(key) {
                        Some(prev) if *prev == fingerprint => {}
                        _ => changed.push(key.clone()),
                    }
                }
                for key in snapshot.fingerprints.keys() {
                    if !ctx.vars.contains_key(key) {
                        changed.push(key.clone());
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
