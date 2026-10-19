/// Runtime AST — mirrors the compile-time macro AST but operates on owned strings.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(Vec<TextPart>),
    If(IfBlock),
    For(ForBlock),
    Match(MatchBlock),
    LetDecl(LetDecl),
    Include(IncludeNode),
    Embed(EmbedNode),
    RawText(String),
    RawHtml(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub conditional_classes: Vec<ConditionalClass>,
    pub event_handlers: Vec<EventHandler>,
    pub bindings: Vec<Binding>,
    pub animations: Vec<AnimationSpec>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalClass {
    pub class: String,
    pub condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandler {
    /// Native Vue ordering between a model assignment and its matching update listener.
    pub model_before_listener: Option<bool>,
    pub event: String,
    pub modifiers: Vec<String>,
    pub handler: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingOrigin {
    Expression,
    Literal,
    /// An HTML attribute binding: null removes it; other values become strings.
    Attribute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub origin: BindingOrigin,
    pub prop: String,
    pub value: String,
}

impl Binding {
    /// Match a known HTML semantic property without changing its serialized spelling.
    /// Property/directive names retain their existing case-sensitive behavior.
    pub fn matches_prop(&self, name: &str) -> bool {
        if self.origin == BindingOrigin::Attribute {
            self.prop.eq_ignore_ascii_case(name)
        } else {
            self.prop == name
        }
    }
}

#[cfg(test)]
mod binding_origin_tests {
    use super::*;

    #[test]
    fn attribute_semantic_matching_preserves_authored_spelling() {
        let attribute = Binding {
            origin: BindingOrigin::Attribute,
            prop: "ClAsS".into(),
            value: "name".into(),
        };
        assert!(attribute.matches_prop("class"));
        assert_eq!(attribute.prop, "ClAsS");
        for origin in [BindingOrigin::Expression, BindingOrigin::Literal] {
            let property = Binding {
                origin,
                ..attribute.clone()
            };
            assert!(!property.matches_prop("class"));
            assert!(property.matches_prop("ClAsS"));
        }
        let svg = Binding {
            origin: BindingOrigin::Attribute,
            prop: "viewBox".into(),
            value: "box".into(),
        };
        assert_eq!(svg.prop, "viewBox");
    }
}

/// An animation specification parsed from `animate:property={duration easing}`.
///
/// ```text
/// div animate:opacity={300ms ease-in-out} animate:scale={500ms linear}
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationSpec {
    /// The property to animate: "opacity", "scale", "slide-x", "slide-y", "fade", "pulse"
    pub property: String,
    /// Duration expression (e.g. "300ms", "1s", "0.5s")
    pub duration_expr: String,
    /// Easing function name: "linear", "ease-in-out", "quadratic", "bounce"
    pub easing: String,
    /// Whether the animation loops
    pub repeat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPart {
    Literal(String),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfBlock {
    pub condition: String,
    pub then_children: Vec<Node>,
    pub else_children: Option<Vec<Node>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForBlock {
    pub pattern: String,
    pub iterator: String,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchBlock {
    pub expr: String,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: String,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDecl {
    pub name: String,
    pub expr: String,
    /// If true, only sets the variable when it is not already present in the context.
    /// Used for component prop defaults: `$: default name = value`
    pub is_default: bool,
}

/// An `include` directive — embeds another `.crepus` file as a component.
///
/// ```text
/// include components/button.crepus label="Click me" count={total}
///     div p-4
///         "slot content"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeNode {
    /// Relative path to the included `.crepus` file.
    pub path: String,
    /// Props passed to the component: (key, expr_string) pairs.
    /// The expr_string is evaluated against the parent context.
    pub props: Vec<(String, String)>,
    /// Children of the include directive — become the component's slot content.
    pub slot: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedNode {
    pub src: String,
    pub adapter: Option<String>,
    pub props: Vec<(String, String)>,
}
