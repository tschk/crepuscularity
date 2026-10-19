//! Binding analysis and fingerprinting for the `.crepus` driver pipeline.
//!
//! # Binding analysis
//!
//! [`analyze_template`] walks the AST and classifies each top-level node as
//! [`Region::Static`] (no runtime expressions anywhere in the subtree) or
//! [`Region::Dynamic`] (contains at least one binding site: text interpolation,
//! dynamic attribute, conditional class, `if`, `for`, `match`, or `include`).
//!
//! # Fingerprinting
//!
//! [`Fingerprint`] gives every `(source, section?, stage)` triple a stable,
//! content-addressed identity (SHA-256 of the source bytes). The driver cache
//! ([`crate::cache::DriverCache`]) uses this as the cache key so that a second
//! save with no logical change does not rewrite generated output and therefore
//! does not bump `rustc`'s input mtime.

use sha2::{Digest, Sha256};

use crate::ast::*;

// ── Region ───────────────────────────────────────────────────────────────────

/// Whether a node subtree contains any runtime-evaluated binding sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// No runtime expressions anywhere in this subtree — fully static HTML.
    Static,
    /// Contains at least one binding site: expression, control flow, or include.
    Dynamic,
}

/// Per-node classifications borrowing one AST ownership scope.
///
/// Keys use node identity rather than structural equality. The borrowed input
/// keeps those addresses alive; nodes outside that input return `None`.
pub struct RegionMap<'a> {
    regions: std::collections::HashMap<*const Node, Region>,
    _nodes: std::marker::PhantomData<&'a [Node]>,
}

/// Classify all nodes in one borrowed AST ownership scope.
pub fn classify_regions(nodes: &[Node]) -> RegionMap<'_> {
    RegionMap::new(nodes)
}

impl<'a> RegionMap<'a> {
    /// Classify every node once, including authored include-slot children.
    pub fn new(nodes: &'a [Node]) -> Self {
        let mut regions = std::collections::HashMap::new();
        for node in nodes {
            classify_node_with::<true>(node, &mut |node, region| {
                regions.insert(node as *const Node, region.clone());
            });
        }
        Self {
            regions,
            _nodes: std::marker::PhantomData,
        }
    }

    /// Return a classification only for the exact indexed node.
    pub fn get(&self, node: &Node) -> Option<&Region> {
        self.regions.get(&(node as *const Node))
    }
}

// ── BindingMap ───────────────────────────────────────────────────────────────

/// A flat summary of the static vs dynamic regions in a template.
#[derive(Debug, Clone)]
pub struct BindingMap {
    /// Indices of top-level nodes that are entirely static.
    pub static_indices: Vec<usize>,
    /// Indices of top-level nodes that have at least one binding site.
    pub dynamic_indices: Vec<usize>,
    /// Total number of expression binding sites across the whole template.
    pub expr_count: usize,
}

/// Classify all top-level nodes in a template and return a [`BindingMap`].
pub fn analyze_template(nodes: &[Node]) -> BindingMap {
    let mut static_indices = Vec::new();
    let mut dynamic_indices = Vec::new();
    let mut expr_count = 0;

    for (i, node) in nodes.iter().enumerate() {
        let (region, count) = classify_node_inner(node);
        expr_count += count;
        if region == Region::Dynamic {
            dynamic_indices.push(i);
        } else {
            static_indices.push(i);
        }
    }

    BindingMap {
        static_indices,
        dynamic_indices,
        expr_count,
    }
}

/// Classify a single node as [`Region::Static`] or [`Region::Dynamic`].
pub fn classify_node(node: &Node) -> Region {
    classify_node_inner(node).0
}

fn classify_node_inner(node: &Node) -> (Region, usize) {
    classify_node_with::<false>(node, &mut |_, _| {})
}

fn classify_node_with<const INDEX: bool>(
    node: &Node,
    visit: &mut impl FnMut(&Node, &Region),
) -> (Region, usize) {
    let result = match node {
        Node::Text(parts) => {
            let expr_count = parts
                .iter()
                .filter(|p| matches!(p, TextPart::Expr(_)))
                .count();
            if expr_count > 0 {
                (Region::Dynamic, expr_count)
            } else {
                (Region::Static, 0)
            }
        }

        Node::Element(el) => classify_element::<INDEX>(el, visit),

        // Control flow and dynamic node types are always dynamic.
        Node::If(block) => {
            let mut count = 1; // the condition itself
            for n in &block.then_children {
                count += classify_node_with::<INDEX>(n, visit).1;
            }
            if let Some(els) = &block.else_children {
                for n in els {
                    count += classify_node_with::<INDEX>(n, visit).1;
                }
            }
            (Region::Dynamic, count)
        }

        Node::For(block) => {
            let mut count = 1; // the iterator expression
            for n in &block.body {
                count += classify_node_with::<INDEX>(n, visit).1;
            }
            (Region::Dynamic, count)
        }

        Node::Match(block) => {
            let mut count = 1; // the match expression
            for arm in &block.arms {
                for n in &arm.body {
                    count += classify_node_with::<INDEX>(n, visit).1;
                }
            }
            (Region::Dynamic, count)
        }

        Node::LetDecl(_) => (Region::Dynamic, 1),
        Node::Include(inc) => {
            if INDEX {
                for child in &inc.slot {
                    classify_node_with::<INDEX>(child, visit);
                }
            }
            (Region::Dynamic, inc.props.len().max(1))
        }
        Node::Embed(embed) => (Region::Dynamic, embed.props.len().max(1)),
        Node::RawText(_) | Node::RawHtml(_) => (Region::Dynamic, 1),
    };
    visit(node, &result.0);
    result
}

fn classify_element<const INDEX: bool>(
    el: &Element,
    visit: &mut impl FnMut(&Node, &Region),
) -> (Region, usize) {
    let mut expr_count = 0;
    let mut is_dynamic = false;

    if el.tag == "slot-rotate" {
        if INDEX {
            for child in &el.children {
                classify_node_with::<INDEX>(child, visit);
            }
        }
        return (Region::Dynamic, 1);
    }

    // Dynamic attribute bindings.
    expr_count += el.bindings.len();
    if !el.bindings.is_empty() {
        is_dynamic = true;
    }

    // Conditional classes.
    expr_count += el.conditional_classes.len();
    if !el.conditional_classes.is_empty() {
        is_dynamic = true;
    }

    // Class tokens that use `{…}` interpolation are dynamic.
    if el.classes.iter().any(|c| c.contains('{')) {
        is_dynamic = true;
    }

    // GPUI / web animations participate in runtime binding.
    if !el.animations.is_empty() {
        is_dynamic = true;
        expr_count += el.animations.len();
    }

    // Event handlers make the element dynamic (client-side binding site).
    if !el.event_handlers.is_empty() {
        is_dynamic = true;
        expr_count += el.event_handlers.len();
    }

    // Recurse into children.
    for child in &el.children {
        let (child_region, child_count) = classify_node_with::<INDEX>(child, visit);
        expr_count += child_count;
        if child_region == Region::Dynamic {
            is_dynamic = true;
        }
    }

    if is_dynamic {
        (Region::Dynamic, expr_count)
    } else {
        (Region::Static, 0)
    }
}

#[cfg(test)]
mod region_map_tests {
    use super::*;

    fn check(nodes: &[Node], regions: &RegionMap<'_>) -> usize {
        let mut count = 0;
        for node in nodes {
            assert_eq!(regions.get(node), Some(&classify_node(node)));
            count += 1;
            count += match node {
                Node::Element(el) => check(&el.children, regions),
                Node::If(block) => {
                    check(&block.then_children, regions)
                        + check(block.else_children.as_deref().unwrap_or_default(), regions)
                }
                Node::For(block) => check(&block.body, regions),
                Node::Match(block) => block.arms.iter().map(|arm| check(&arm.body, regions)).sum(),
                Node::Include(include) => check(&include.slot, regions),
                _ => 0,
            };
        }
        count
    }

    #[test]
    fn indexed_regions_match_existing_rules_and_include_caller_slots() {
        let nodes = crate::parser::parse_template(
            "div\n  if {ok}\n    \"{name}\"\n  else\n    \"static\"\n  for item in {items}\n    span\n      \"{item}\"\n  match {kind}\n    _ =>\n      \"fallback\"\n  include child.crepus value={value}\n    div\n      \"{slot_only}\"\n  slot-rotate\n    \"first\"\n    \"second\"",
        ).unwrap();
        let before = analyze_template(&nodes);
        let regions = classify_regions(&nodes);
        assert_eq!(check(&nodes, &regions), regions.regions.len());
        let after = analyze_template(&nodes);
        assert_eq!(before.expr_count, after.expr_count);
        assert_eq!(before.static_indices, after.static_indices);
        assert_eq!(before.dynamic_indices, after.dynamic_indices);
        let foreign = Node::RawText("outside".into());
        assert_eq!(regions.get(&foreign), None);
    }

    #[test]
    fn include_slot_expressions_do_not_change_legacy_expression_counts() {
        let nodes = vec![Node::Include(IncludeNode {
            path: "child.crepus".into(),
            props: vec![],
            slot: vec![Node::RawText("a".into()), Node::RawHtml("b".into())],
        })];
        let regions = classify_regions(&nodes);
        assert_eq!(check(&nodes, &regions), 3);
        assert_eq!(analyze_template(&nodes).expr_count, 1);
    }

    #[test]
    fn indexing_visits_nested_nodes_exactly_once() {
        for depth in [32, 64] {
            let source = format!(
                "{}{{value}}{}",
                "<div>".repeat(depth),
                "</div>".repeat(depth)
            );
            let nodes = crate::parser::parse_template(&source).unwrap();
            let mut visits = std::collections::HashMap::new();
            for node in &nodes {
                classify_node_with::<true>(node, &mut |node, _| {
                    *visits.entry(node as *const Node).or_insert(0) += 1;
                });
            }
            assert_eq!(visits.len(), depth + 1);
            assert!(visits.values().all(|count| *count == 1));
            assert_eq!(classify_regions(&nodes).regions.len(), depth + 1);
        }
    }
}

// ── Fingerprint ──────────────────────────────────────────────────────────────

/// A content-addressed identity for a `(source, section?, stage)` triple.
///
/// The `content_hash` is the SHA-256 hex digest of the source bytes.
/// `stage` is one of `"parse"`, `"analyze"`, `"render"`, or `"codegen"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint {
    /// Full hex SHA-256 of the source content.
    pub content_hash: String,
    /// Component/section name for multi-component files, or `None` for whole-file ops.
    pub section: Option<String>,
    /// Pipeline stage: `"parse"`, `"analyze"`, `"render"`, `"codegen"`.
    pub stage: String,
}

impl Fingerprint {
    /// Build a fingerprint from raw source text.
    pub fn new(source: &str, section: Option<&str>, stage: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let hash = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        Self {
            content_hash: hash,
            section: section.map(|s| s.to_string()),
            stage: stage.to_string(),
        }
    }

    /// A filesystem-safe cache key derived from this fingerprint.
    ///
    /// Format: `{first_16_hex_chars}-{section_or_underscore}-{stage}`
    pub fn cache_key(&self) -> String {
        let sec = self.section.as_deref().unwrap_or("_");
        format!("{}-{}-{}", &self.content_hash[..16], sec, self.stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_deterministic() {
        let a = Fingerprint::new("hello", None, "render");
        let b = Fingerprint::new("hello", None, "render");
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.cache_key(), b.cache_key());
    }

    #[test]
    fn different_source_different_fingerprint() {
        let a = Fingerprint::new("hello", None, "render");
        let b = Fingerprint::new("world", None, "render");
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn section_in_cache_key() {
        let fp = Fingerprint::new("src", Some("Card"), "codegen");
        assert!(fp.cache_key().contains("Card"));
    }

    #[test]
    fn test_classify_node_static_text() {
        let node = Node::Text(vec![TextPart::Literal("hello".to_string())]);
        assert_eq!(classify_node(&node), Region::Static);
    }

    #[test]
    fn test_classify_node_dynamic_text() {
        let node = Node::Text(vec![
            TextPart::Literal("hello ".to_string()),
            TextPart::Expr("name".to_string()),
        ]);
        assert_eq!(classify_node(&node), Region::Dynamic);
    }

    fn dummy_element() -> Element {
        Element {
            tag: "div".to_string(),
            id: None,
            classes: vec![],
            conditional_classes: vec![],
            event_handlers: vec![],
            bindings: vec![],
            animations: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_classify_node_static_element() {
        let node = Node::Element(dummy_element());
        assert_eq!(classify_node(&node), Region::Static);
    }

    #[test]
    fn test_classify_node_dynamic_element() {
        // test bindings
        let mut el = dummy_element();
        el.bindings.push(Binding {
            origin: crate::ast::BindingOrigin::Expression,
            prop: "src".to_string(),
            value: "url".to_string(),
        });
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test conditional classes
        let mut el = dummy_element();
        el.conditional_classes.push(ConditionalClass {
            class: "active".to_string(),
            condition: "is_active".to_string(),
        });
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test interpolated classes
        let mut el = dummy_element();
        el.classes.push("bg-{color}".to_string());
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test animations
        let mut el = dummy_element();
        el.animations.push(AnimationSpec {
            property: "opacity".to_string(),
            duration_expr: "300ms".to_string(),
            easing: "linear".to_string(),
            repeat: false,
        });
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test event handlers
        let mut el = dummy_element();
        el.event_handlers.push(EventHandler {
            model_before_listener: None,
            event: "click".to_string(),
            modifiers: vec![],
            handler: "on_click".to_string(),
        });
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test slot-rotate
        let mut el = dummy_element();
        el.tag = "slot-rotate".to_string();
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);

        // test dynamic child
        let mut el = dummy_element();
        el.children
            .push(Node::Text(vec![TextPart::Expr("name".to_string())]));
        assert_eq!(classify_node(&Node::Element(el)), Region::Dynamic);
    }

    #[test]
    fn test_classify_node_control_flow_and_others() {
        let if_node = Node::If(IfBlock {
            condition: "true".to_string(),
            then_children: vec![],
            else_children: None,
        });
        assert_eq!(classify_node(&if_node), Region::Dynamic);

        let for_node = Node::For(ForBlock {
            pattern: "item".to_string(),
            iterator: "items".to_string(),
            body: vec![],
        });
        assert_eq!(classify_node(&for_node), Region::Dynamic);

        let match_node = Node::Match(MatchBlock {
            expr: "value".to_string(),
            arms: vec![],
        });
        assert_eq!(classify_node(&match_node), Region::Dynamic);

        let let_node = Node::LetDecl(LetDecl {
            name: "x".to_string(),
            expr: "1".to_string(),
            is_default: false,
        });
        assert_eq!(classify_node(&let_node), Region::Dynamic);

        let include_node = Node::Include(IncludeNode {
            path: "other.crepus".to_string(),
            props: vec![],
            slot: vec![],
        });
        assert_eq!(classify_node(&include_node), Region::Dynamic);

        let embed_node = Node::Embed(EmbedNode {
            src: "foo".to_string(),
            adapter: None,
            props: vec![],
        });
        assert_eq!(classify_node(&embed_node), Region::Dynamic);

        let match_node_with_arms = Node::Match(MatchBlock {
            expr: "val".to_string(),
            arms: vec![
                MatchArm {
                    pattern: "1".to_string(),
                    body: vec![Node::Text(vec![TextPart::Literal("One".to_string())])],
                },
                MatchArm {
                    pattern: "_".to_string(),
                    body: vec![Node::Text(vec![TextPart::Expr("other".to_string())])],
                },
            ],
        });
        assert_eq!(
            classify_node_inner(&match_node_with_arms),
            (Region::Dynamic, 2)
        );

        let raw_text = Node::RawText("text".to_string());
        assert_eq!(classify_node(&raw_text), Region::Dynamic);

        let raw_html = Node::RawHtml("<div></div>".to_string());
        assert_eq!(classify_node(&raw_html), Region::Dynamic);
    }

    #[test]
    fn test_analyze_template() {
        let nodes = vec![
            // Index 0: Static text
            Node::Text(vec![TextPart::Literal("static text".to_string())]),
            // Index 1: Dynamic text (1 expr)
            Node::Text(vec![
                TextPart::Literal("hello ".to_string()),
                TextPart::Expr("name".to_string()),
            ]),
            // Index 2: Static element
            Node::Element(dummy_element()),
            // Index 3: Dynamic element (1 binding)
            Node::Element({
                let mut el = dummy_element();
                el.bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Expression,
                    prop: "src".to_string(),
                    value: "url".to_string(),
                });
                el
            }),
        ];

        let map = analyze_template(&nodes);
        assert_eq!(map.static_indices, vec![0, 2]);
        assert_eq!(map.dynamic_indices, vec![1, 3]);
        assert_eq!(map.expr_count, 2);
    }
}
