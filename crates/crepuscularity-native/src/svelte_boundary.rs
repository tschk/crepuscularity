//! Let the existing JavaScript parser locate authored expression boundaries.
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

fn trivia(source: &str, mut at: usize) -> Option<usize> {
    loop {
        while let Some(ch) = source[at..].chars().next() {
            if !ch.is_whitespace() && ch != '\u{feff}' {
                break;
            }
            at += ch.len_utf8();
        }
        if source[at..].starts_with("/*") {
            at += 2 + source[at + 2..].find("*/")? + 2;
        } else if source[at..].starts_with("//") {
            at += source[at..].find(['\n', '\r', '\u{2028}', '\u{2029}'])?;
        } else {
            return Some(at);
        }
    }
}

pub(crate) fn boundary(source: &str) -> Option<usize> {
    let inner = source.strip_prefix('{')?;
    let body = inner.trim_start();
    let start = source.len() - body.len();
    if body.starts_with("...") {
        return expression_end(source, start + 3);
    }
    if let Some(header) = body.strip_prefix("#snippet") {
        let open = header.find('(')?;
        let params = &header[open..];
        // Parameter types may contain nested parentheses and comments. Ask Oxc
        // which closing parenthesis completes a formal parameter list.
        for (end, _) in params.match_indices(')') {
            let candidate = format!("{}=>0", &params[..=end]);
            let allocator = Allocator::default();
            if Parser::new(&allocator, &candidate, SourceType::tsx())
                .parse_expression()
                .is_ok_and(|expression| expression.span().end as usize == candidate.len())
            {
                let after = source.len() - params.len() + end + 1;
                let close = trivia(source, after)?;
                return (source.as_bytes().get(close) == Some(&b'}')).then_some(close);
            }
        }
        return None;
    }
    if let Some(declaration) = body
        .strip_prefix("@const")
        .filter(|tail| tail.starts_with(char::is_whitespace))
    {
        // Let Oxc distinguish a pattern/object/comment/regex brace from the
        // enclosing tag. Unsupported directives retain their old rejection path.
        for (end, _) in declaration.match_indices('}') {
            let candidate = format!("const {}\n", &declaration[..end]);
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &candidate, SourceType::tsx()).parse();
            // The synthetic newline must not terminate an authored line comment:
            // in that case this candidate brace still belongs to the comment.
            let closes_line_comment = parsed.program.comments.iter().any(|comment| {
                comment.is_line() && comment.span.end as usize == candidate.len() - 1
            });
            if parsed.errors.is_empty()
                && !closes_line_comment
                && matches!(parsed.program.body.as_slice(),[Statement::VariableDeclaration(declaration)] if declaration.declarations.len()==1&&declaration.declarations[0].init.is_some())
            {
                return Some(source.len() - declaration.len() + end);
            }
        }
        return None;
    }
    if let Some(header) = body
        .strip_prefix("#await")
        .filter(|tail| tail.starts_with(char::is_whitespace))
    {
        for (end, _) in header.match_indices('}') {
            if crate::svelte_await::header(&header[..end]).is_ok() {
                return Some(source.len() - header.len() + end);
            }
        }
        return None;
    }
    for prefix in [":then", ":catch"] {
        if let Some(pattern) = body.strip_prefix(prefix) {
            if !pattern.is_empty()
                && !pattern.starts_with(char::is_whitespace)
                && !pattern.starts_with('}')
            {
                return None;
            }
            for (end, _) in pattern.match_indices('}') {
                if crate::svelte_await::binding(pattern[..end].trim()) {
                    return Some(source.len() - pattern.len() + end);
                }
            }
            return None;
        }
    }
    if let Some(header) = body.strip_prefix("#each ") {
        for (end, _) in header.match_indices('}') {
            // Boundary discovery precedes script-language selection; lowering revalidates it.
            if crate::svelte_each::header_boundary(
                &header[..end],
                crate::svelte_javascript::IrPolicy::Typed,
            ) || crate::svelte_each::header_boundary(
                &header[..end],
                crate::svelte_javascript::IrPolicy::JavaScript,
            ) {
                return Some(source.len() - header.len() + end);
            }
        }
    }
    for prefix in ["#if", "#key", ":else if", "@html", "@render"] {
        if let Some(rest) = body.strip_prefix(prefix) {
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            return expression_end(source, start + prefix.len());
        }
    }
    if body.starts_with(['/', ':', '#', '@']) {
        // Unsupported directives and literal block terminators are validated by
        // the markup parser, rather than interpreted as JavaScript.
        return body.find('}').map(|offset| start + offset);
    }
    expression_end(source, start)
}

fn expression_end(source: &str, start: usize) -> Option<usize> {
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, &source[start..], SourceType::tsx())
        .parse_expression()
        .ok()?;
    let close = trivia(source, start + expression.span().end as usize)?;
    (source.as_bytes().get(close) == Some(&b'}')).then_some(close)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn javascript_grammar_distinguishes_comments_regex_and_division() {
        for source in [
            "{value /* } */ + 2}",
            "{...props /* } */}",
            "{...(/}/).test(value) ? first : second}",
            "{value // }\n + 2}",
            "{(/}/).test('}')}",
            "{(/{/).test('{')}",
            "{(/[/*]/).test('/')}",
            "{(/[//]/).test('/')}",
            "{8 / 2 / 2}",
            "{8 / /}/.source.length}",
            "{(()=>{if(true) /}/.test('}');return 8})()}",
            "{`a${`b${value}`}c`}",
            "{value // }\u{2028}}",
            "{@html value /* } */}",
            "{#if (/}/).test(value)}",
            "{#key value /* } */}",
            "{@render row(value /* } */)}",
            "{#snippet row(value:string,callback:()=>number)}",
            "{#snippet row(value /* ) } */:string)}",
            "{/snippet}",
            "{#await input then {nested:{value=defaultValue}={},...rest}}",
            "{#await (/then}/).test(value) ? first : second}",
            "{:then [first,,...rest]}",
            "{:catch {nested:{message='}'}}}",
            "{:then value:Result}",
            "{/await}",
            "{:else}",
            "{:else if /}/.test(value)}",
        ] {
            assert_eq!(
                boundary(&format!("{source}<p/>")),
                Some(source.len() - 1),
                "{source}"
            );
        }
    }
    #[test]
    fn invalid_or_unterminated_expressions_do_not_guess_a_boundary() {
        for source in [
            "{value /* }",
            "{value // }",
            "{(/}/)",
            "{value extra}",
            "{...}",
            "{#if}",
            "{value /*missing}",
            "{#snippet row(value}",
        ] {
            assert_eq!(boundary(source), None, "{source}");
        }
    }
    #[test]
    fn const_boundaries_use_declaration_grammar_for_patterns_and_brace_tokens() {
        for source in [
            "{@const {a=1,...rest}={a:2}}",
            "{@const [a={nested:1},...rest]=[{}]}",
            "{@const x= /}/.test('}')}",
            "{@const x=call(/* } */ {a:'}'})}",
            "{@const x=1 // }\n}",
            "{@const x=(()=>{return {a:1}})()}",
        ] {
            assert_eq!(boundary(source), Some(source.len() - 1), "{source}");
        }
        for source in ["{@const x}", "{@const x=1,y=2}", "{@const export x=1}"] {
            assert!(boundary(source).is_none(), "{source}");
        }
    }
}
