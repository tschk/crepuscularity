//! Compiler-private pattern projections; no renderer or runtime pattern interpreter.
use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};

const RESULT: &str = "T extends readonly unknown[] ? T : T extends Iterable<infer V> ? V[] : T extends ArrayLike<infer V> ? V[] : never";

/// Keep the existing derived-array source ABI and output unchanged.
pub(crate) fn fixed_gather(
    gather: &str,
    input: &str,
    slots: &[String],
    globals: &crate::source_globals::SourceGlobals,
) -> String {
    let realm = &globals.realm;
    let tuple = slots.join(",");
    format!("function {gather}<T>({input}:T):{RESULT}{{if({realm}.Array.isArray({input}))return {input} as unknown as ({RESULT});if(!({realm}.Symbol.iterator in ({input} as object)))return {realm}.Array.from({input} as ArrayLike<unknown>) as unknown as ({RESULT});const [{tuple}]={input} as Iterable<unknown>;return [{tuple}] as unknown as ({RESULT})}}")
}
fn rest_gather(
    gather: &str,
    input: &str,
    globals: &crate::source_globals::SourceGlobals,
) -> String {
    let realm = &globals.realm;
    format!("function {gather}<T>({input}:T):{RESULT}{{if({realm}.Array.isArray({input}))return {input} as unknown as ({RESULT});return {realm}.Array.from({input} as Iterable<unknown>|ArrayLike<unknown>) as unknown as ({RESULT})}}")
}

pub(crate) fn supported(pattern: &BindingPattern<'_>) -> bool {
    match pattern {
        BindingPattern::BindingIdentifier(_) => true,
        BindingPattern::AssignmentPattern(p) => supported(&p.left),
        BindingPattern::ObjectPattern(p) => {
            p.properties.iter().all(|p| {
                !p.computed
                    && matches!(
                        &p.key,
                        PropertyKey::StaticIdentifier(_) | PropertyKey::StringLiteral(_)
                    )
                    && supported(&p.value)
            }) && p.rest.as_ref().is_none_or(|r| supported(&r.argument))
        }
        BindingPattern::ArrayPattern(p) => {
            p.elements.iter().flatten().all(supported)
                && p.rest.as_ref().is_none_or(|r| supported(&r.argument))
        }
    }
}

#[derive(Clone)]
enum Read {
    Text(String),
    Member(Box<Read>, String),
    Fallback(Box<Read>, Span, String),
    Slice(Box<Read>, usize),
    Rest(Box<Read>, Vec<String>),
}
impl Read {
    fn render(
        &self,
        default: &impl Fn(Span) -> Result<String, String>,
        rest: &str,
    ) -> Result<String, String> {
        Ok(match self {
            Self::Text(s) => s.clone(),
            Self::Member(value, key) => format!(
                "{}[{}]",
                value.render(default, rest)?,
                serde_json::to_string(key)
                    .map_err(|e| format!("cannot encode pattern key: {e}"))?
            ),
            Self::Fallback(value, span, temp) => format!(
                "(()=>{{const {temp}={};return {temp}===void 0?({}):{temp}}})()",
                value.render(default, rest)?,
                default(*span)?
            ),
            Self::Slice(value, index) => format!("{}.slice({index})", value.render(default, rest)?),
            Self::Rest(value, keys) => format!(
                "{rest}({}, {})",
                value.render(default, rest)?,
                serde_json::to_string(keys).map_err(|e| format!("cannot encode rest keys: {e}"))?
            ),
        })
    }
}
struct Gather {
    helper: String,
    name: String,
    read: Read,
    function: String,
}
pub(crate) struct Leaf<'s, 'a> {
    pub id: &'s BindingIdentifier<'a>,
    read: Read,
    memo: Option<String>,
}
pub(crate) struct Plan<'s, 'a> {
    pub leaves: Vec<Leaf<'s, 'a>>,
    gathers: Vec<Gather>,
    pub needs_rest: bool,
}
impl<'s, 'a> Plan<'s, 'a> {
    pub(crate) fn build(
        pattern: &'s BindingPattern<'a>,
        raw: &str,
        fresh: &mut impl FnMut(&str) -> String,
        globals: &crate::source_globals::SourceGlobals,
    ) -> Result<Self, String> {
        let mut plan = Self {
            leaves: Vec::new(),
            gathers: Vec::new(),
            needs_rest: false,
        };
        plan.walk(
            pattern,
            Read::Text(format!("{raw}()")),
            false,
            fresh,
            globals,
        )?;
        Ok(plan)
    }
    fn walk(
        &mut self,
        pattern: &'s BindingPattern<'a>,
        read: Read,
        default: bool,
        fresh: &mut impl FnMut(&str) -> String,
        globals: &crate::source_globals::SourceGlobals,
    ) -> Result<(), String> {
        match pattern {
            BindingPattern::BindingIdentifier(id) => self.leaves.push(Leaf {
                id,
                read,
                memo: default.then(|| fresh("__crepusEachDefault")),
            }),
            BindingPattern::AssignmentPattern(p) => self.walk(
                &p.left,
                Read::Fallback(
                    Box::new(read),
                    p.right.span(),
                    fresh("__crepusEachFallback"),
                ),
                true,
                fresh,
                globals,
            )?,
            BindingPattern::ObjectPattern(p) => {
                let mut keys = Vec::new();
                for prop in &p.properties {
                    if prop.computed {
                        return Err("computed each pattern keys are unsupported".into());
                    }
                    let key = match &prop.key {
                        PropertyKey::StaticIdentifier(k) => k.name.to_string(),
                        PropertyKey::StringLiteral(k) => k.value.to_string(),
                        _ => return Err("unsupported each pattern property key".into()),
                    };
                    keys.push(key.clone());
                    self.walk(
                        &prop.value,
                        Read::Member(Box::new(read.clone()), key),
                        default,
                        fresh,
                        globals,
                    )?;
                }
                if let Some(rest) = &p.rest {
                    self.needs_rest = true;
                    self.walk(
                        &rest.argument,
                        Read::Rest(Box::new(read), keys),
                        default,
                        fresh,
                        globals,
                    )?;
                }
            }
            BindingPattern::ArrayPattern(p) => {
                let name = fresh("__crepusEachArray");
                let helper = fresh("__crepusEachGather");
                let input = fresh("__crepusEachInput");
                let function = if p.rest.is_some() {
                    rest_gather(&helper, &input, globals)
                } else {
                    let slots = (0..p.elements.len())
                        .map(|_| fresh("__crepusEachSlot"))
                        .collect::<Vec<_>>();
                    fixed_gather(&helper, &input, &slots, globals)
                };
                // This source contains only the compiler-owned gather template,
                // never authored expressions/types. Preserve the old derived
                // caller's byte-identical builder while qualifying new each types.
                let types = &globals.types;
                let function = function
                    .replace(" extends Iterable<", &format!(" extends {types}.Iterable<"))
                    .replace(
                        " extends ArrayLike<",
                        &format!(" extends {types}.ArrayLike<"),
                    )
                    .replace(" as Iterable<", &format!(" as {types}.Iterable<"))
                    .replace(" as ArrayLike<", &format!(" as {types}.ArrayLike<"))
                    .replace("|ArrayLike<", &format!("|{types}.ArrayLike<"));
                self.gathers.push(Gather {
                    helper,
                    name: name.clone(),
                    read,
                    function,
                });
                for (index, element) in p.elements.iter().enumerate() {
                    if let Some(element) = element {
                        self.walk(
                            element,
                            Read::Member(
                                Box::new(Read::Text(format!("{name}.value"))),
                                index.to_string(),
                            ),
                            default,
                            fresh,
                            globals,
                        )?;
                    }
                }
                if let Some(rest) = &p.rest {
                    self.walk(
                        &rest.argument,
                        Read::Slice(
                            Box::new(Read::Text(format!("{name}.value"))),
                            p.elements.len(),
                        ),
                        default,
                        fresh,
                        globals,
                    )?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn needs_memo(&self) -> bool {
        !self.gathers.is_empty() || self.leaves.iter().any(|leaf| leaf.memo.is_some())
    }
    pub(crate) fn default_spans(&self) -> Vec<Span> {
        fn spans(read: &Read, out: &mut Vec<Span>) {
            match read {
                Read::Fallback(read, span, _) => {
                    out.push(*span);
                    spans(read, out)
                }
                Read::Member(read, _) | Read::Slice(read, _) | Read::Rest(read, _) => {
                    spans(read, out)
                }
                Read::Text(_) => {}
            }
        }
        let mut out = Vec::new();
        for leaf in &self.leaves {
            spans(&leaf.read, &mut out)
        }
        for gather in &self.gathers {
            spans(&gather.read, &mut out)
        }
        out
    }
    pub(crate) fn declarations(
        &self,
        memo: &str,
        rest: &str,
        default: &impl Fn(Span) -> Result<String, String>,
    ) -> Result<String, String> {
        let mut source = String::new();
        for g in &self.gathers {
            source.push_str(&format!(
                "{}const {}={memo}(()=>{}({}));",
                g.function,
                g.name,
                g.helper,
                g.read.render(default, rest)?
            ));
        }
        for leaf in &self.leaves {
            if let Some(name) = &leaf.memo {
                source.push_str(&format!(
                    "const {name}={memo}(()=>{});",
                    leaf.read.render(default, rest)?
                ));
            }
        }
        Ok(source)
    }
    pub(crate) fn read(
        &self,
        index: usize,
        rest: &str,
        default: &impl Fn(Span) -> Result<String, String>,
    ) -> Result<String, String> {
        let leaf = &self.leaves[index];
        if let Some(memo) = &leaf.memo {
            Ok(format!("{memo}.value"))
        } else {
            leaf.read.render(default, rest)
        }
    }
}

pub(crate) fn rest_helper(
    name: &str,
    shape: &str,
    globals: &crate::source_globals::SourceGlobals,
) -> String {
    let realm = &globals.realm;
    // Svelte DOM includes enumerable inherited strings, unlike native SSR/key rest.
    // Create own data properties so an authored __proto__ never mutates the result prototype.
    format!("type {shape}<T,K>={{[P in keyof T as P extends K?never:P]:T[P]}};function {name}<T,K extends keyof T>(input:T,keys:readonly K[]):{shape}<T,K>{{const result={{}};const excluded:readonly (string|number|symbol)[]=keys;for(const key in input){{if(!excluded.includes(key)){realm}.Object.defineProperty(result,key,{{value:input[key],enumerable:true,writable:true,configurable:true}});}}for(const key of {realm}.Object.getOwnPropertySymbols(input)){{if({realm}.Object.prototype.propertyIsEnumerable.call(input,key)&&!excluded.includes(key)){realm}.Object.defineProperty(result,key,{{value:input[key as keyof T],enumerable:true,writable:true,configurable:true}});}}return result as {shape}<T,K>;}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    fn render(pattern: &str) -> String {
        render_with(pattern, &Default::default())
    }
    fn render_with(pattern: &str, globals: &crate::source_globals::SourceGlobals) -> String {
        let allocator = Allocator::default();
        let wrapped = format!("({pattern})=>0");
        let Expression::ArrowFunctionExpression(arrow) =
            Parser::new(&allocator, &wrapped, SourceType::tsx())
                .parse_expression()
                .unwrap()
        else {
            panic!("expected pattern callback")
        };
        let mut count = 0;
        let mut fresh = |base: &str| {
            let name = format!("{base}{count}");
            count += 1;
            name
        };
        let plan = Plan::build(&arrow.params.items[0].pattern, "raw", &mut fresh, globals).unwrap();
        let default = |span: Span| Ok(wrapped[span.start as usize..span.end as usize].to_owned());
        let mut source = plan.declarations("memo", "rest", &default).unwrap();
        for (index, leaf) in plan.leaves.iter().enumerate() {
            source.push_str(&format!(
                "const read_{}=()=>({});",
                leaf.id.name,
                plan.read(index, "rest", &default).unwrap()
            ));
        }
        let parsed = Parser::new(&allocator, &source, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
        source
    }
    #[test]
    fn fixed_gather_preserves_the_existing_source_builder_byte_for_byte() {
        let expected="function gather<T>(input:T):T extends readonly unknown[] ? T : T extends Iterable<infer V> ? V[] : T extends ArrayLike<infer V> ? V[] : never{if(globalThis.Array.isArray(input))return input as unknown as (T extends readonly unknown[] ? T : T extends Iterable<infer V> ? V[] : T extends ArrayLike<infer V> ? V[] : never);if(!(globalThis.Symbol.iterator in (input as object)))return globalThis.Array.from(input as ArrayLike<unknown>) as unknown as (T extends readonly unknown[] ? T : T extends Iterable<infer V> ? V[] : T extends ArrayLike<infer V> ? V[] : never);const [a,b]=input as Iterable<unknown>;return [a,b] as unknown as (T extends readonly unknown[] ? T : T extends Iterable<infer V> ? V[] : T extends ArrayLike<infer V> ? V[] : never)}";
        assert_eq!(
            fixed_gather(
                "gather",
                "input",
                &["a".into(), "b".into()],
                &Default::default()
            ),
            expected
        );
    }
    #[test]
    fn defaults_are_separate_lazy_leaves_but_arrays_share_one_gather() {
        let source = render("[unused=fallback('unused'),used=fallback('used')]");
        assert_eq!(
            source.matches("function __crepusEachGather").count(),
            1,
            "{source}"
        );
        assert_eq!(source.matches("=memo(").count(), 3, "{source}");
        assert_eq!(source.matches("fallback('unused')").count(), 1, "{source}");
        assert_eq!(source.matches("fallback('used')").count(), 1, "{source}");
    }
    #[test]
    fn nested_rest_and_holes_keep_native_array_and_live_rest_operations() {
        let source = render("{items:[first,,...tail],nested:{value=3},...other}");
        assert!(source.contains("globalThis.Array.from"), "{source}");
        assert!(source.contains("globalThis.Iterable<"), "{source}");
        assert!(source.contains("globalThis.ArrayLike<"), "{source}");
        assert!(!source.contains(" as Iterable<"), "{source}");
        assert!(!source.contains(" extends ArrayLike<"), "{source}");
        assert!(source.contains(".slice(2)"), "{source}");
        assert!(
            source.contains("rest(raw(), [\"items\",\"nested\"])"),
            "{source}"
        );
        assert!(source.contains("[\"0\"]"), "{source}");
    }
}
#[cfg(test)]
mod globals_tests {
    use super::*;
    #[test]
    fn supplied_realm_and_type_aliases_replace_only_generated_references() {
        let globals = crate::source_globals::SourceGlobals::with_aliases("R", "G", true);
        let gather = rest_gather("g", "i", &globals);
        assert!(gather.contains("if((R()).Array.isArray(i))"), "{gather}");
        assert!(!gather.contains("globalThis"), "{gather}");
        let helper = rest_helper("h", "S", &globals);
        assert!(helper.contains("(R()).Object.defineProperty"), "{helper}");
        assert!(!helper.contains("globalThis"), "{helper}");
    }
}
