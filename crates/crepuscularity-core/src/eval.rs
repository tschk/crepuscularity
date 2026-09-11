/// Expression evaluator for the crepuscularity template DSL.
///
/// Supports: literals, variables, property access, arithmetic, comparisons, boolean ops.
///
/// Grammar (highest precedence last):
///   expr = or
///   or   = and ("||" and)*
///   and  = cmp  ("&&" cmp)*
///   cmp  = add  (("==" | "!=" | "<" | "<=" | ">" | ">=") add)*
///   add  = mul  (("+" | "-") mul)*
///   mul  = una  (("*" | "/" | "%") una)*
///   una  = "!" una | "-" una | post
///   post = prim ("." ident)*
///   prim = int | float | str | bool | null | "(" expr ")" | ident
use crate::context::{TemplateContext, TemplateValue};
use crate::error::CrepusError;

// ── Tokens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AmpAmp,
    PipePipe,
    Bang,
    Dot,
    LParen,
    RParen,
    Eof,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // String literal — must walk UTF-8 codepoints, not raw bytes, so
        // multi-byte chars (`"Hola ñ"`) survive the round-trip intact.
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            let mut s = String::new();
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 1;
                    // The escaped item is a codepoint, not a byte: advancing by one byte
                    // past a multi-byte char would leave `i` mid-codepoint.
                    let esc = input[i..].chars().next().unwrap_or('\\');
                    match esc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        c => {
                            s.push('\\');
                            s.push(c);
                        }
                    }
                    i += esc.len_utf8();
                } else {
                    let ch = input[i..]
                        .chars()
                        .next()
                        .expect("byte index inside string literal must point at a UTF-8 codepoint");
                    let n = ch.len_utf8();
                    s.push(ch);
                    i += n;
                }
            }
            if i < len {
                i += 1;
            } // consume closing quote
            tokens.push(Token::Str(s));
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'-'
                && i + 1 < len
                && bytes[i + 1].is_ascii_digit()
                && tokens.last().is_none_or(|t| {
                    matches!(
                        t,
                        Token::LParen
                            | Token::Plus
                            | Token::Minus
                            | Token::Star
                            | Token::Slash
                            | Token::Percent
                            | Token::EqEq
                            | Token::BangEq
                            | Token::Lt
                            | Token::LtEq
                            | Token::Gt
                            | Token::GtEq
                            | Token::AmpAmp
                            | Token::PipePipe
                            | Token::Bang
                    )
                }))
        {
            let start = i;
            if bytes[i] == b'-' {
                i += 1;
            }
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let is_float =
                i < len && bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit();
            if is_float {
                i += 1; // consume '.'
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if let Ok(f) = input[start..i].parse::<f64>() {
                    tokens.push(Token::Float(f));
                }
            } else if let Ok(n) = input[start..i].parse::<i64>() {
                tokens.push(Token::Int(n));
            }
            continue;
        }

        // Identifiers and keywords
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &input[start..i];
            tokens.push(match word {
                "true" => Token::Bool(true),
                "false" => Token::Bool(false),
                "null" | "none" | "None" | "Null" => Token::Null,
                _ => Token::Ident(word.to_string()),
            });
            continue;
        }

        // Two-char operators (check before single-char)
        if i + 1 < len {
            let two = &input[i..i + 2];
            let tok = match two {
                "==" => Some(Token::EqEq),
                "!=" => Some(Token::BangEq),
                "<=" => Some(Token::LtEq),
                ">=" => Some(Token::GtEq),
                "&&" => Some(Token::AmpAmp),
                "||" => Some(Token::PipePipe),
                _ => None,
            };
            if let Some(t) = tok {
                tokens.push(t);
                i += 2;
                continue;
            }
        }

        // Single-char operators
        let tok = match bytes[i] {
            b'+' => Some(Token::Plus),
            b'-' => Some(Token::Minus),
            b'*' => Some(Token::Star),
            b'/' => Some(Token::Slash),
            b'%' => Some(Token::Percent),
            b'<' => Some(Token::Lt),
            b'>' => Some(Token::Gt),
            b'!' => Some(Token::Bang),
            b'.' => Some(Token::Dot),
            b'(' => Some(Token::LParen),
            b')' => Some(Token::RParen),
            _ => None,
        };
        if let Some(t) = tok {
            tokens.push(t);
        }
        i += 1;
    }

    tokens.push(Token::Eof);
    tokens
}

// ── Parser ───────────────────────────────────────────────────────────────────

/// Maximum recursive descent depth. Every recursion cycle in the grammar passes
/// through `parse_unary`, so bounding it there bounds total stack usage.
const MAX_EXPR_DEPTH: usize = 128;

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
    depth_exceeded: bool,
    ctx: &'a TemplateContext,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token>, ctx: &'a TemplateContext) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
            depth_exceeded: false,
            ctx,
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn eat(&mut self, expected: &Token) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    // expr = or
    fn parse_expr(&mut self) -> TemplateValue {
        self.parse_or()
    }

    // or = and ("||" and)*
    fn parse_or(&mut self) -> TemplateValue {
        let mut lhs = self.parse_and();
        while *self.peek() == Token::PipePipe {
            self.advance();
            let rhs = self.parse_and();
            lhs = TemplateValue::Bool(is_truthy(&lhs) || is_truthy(&rhs));
        }
        lhs
    }

    // and = cmp ("&&" cmp)*
    fn parse_and(&mut self) -> TemplateValue {
        let mut lhs = self.parse_cmp();
        while *self.peek() == Token::AmpAmp {
            self.advance();
            let rhs = self.parse_cmp();
            lhs = TemplateValue::Bool(is_truthy(&lhs) && is_truthy(&rhs));
        }
        lhs
    }

    // cmp = add (op add)*
    fn parse_cmp(&mut self) -> TemplateValue {
        let lhs = self.parse_add();
        let op = match self.peek() {
            Token::EqEq => "==",
            Token::BangEq => "!=",
            Token::Lt => "<",
            Token::LtEq => "<=",
            Token::Gt => ">",
            Token::GtEq => ">=",
            _ => return lhs,
        };
        self.advance();
        let rhs = self.parse_add();
        TemplateValue::Bool(compare(&lhs, &rhs, op))
    }

    // add = mul (("+" | "-") mul)*
    fn parse_add(&mut self) -> TemplateValue {
        let mut lhs = self.parse_mul();
        loop {
            let op = match self.peek() {
                Token::Plus => "+",
                Token::Minus => "-",
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul();
            lhs = arith(&lhs, &rhs, op);
        }
        lhs
    }

    // mul = una (("*" | "/" | "%") una)*
    fn parse_mul(&mut self) -> TemplateValue {
        let mut lhs = self.parse_unary();
        loop {
            let op = match self.peek() {
                Token::Star => "*",
                Token::Slash => "/",
                Token::Percent => "%",
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary();
            lhs = arith(&lhs, &rhs, op);
        }
        lhs
    }

    // unary = "!" una | "-" una | post
    fn parse_unary(&mut self) -> TemplateValue {
        if self.depth >= MAX_EXPR_DEPTH {
            self.depth_exceeded = true;
            return TemplateValue::Null;
        }
        self.depth += 1;
        let value = self.parse_unary_inner();
        self.depth -= 1;
        value
    }

    fn parse_unary_inner(&mut self) -> TemplateValue {
        if self.eat(&Token::Bang) {
            let v = self.parse_unary();
            return TemplateValue::Bool(!is_truthy(&v));
        }
        if self.eat(&Token::Minus) {
            let v = self.parse_unary();
            return match v {
                TemplateValue::Int(n) => TemplateValue::Int(-n),
                TemplateValue::Float(f) => TemplateValue::Float(-f),
                _ => TemplateValue::Null,
            };
        }
        self.parse_postfix()
    }

    // postfix = primary ("." ident)*
    fn parse_postfix(&mut self) -> TemplateValue {
        let mut val = self.parse_primary();
        while *self.peek() == Token::Dot {
            self.advance();
            if let Token::Ident(prop) = self.advance() {
                val = apply_property(val, &prop);
            } else {
                break;
            }
        }
        val
    }

    // primary = int | float | str | bool | null | "(" expr ")" | ident
    fn parse_primary(&mut self) -> TemplateValue {
        match self.advance() {
            Token::Int(n) => TemplateValue::Int(n),
            Token::Float(f) => TemplateValue::Float(f),
            Token::Str(s) => TemplateValue::Str(s),
            Token::Bool(b) => TemplateValue::Bool(b),
            Token::Null => TemplateValue::Null,
            Token::LParen => {
                let v = self.parse_expr();
                self.eat(&Token::RParen);
                v
            }
            Token::Ident(name) => self
                .ctx
                .vars
                .get(&name)
                .cloned()
                .unwrap_or(TemplateValue::Null),
            _ => TemplateValue::Null,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn is_truthy(v: &TemplateValue) -> bool {
    match v {
        TemplateValue::Bool(b) => *b,
        TemplateValue::Int(n) => *n != 0,
        TemplateValue::Float(f) => *f != 0.0,
        TemplateValue::Str(s) => !s.is_empty(),
        TemplateValue::List(l) => !l.is_empty(),
        TemplateValue::Scope(s) => !s.vars.is_empty(),
        TemplateValue::Null => false,
    }
}

fn compare(lhs: &TemplateValue, rhs: &TemplateValue, op: &str) -> bool {
    // Numeric comparison (coerce Int to Float for mixed)
    let as_f64 = |v: &TemplateValue| -> Option<f64> {
        match v {
            TemplateValue::Int(n) => Some(*n as f64),
            TemplateValue::Float(f) => Some(*f),
            _ => None,
        }
    };

    if let (Some(l), Some(r)) = (as_f64(lhs), as_f64(rhs)) {
        return match op {
            "==" => (l - r).abs() < f64::EPSILON,
            "!=" => (l - r).abs() >= f64::EPSILON,
            "<" => l < r,
            "<=" => l <= r,
            ">" => l > r,
            ">=" => l >= r,
            _ => false,
        };
    }

    // String comparison
    let lstr = value_to_cmp_str(lhs);
    let rstr = value_to_cmp_str(rhs);
    match op {
        "==" => lstr == rstr,
        "!=" => lstr != rstr,
        "<" => lstr < rstr,
        "<=" => lstr <= rstr,
        ">" => lstr > rstr,
        ">=" => lstr >= rstr,
        _ => false,
    }
}

fn value_to_cmp_str(v: &TemplateValue) -> String {
    match v {
        TemplateValue::Str(s) => s.clone(),
        TemplateValue::Int(n) => n.to_string(),
        TemplateValue::Float(f) => f.to_string(),
        TemplateValue::Bool(b) => b.to_string(),
        TemplateValue::Null => String::new(),
        TemplateValue::List(l) => format!("[{} items]", l.len()),
        TemplateValue::Scope(_) => String::new(),
    }
}

fn arith(lhs: &TemplateValue, rhs: &TemplateValue, op: &str) -> TemplateValue {
    // String concatenation with +
    if op == "+" {
        if let TemplateValue::Str(l) = lhs {
            let r = crate::context::value_to_str(rhs);
            return TemplateValue::Str(format!("{}{}", l, r));
        }
        if let TemplateValue::Str(r) = rhs {
            let l = crate::context::value_to_str(lhs);
            return TemplateValue::Str(format!("{}{}", l, r));
        }
    }

    // Numeric arithmetic
    let as_nums = |l: &TemplateValue, r: &TemplateValue| -> Option<(bool, f64, f64)> {
        match (l, r) {
            (TemplateValue::Int(a), TemplateValue::Int(b)) => Some((true, *a as f64, *b as f64)),
            (TemplateValue::Float(a), TemplateValue::Float(b)) => Some((false, *a, *b)),
            (TemplateValue::Int(a), TemplateValue::Float(b)) => Some((false, *a as f64, *b)),
            (TemplateValue::Float(a), TemplateValue::Int(b)) => Some((false, *a, *b as f64)),
            _ => None,
        }
    };

    if let Some((both_int, l, r)) = as_nums(lhs, rhs) {
        let result = match op {
            "+" => l + r,
            "-" => l - r,
            "*" => l * r,
            "/" => {
                if r == 0.0 {
                    return TemplateValue::Null;
                } else {
                    l / r
                }
            }
            "%" => {
                if r == 0.0 {
                    return TemplateValue::Null;
                } else {
                    l % r
                }
            }
            _ => return TemplateValue::Null,
        };
        if both_int && op != "/" {
            return TemplateValue::Int(result as i64);
        }
        return TemplateValue::Float(result);
    }

    TemplateValue::Null
}

fn apply_property(val: TemplateValue, prop: &str) -> TemplateValue {
    match (&val, prop) {
        (TemplateValue::Str(s), "len") | (TemplateValue::Str(s), "length") => {
            TemplateValue::Int(s.chars().count() as i64)
        }
        (TemplateValue::Str(s), "upper") => TemplateValue::Str(s.to_uppercase()),
        (TemplateValue::Str(s), "lower") => TemplateValue::Str(s.to_lowercase()),
        (TemplateValue::Str(s), "trim") => TemplateValue::Str(s.trim().to_string()),
        (TemplateValue::Str(s), "is_empty") | (TemplateValue::Str(s), "empty") => {
            TemplateValue::Bool(s.is_empty())
        }
        (TemplateValue::List(l), "len") | (TemplateValue::List(l), "length") => {
            TemplateValue::Int(l.len() as i64)
        }
        (TemplateValue::List(l), "is_empty") | (TemplateValue::List(l), "empty") => {
            TemplateValue::Bool(l.is_empty())
        }
        (TemplateValue::Int(n), "abs") => TemplateValue::Int(n.abs()),
        (TemplateValue::Float(f), "abs") => TemplateValue::Float(f.abs()),
        (TemplateValue::Float(f), "floor") => TemplateValue::Int(f.floor() as i64),
        (TemplateValue::Float(f), "ceil") => TemplateValue::Int(f.ceil() as i64),
        (TemplateValue::Float(f), "round") => TemplateValue::Int(f.round() as i64),
        (TemplateValue::Scope(ctx), prop) => {
            ctx.vars.get(prop).cloned().unwrap_or(TemplateValue::Null)
        }
        _ => TemplateValue::Null,
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn eval_expr(expr: &str, ctx: &TemplateContext) -> Result<TemplateValue, CrepusError> {
    let _span = if tracing::enabled!(tracing::Level::TRACE) {
        Some(tracing::trace_span!("eval_expr", expr = %expr).entered())
    } else {
        None
    };
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(TemplateValue::Null);
    }
    let tokens = tokenize(expr);
    let mut parser = Parser::new(tokens, ctx);
    let value = parser.parse_expr();
    if parser.depth_exceeded {
        return Err(CrepusError::eval(
            expr,
            "expression nesting too deep to evaluate",
        ));
    }
    if parser.is_at_end() {
        Ok(value)
    } else {
        Err(CrepusError::eval(
            expr,
            "unexpected tokens after expression",
        ))
    }
}

/// Evaluate an expression as a boolean condition.
pub fn eval_condition(expr: &str, ctx: &TemplateContext) -> Result<bool, CrepusError> {
    Ok(is_truthy(&eval_expr(expr, ctx)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_expr_basic() {
        let mut ctx = TemplateContext::new();
        ctx.vars.insert("x".into(), TemplateValue::Int(10));
        ctx.vars.insert("b".into(), TemplateValue::Bool(true));

        assert!(matches!(eval_expr("", &ctx).unwrap(), TemplateValue::Null));
        assert!(matches!(
            eval_expr("null", &ctx).unwrap(),
            TemplateValue::Null
        ));
        assert!(matches!(
            eval_expr("42", &ctx).unwrap(),
            TemplateValue::Int(42)
        ));
        #[allow(clippy::approx_constant)]
        assert!(
            matches!(eval_expr("3.14", &ctx).unwrap(), TemplateValue::Float(f) if (f - 3.14).abs() < f64::EPSILON)
        );
        assert!(matches!(eval_expr("\"foo\"", &ctx).unwrap(), TemplateValue::Str(s) if s == "foo"));
        assert!(matches!(
            eval_expr("true", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("false", &ctx).unwrap(),
            TemplateValue::Bool(false)
        ));
        assert!(matches!(
            eval_expr("x", &ctx).unwrap(),
            TemplateValue::Int(10)
        ));
    }

    #[test]
    fn test_eval_expr_arithmetic() {
        let ctx = TemplateContext::new();
        assert!(matches!(
            eval_expr("1 + 2", &ctx).unwrap(),
            TemplateValue::Int(3)
        ));
        assert!(matches!(
            eval_expr("5 - 3", &ctx).unwrap(),
            TemplateValue::Int(2)
        ));
        assert!(matches!(
            eval_expr("4 * 3", &ctx).unwrap(),
            TemplateValue::Int(12)
        ));
        assert!(matches!(
            eval_expr("10 / 2", &ctx).unwrap(),
            TemplateValue::Float(f) if (f - 5.0).abs() < f64::EPSILON
        ));
        assert!(matches!(
            eval_expr("10 % 3", &ctx).unwrap(),
            TemplateValue::Int(1)
        ));
        assert!(
            matches!(eval_expr("2.5 * 2.0", &ctx).unwrap(), TemplateValue::Float(f) if (f - 5.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn test_eval_expr_comparison() {
        let ctx = TemplateContext::new();
        assert!(matches!(
            eval_expr("1 == 1", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("1 != 2", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("5 > 3", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("3 < 5", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("5 >= 5", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("3 <= 5", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("\"a\" == \"a\"", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
    }

    #[test]
    fn test_eval_expr_boolean() {
        let ctx = TemplateContext::new();
        assert!(matches!(
            eval_expr("true && true", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("true && false", &ctx).unwrap(),
            TemplateValue::Bool(false)
        ));
        assert!(matches!(
            eval_expr("true || false", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("false || false", &ctx).unwrap(),
            TemplateValue::Bool(false)
        ));
        assert!(matches!(
            eval_expr("!true", &ctx).unwrap(),
            TemplateValue::Bool(false)
        ));
        assert!(matches!(
            eval_expr("!false", &ctx).unwrap(),
            TemplateValue::Bool(true)
        ));
    }

    #[test]
    fn test_eval_expr_properties() {
        let ctx = TemplateContext::new();
        assert!(matches!(
            eval_expr("\"abc\".len", &ctx).unwrap(),
            TemplateValue::Int(3)
        ));
        assert!(
            matches!(eval_expr("\"abc\".upper", &ctx).unwrap(), TemplateValue::Str(s) if s == "ABC")
        );
        assert!(matches!(
            eval_expr("-5.abs", &ctx).unwrap(),
            TemplateValue::Int(5)
        ));
    }

    #[test]
    fn test_eval_condition_truthy_falsy() {
        let ctx = TemplateContext::new();

        // Truthy values
        assert!(eval_condition("true", &ctx).unwrap());
        assert!(eval_condition("1", &ctx).unwrap());
        assert!(eval_condition("-1", &ctx).unwrap());
        assert!(eval_condition("0.1", &ctx).unwrap());
        assert!(eval_condition("-0.1", &ctx).unwrap());
        assert!(eval_condition("\"hello\"", &ctx).unwrap());
        assert!(eval_condition("1 + 2", &ctx).unwrap());
        // String coercion: non-empty strings are truthy, even if they contain falsy text
        assert!(eval_condition("\"false\"", &ctx).unwrap());
        assert!(eval_condition("\"0\"", &ctx).unwrap());

        // Falsy values
        assert!(!eval_condition("false", &ctx).unwrap());
        assert!(!eval_condition("0", &ctx).unwrap());
        assert!(!eval_condition("0.0", &ctx).unwrap());
        assert!(!eval_condition("\"\"", &ctx).unwrap());
        assert!(!eval_condition("null", &ctx).unwrap());
    }

    #[test]
    fn test_eval_condition_complex_types_coercion() {
        let mut ctx = TemplateContext::new();

        // List coercion
        ctx.vars
            .insert("empty_list".into(), TemplateValue::List(vec![]));
        ctx.vars.insert(
            "full_list".into(),
            TemplateValue::List(vec![TemplateContext::new()]),
        );

        // Scope coercion
        let empty_scope = TemplateContext::new();
        let mut full_scope = TemplateContext::new();
        full_scope
            .vars
            .insert("key".into(), TemplateValue::Bool(true));

        ctx.vars
            .insert("empty_scope".into(), TemplateValue::Scope(empty_scope));
        ctx.vars
            .insert("full_scope".into(), TemplateValue::Scope(full_scope));

        // Lists
        assert!(!eval_condition("empty_list", &ctx).unwrap());
        assert!(eval_condition("full_list", &ctx).unwrap());

        // Scopes
        assert!(!eval_condition("empty_scope", &ctx).unwrap());
        assert!(eval_condition("full_scope", &ctx).unwrap());
    }

    #[test]
    fn test_is_truthy_direct() {
        assert!(is_truthy(&TemplateValue::Bool(true)));
        assert!(!is_truthy(&TemplateValue::Bool(false)));
        assert!(is_truthy(&TemplateValue::Int(42)));
        assert!(!is_truthy(&TemplateValue::Int(0)));
        assert!(is_truthy(&TemplateValue::Float(3.14)));
        assert!(!is_truthy(&TemplateValue::Float(0.0)));
        assert!(is_truthy(&TemplateValue::Str("foo".into())));
        assert!(!is_truthy(&TemplateValue::Str("".into())));
        assert!(is_truthy(&TemplateValue::List(
            vec![TemplateContext::new()]
        )));
        assert!(!is_truthy(&TemplateValue::List(vec![])));
        let mut scope = TemplateContext::new();
        scope.vars.insert("a".into(), TemplateValue::Int(1));
        assert!(is_truthy(&TemplateValue::Scope(scope)));
        assert!(!is_truthy(&TemplateValue::Scope(TemplateContext::new())));
        assert!(!is_truthy(&TemplateValue::Null));
    }

    #[test]
    fn test_eval_condition_with_context() {
        let mut ctx = TemplateContext::new();
        ctx.vars
            .insert("empty_str".into(), TemplateValue::Str("".into()));
        ctx.vars
            .insert("full_str".into(), TemplateValue::Str("foo".into()));
        ctx.vars.insert("zero".into(), TemplateValue::Int(0));
        ctx.vars.insert("nonzero".into(), TemplateValue::Int(42));

        assert!(!eval_condition("empty_str", &ctx).unwrap());
        assert!(eval_condition("full_str", &ctx).unwrap());
        assert!(!eval_condition("zero", &ctx).unwrap());
        assert!(eval_condition("nonzero", &ctx).unwrap());

        // Compound condition
        assert!(eval_condition("full_str && nonzero", &ctx).unwrap());
        assert!(!eval_condition("empty_str && nonzero", &ctx).unwrap());
        assert!(eval_condition("empty_str || nonzero", &ctx).unwrap());
    }

    #[test]
    fn test_eval_condition_errors() {
        let ctx = TemplateContext::new();

        // Syntax errors (trailing tokens)
        assert!(eval_condition("true foo", &ctx).is_err());
    }

    #[test]
    fn string_literal_preserves_multi_byte_utf8() {
        let ctx = TemplateContext::new();
        let v = eval_expr("\"Hola ñ — 你好\"", &ctx).expect("valid literal");
        match v {
            TemplateValue::Str(s) => assert_eq!(s, "Hola ñ — 你好"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn string_concatenation_preserves_unicode() {
        let mut ctx = TemplateContext::new();
        ctx.vars
            .insert("name".into(), TemplateValue::Str("世界".into()));
        let v = eval_expr("\"hi \" + name", &ctx).expect("valid concat");
        match v {
            TemplateValue::Str(s) => assert_eq!(s, "hi 世界"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn unterminated_string_does_not_panic() {
        // Regression: byte-indexed tokenizer must not slice on a non-char
        // boundary even when the literal runs off the end of input.
        let ctx = TemplateContext::new();
        let _ = eval_expr("\"abc", &ctx);
        let _ = eval_expr("\"ñ", &ctx);
    }

    #[test]
    fn trailing_tokens_are_errors() {
        let ctx = TemplateContext::new();
        let err = eval_expr("1 + 2 foo", &ctx).unwrap_err();
        assert!(err.to_string().contains("unexpected tokens"));
    }
}
