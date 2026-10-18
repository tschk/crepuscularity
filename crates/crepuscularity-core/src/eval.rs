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
use std::borrow::Cow;

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

fn tokenize(input: &str) -> Result<Vec<Token>, CrepusError> {
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
                        c if c == quote as char => s.push(c),
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
            if i == len {
                return Err(CrepusError::eval(input, "unterminated string literal"));
            }
            i += 1; // consume closing quote
            tokens.push(Token::Str(s));
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
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
            let mut is_float = if i < len && bytes[i] == b'.' {
                let property = input[i + 1..]
                    .trim_start()
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
                !property
                    || (i + 2 < len
                        && matches!(bytes[i + 1], b'e' | b'E')
                        && (bytes[i + 2].is_ascii_digit() || matches!(bytes[i + 2], b'+' | b'-')))
            } else {
                false
            };
            if is_float {
                i += 1; // consume '.'
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < len && matches!(bytes[i], b'e' | b'E') {
                is_float = true;
                i += 1;
                if i < len && matches!(bytes[i], b'+' | b'-') {
                    i += 1;
                }
                let exponent_start = i;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i == exponent_start {
                    return Err(CrepusError::eval(input, "invalid floating-point exponent"));
                }
            }
            if is_float {
                let f = input[start..i]
                    .parse::<f64>()
                    .map_err(|_| CrepusError::eval(input, "invalid floating-point literal"))?;
                if !f.is_finite() {
                    return Err(CrepusError::eval(
                        input,
                        "floating-point literal out of range",
                    ));
                }
                tokens.push(Token::Float(f));
            } else {
                let n = input[start..i]
                    .parse::<i64>()
                    .map_err(|_| CrepusError::eval(input, "integer literal out of range"))?;
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
            let tok = match (bytes[i], bytes[i + 1]) {
                (b'=', b'=') => Some(Token::EqEq),
                (b'!', b'=') => Some(Token::BangEq),
                (b'<', b'=') => Some(Token::LtEq),
                (b'>', b'=') => Some(Token::GtEq),
                (b'&', b'&') => Some(Token::AmpAmp),
                (b'|', b'|') => Some(Token::PipePipe),
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
        } else {
            let ch = input[i..].chars().next().unwrap();
            return Err(CrepusError::eval(
                input,
                format!("unexpected character '{ch}' at byte {i}"),
            ));
        }
        i += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
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
    evaluation_error: Option<&'static str>,
    // Short-circuited branches must still consume tokens and validate syntax.
    skip_evaluation: bool,
    ctx: &'a TemplateContext,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token>, ctx: &'a TemplateContext) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
            depth_exceeded: false,
            evaluation_error: None,
            skip_evaluation: false,
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

    fn finish(self, expr: &str, value: TemplateValue) -> Result<TemplateValue, CrepusError> {
        if self.depth_exceeded {
            return Err(CrepusError::eval(
                expr,
                "expression nesting too deep to evaluate",
            ));
        }
        if let Some(message) = self.evaluation_error {
            return Err(CrepusError::eval(expr, message));
        }
        if self.is_at_end() {
            Ok(value)
        } else {
            Err(CrepusError::eval(
                expr,
                "unexpected tokens after expression",
            ))
        }
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
            let previous_skip = self.skip_evaluation;
            self.skip_evaluation |= is_truthy(&lhs);
            let rhs = self.parse_and();
            self.skip_evaluation = previous_skip;
            lhs = TemplateValue::Bool(is_truthy(&lhs) || is_truthy(&rhs));
        }
        lhs
    }

    // and = cmp ("&&" cmp)*
    fn parse_and(&mut self) -> TemplateValue {
        let mut lhs = self.parse_cmp();
        while *self.peek() == Token::AmpAmp {
            self.advance();
            let previous_skip = self.skip_evaluation;
            self.skip_evaluation |= !is_truthy(&lhs);
            let rhs = self.parse_cmp();
            self.skip_evaluation = previous_skip;
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
        if self.skip_evaluation {
            return TemplateValue::Null;
        }
        match compare(&lhs, &rhs, op) {
            Ok(value) => TemplateValue::Bool(value),
            Err(message) => {
                self.evaluation_error = Some(message);
                TemplateValue::Null
            }
        }
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
            if self.skip_evaluation {
                lhs = TemplateValue::Null;
                continue;
            }
            lhs = match arith(&lhs, &rhs, op) {
                Ok(value) => value,
                Err(message) => {
                    self.evaluation_error = Some(message);
                    TemplateValue::Null
                }
            };
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
            if self.skip_evaluation {
                lhs = TemplateValue::Null;
                continue;
            }
            lhs = match arith(&lhs, &rhs, op) {
                Ok(value) => value,
                Err(message) => {
                    self.evaluation_error = Some(message);
                    TemplateValue::Null
                }
            };
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
            if self.skip_evaluation {
                return TemplateValue::Null;
            }
            return match v {
                TemplateValue::Int(n) => match n.checked_neg() {
                    Some(value) => TemplateValue::Int(value),
                    None => {
                        self.evaluation_error = Some("integer arithmetic overflow");
                        TemplateValue::Null
                    }
                },
                TemplateValue::Float(f) if f.is_finite() => TemplateValue::Float(-f),
                TemplateValue::Float(_) => {
                    self.evaluation_error = Some("non-finite floating-point arithmetic result");
                    TemplateValue::Null
                }
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
                if self.skip_evaluation {
                    val = Cow::Owned(TemplateValue::Null);
                    continue;
                }
                val = match val {
                    Cow::Borrowed(TemplateValue::Scope(ctx)) => ctx
                        .vars
                        .get(&prop)
                        .map_or(Cow::Owned(TemplateValue::Null), Cow::Borrowed),
                    value => Cow::Owned(match apply_property(value.as_ref(), &prop) {
                        Ok(value) => value,
                        Err(message) => {
                            self.evaluation_error = Some(message);
                            TemplateValue::Null
                        }
                    }),
                };
            } else {
                self.evaluation_error = Some("expected property name after '.'");
                break;
            }
        }
        val.into_owned()
    }

    // primary = int | float | str | bool | null | "(" expr ")" | ident
    fn parse_primary(&mut self) -> Cow<'a, TemplateValue> {
        Cow::Owned(match self.advance() {
            Token::Int(n) => TemplateValue::Int(n),
            Token::Float(f) => TemplateValue::Float(f),
            Token::Str(s) => TemplateValue::Str(s),
            Token::Bool(b) => TemplateValue::Bool(b),
            Token::Null => TemplateValue::Null,
            Token::LParen => {
                let v = self.parse_expr();
                if !self.eat(&Token::RParen) {
                    self.evaluation_error = Some("expected closing ')' in expression");
                }
                v
            }
            Token::Ident(_) if self.skip_evaluation => TemplateValue::Null,
            Token::Ident(name) => {
                return self
                    .ctx
                    .vars
                    .get(&name)
                    .map_or(Cow::Owned(TemplateValue::Null), Cow::Borrowed)
            }
            _ => {
                self.evaluation_error = Some("expected expression operand");
                TemplateValue::Null
            }
        })
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

fn compare(lhs: &TemplateValue, rhs: &TemplateValue, op: &str) -> Result<bool, &'static str> {
    if matches!(lhs, TemplateValue::List(_) | TemplateValue::Scope(_))
        || matches!(rhs, TemplateValue::List(_) | TemplateValue::Scope(_))
    {
        return match op {
            "==" => Ok(collections_equal(lhs, rhs)),
            "!=" => Ok(!collections_equal(lhs, rhs)),
            _ => Err("ordering comparisons are not supported for collections"),
        };
    }
    Ok(compare_scalar(lhs, rhs, op))
}

fn collections_equal(lhs: &TemplateValue, rhs: &TemplateValue) -> bool {
    let mut pending = vec![(lhs, rhs)];
    while let Some((lhs, rhs)) = pending.pop() {
        match (lhs, rhs) {
            (TemplateValue::Scope(lhs), TemplateValue::Scope(rhs)) => {
                if !queue_context_values(lhs, rhs, &mut pending) {
                    return false;
                }
            }
            (TemplateValue::List(lhs), TemplateValue::List(rhs)) => {
                if lhs.len() != rhs.len() {
                    return false;
                }
                for (lhs, rhs) in lhs.iter().zip(rhs) {
                    if !queue_context_values(lhs, rhs, &mut pending) {
                        return false;
                    }
                }
            }
            (TemplateValue::List(_) | TemplateValue::Scope(_), _)
            | (_, TemplateValue::List(_) | TemplateValue::Scope(_)) => return false,
            _ if !compare_scalar(lhs, rhs, "==") => return false,
            _ => {}
        }
    }
    true
}

fn queue_context_values<'a>(
    lhs: &'a TemplateContext,
    rhs: &'a TemplateContext,
    pending: &mut Vec<(&'a TemplateValue, &'a TemplateValue)>,
) -> bool {
    // Rendering resources and slot metadata are not object fields.
    if lhs.vars.len() != rhs.vars.len() {
        return false;
    }
    for (key, lhs) in &lhs.vars {
        let Some(rhs) = rhs.vars.get(key) else {
            return false;
        };
        pending.push((lhs, rhs));
    }
    true
}

fn compare_scalar(lhs: &TemplateValue, rhs: &TemplateValue, op: &str) -> bool {
    use std::cmp::Ordering;

    let ordering = match (lhs, rhs) {
        (TemplateValue::Int(a), TemplateValue::Int(b)) => Some(Some(a.cmp(b))),
        (TemplateValue::Float(a), TemplateValue::Float(b)) => Some(a.partial_cmp(b)),
        (TemplateValue::Int(a), TemplateValue::Float(b)) => Some(compare_int_float(*a, *b)),
        (TemplateValue::Float(a), TemplateValue::Int(b)) => {
            Some(compare_int_float(*b, *a).map(Ordering::reverse))
        }
        _ => None,
    };
    if let Some(ordering) = ordering {
        return match op {
            "==" => ordering == Some(Ordering::Equal),
            "!=" => ordering != Some(Ordering::Equal),
            "<" => ordering == Some(Ordering::Less),
            "<=" => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
            ">" => ordering == Some(Ordering::Greater),
            ">=" => matches!(ordering, Some(Ordering::Greater | Ordering::Equal)),
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

/// Compare mixed numbers without rounding the integer through f64.
fn compare_int_float(integer: i64, float: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if float.is_nan() {
        return None;
    }
    if float < i64::MIN as f64 {
        return Some(Ordering::Greater);
    }
    if float >= -(i64::MIN as f64) {
        return Some(Ordering::Less);
    }
    match integer.cmp(&(float as i64)) {
        Ordering::Equal => 0.0_f64.partial_cmp(&float.fract()),
        ordering => Some(ordering),
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

fn arith(
    lhs: &TemplateValue,
    rhs: &TemplateValue,
    op: &str,
) -> Result<TemplateValue, &'static str> {
    // String concatenation with +
    if op == "+" {
        if let TemplateValue::Str(l) = lhs {
            let r = crate::context::value_to_str(rhs);
            return Ok(TemplateValue::Str(format!("{}{}", l, r)));
        }
        if let TemplateValue::Str(r) = rhs {
            let l = crate::context::value_to_str(lhs);
            return Ok(TemplateValue::Str(format!("{}{}", l, r)));
        }
    }

    // Preserve exact integer arithmetic instead of rounding through f64.
    if let (TemplateValue::Int(a), TemplateValue::Int(b)) = (lhs, rhs) {
        let value = match op {
            "+" => Some(a.checked_add(*b)),
            "-" => Some(a.checked_sub(*b)),
            "*" => Some(a.checked_mul(*b)),
            "%" if *b == 0 => return Ok(TemplateValue::Null),
            "%" if *b == -1 => Some(Some(0)),
            "%" => Some(a.checked_rem(*b)),
            _ => None,
        };
        if let Some(value) = value {
            return value
                .map(TemplateValue::Int)
                .ok_or("integer arithmetic overflow");
        }
    }

    // Numeric arithmetic
    let as_nums = |l: &TemplateValue, r: &TemplateValue| -> Option<(f64, f64)> {
        match (l, r) {
            (TemplateValue::Int(a), TemplateValue::Int(b)) => Some((*a as f64, *b as f64)),
            (TemplateValue::Float(a), TemplateValue::Float(b)) => Some((*a, *b)),
            (TemplateValue::Int(a), TemplateValue::Float(b)) => Some((*a as f64, *b)),
            (TemplateValue::Float(a), TemplateValue::Int(b)) => Some((*a, *b as f64)),
            _ => None,
        }
    };

    if let Some((l, r)) = as_nums(lhs, rhs) {
        let result = match op {
            "+" => l + r,
            "-" => l - r,
            "*" => l * r,
            "/" => {
                if r == 0.0 {
                    return Ok(TemplateValue::Null);
                } else {
                    l / r
                }
            }
            "%" => {
                if r == 0.0 {
                    return Ok(TemplateValue::Null);
                } else {
                    l % r
                }
            }
            _ => return Ok(TemplateValue::Null),
        };
        if !result.is_finite() {
            return Err("non-finite floating-point arithmetic result");
        }
        return Ok(TemplateValue::Float(result));
    }

    Ok(TemplateValue::Null)
}

fn apply_property(val: &TemplateValue, prop: &str) -> Result<TemplateValue, &'static str> {
    Ok(match (val, prop) {
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
        (TemplateValue::Int(n), "abs") => {
            TemplateValue::Int(n.checked_abs().ok_or("integer arithmetic overflow")?)
        }
        (TemplateValue::Float(f), "abs") => {
            if !f.is_finite() {
                return Err("non-finite numeric property result");
            }
            TemplateValue::Float(f.abs())
        }
        (TemplateValue::Float(f), "floor" | "ceil" | "round") => {
            let rounded = match prop {
                "floor" => f.floor(),
                "ceil" => f.ceil(),
                _ => f.round(),
            };
            // i64::MAX rounds up to 2^63 in f64, so the upper bound is exclusive.
            if !rounded.is_finite() || rounded < i64::MIN as f64 || rounded >= -(i64::MIN as f64) {
                return Err("numeric property result out of integer range");
            }
            TemplateValue::Int(rounded as i64)
        }
        (TemplateValue::Scope(ctx), prop) => {
            ctx.vars.get(prop).cloned().unwrap_or(TemplateValue::Null)
        }
        _ => TemplateValue::Null,
    })
}

// ── Public API ────────────────────────────────────────────────────────────────

pub(crate) fn is_variable_name(name: &str) -> bool {
    matches!(tokenize(name).as_deref(), Ok([Token::Ident(identifier), Token::Eof]) if identifier == name)
}

/// Evaluate one literal operand without accepting names or compound expressions.
pub fn eval_literal(expr: &str) -> Option<TemplateValue> {
    if expr
        .trim_start()
        .strip_prefix('-')
        .is_some_and(|rest| rest.trim_start().starts_with('-'))
    {
        return None;
    }
    let tokens = tokenize(expr).ok()?;
    match tokens.as_slice() {
        [Token::Int(_), Token::Eof]
        | [Token::Float(_), Token::Eof]
        | [Token::Str(_), Token::Eof]
        | [Token::Bool(_), Token::Eof]
        | [Token::Null, Token::Eof] => eval_expr(expr, &TemplateContext::new()).ok(),
        [Token::Minus, Token::Int(_), Token::Eof] => eval_expr(expr, &TemplateContext::new()).ok(),
        [Token::Minus, Token::Float(_), Token::Eof] => {
            eval_expr(expr, &TemplateContext::new()).ok()
        }
        _ => None,
    }
}

/// Check DSL expression syntax without reading variables or evaluating operations.
pub fn validate_expr(expr: &str) -> Result<(), CrepusError> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Err(CrepusError::eval(expr, "expression must not be empty"));
    }
    let ctx = TemplateContext::new();
    let mut parser = Parser::new(tokenize(expr)?, &ctx);
    parser.skip_evaluation = true;
    let value = parser.parse_expr();
    parser.finish(expr, value).map(|_| ())
}

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
    let tokens = tokenize(expr)?;
    let mut parser = Parser::new(tokens, ctx);
    let value = parser.parse_expr();
    parser.finish(expr, value)
}

/// Evaluate a binding while retaining HTML attribute presence semantics.
pub fn eval_binding(
    binding: &crate::ast::Binding,
    ctx: &TemplateContext,
) -> Result<Option<TemplateValue>, CrepusError> {
    let value = eval_expr(&binding.value, ctx)?;
    if binding.origin == crate::ast::BindingOrigin::Attribute {
        return Ok(match value {
            TemplateValue::Null => None,
            value => Some(TemplateValue::Str(crate::context::value_to_str(&value))),
        });
    }
    Ok(Some(value))
}

/// Evaluate an expression as a boolean condition.
pub fn eval_condition(expr: &str, ctx: &TemplateContext) -> Result<bool, CrepusError> {
    Ok(is_truthy(&eval_expr(expr, ctx)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_operands_use_the_evaluator_without_accepting_expressions() {
        for (source, expected) in [
            ("7", "7"),
            ("-2.5", "-2.5"),
            ("1e2", "100"),
            ("'hello'", "hello"),
            (r#""line\n界""#, "line\n界"),
            ("true", "true"),
            ("false", "false"),
            ("null", ""),
            ("none", ""),
            ("None", ""),
            ("Null", ""),
        ] {
            assert_eq!(
                crate::context::value_to_str(&eval_literal(source).unwrap()),
                expected,
                "{source}"
            );
        }
        for source in [
            "",
            "name",
            "name.upper",
            "1 + 2",
            "'a' + 'b'",
            "(7)",
            "--2",
            "--0",
            "- -0.0",
            "'unterminated",
        ] {
            assert!(eval_literal(source).is_none(), "{source}");
        }
    }

    #[test]
    fn skipped_variable_operands_do_not_materialize_context_values() {
        let mut context = TemplateContext::new();
        context.set("payload", "large value".repeat(1024));
        let mut parser = Parser::new(tokenize("payload").unwrap(), &context);
        parser.skip_evaluation = true;
        assert!(matches!(parser.parse_expr(), TemplateValue::Null));
        assert!(parser.is_at_end());
        assert!(parser.evaluation_error.is_none());
        assert!(matches!(
            eval_expr("true || payload", &context).unwrap(),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            eval_expr("false && payload", &context).unwrap(),
            TemplateValue::Bool(false)
        ));
        for expression in ["true || payload.", "false && (payload", "true || payload +"] {
            assert!(eval_expr(expression, &context).is_err(), "{expression}");
        }
    }

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
        assert!(
            matches!(eval_expr("2.75", &ctx).unwrap(), TemplateValue::Float(f) if (f - 2.75).abs() < f64::EPSILON)
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
    fn nested_property_reads_preserve_owned_public_results() {
        let mut scope = TemplateContext::new();
        scope.set("label", "雪");
        scope.vars.insert(
            "items".into(),
            TemplateValue::List(vec![TemplateContext::new(); 2]),
        );
        let mut ctx = TemplateContext::new();
        ctx.vars.insert("scope".into(), TemplateValue::Scope(scope));
        for expression in ["scope.items.len", "scope.items.length"] {
            assert!(matches!(
                eval_expr(expression, &ctx).unwrap(),
                TemplateValue::Int(2)
            ));
        }
        assert!(matches!(
            eval_expr("scope.items.is_empty", &ctx).unwrap(),
            TemplateValue::Bool(false)
        ));
        assert!(matches!(
            eval_expr("scope.label.length", &ctx).unwrap(),
            TemplateValue::Int(1)
        ));
        assert!(matches!(
            eval_expr("scope.missing.length", &ctx).unwrap(),
            TemplateValue::Null
        ));
        let TemplateValue::List(mut owned) = eval_expr("scope.items", &ctx).unwrap() else {
            panic!("expected list")
        };
        owned.clear();
        assert!(matches!(
            eval_expr("scope.items.length", &ctx).unwrap(),
            TemplateValue::Int(2)
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
        assert!(is_truthy(&TemplateValue::Float(2.75)));
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
    fn unterminated_strings_are_errors() {
        // Regression: byte-indexed tokenizer must not slice on a non-char
        // boundary even when the literal runs off the end of input.
        let ctx = TemplateContext::new();
        for source in ["\"abc", "\"ñ", "'你好", "\"abc\\", "\"abc\\\""] {
            let error = eval_expr(source, &ctx).unwrap_err();
            assert!(error.to_string().contains("unterminated string"), "{error}");
        }
    }

    #[test]
    fn unsupported_expression_characters_are_errors_without_panicking() {
        let ctx = TemplateContext::new();
        for source in ["1 @ + 2", "你好", "ñ", "🙂", "true & false", "value = 1"] {
            let error = eval_expr(source, &ctx).unwrap_err();
            assert!(
                error.to_string().contains("unexpected character"),
                "{error}"
            );
        }
    }

    #[test]
    fn trailing_tokens_are_errors() {
        let ctx = TemplateContext::new();
        let err = eval_expr("1 + 2 foo", &ctx).unwrap_err();
        assert!(err.to_string().contains("unexpected tokens"));
    }

    #[test]
    fn boolean_operators_skip_unused_runtime_errors_but_check_syntax() {
        let mut ctx = TemplateContext::new();
        ctx.set("minimum", i64::MIN);
        ctx.set("maximum", i64::MAX);
        for source in [
            "true || -minimum",
            "true || maximum + 1",
            "true || minimum.abs",
            "false || (true || -minimum)",
        ] {
            assert!(eval_condition(source, &ctx).unwrap(), "{source}");
        }
        for source in [
            "false && -minimum",
            "false && maximum + 1",
            "false && minimum.abs",
            "true && (false && -minimum)",
        ] {
            assert!(!eval_condition(source, &ctx).unwrap(), "{source}");
        }
        for source in [
            "false || -minimum",
            "true && maximum + 1",
            "true || (1 +)",
            "false && name.",
        ] {
            assert!(eval_expr(source, &ctx).is_err(), "{source}");
        }
    }

    #[test]
    fn escaped_expression_quotes_decode_without_losing_unknown_escapes() {
        let ctx = TemplateContext::new();
        for (source, expected) in [
            (r#""say \"hello\"""#, "say \"hello\""),
            (r"'it\'s fine'", "it's fine"),
            (r#""雪\"}""#, "雪\"}"),
            (r#""keep\q""#, r"keep\q"),
            (r#""slash\\""#, "slash\\"),
        ] {
            assert!(
                matches!(eval_expr(source, &ctx).unwrap(), TemplateValue::Str(value) if value == expected),
                "{source}"
            );
        }
    }

    #[test]
    fn incomplete_expression_syntax_is_an_error() {
        let ctx = TemplateContext::new();
        for source in [
            "1 +", "+", "!", "-", "(1 + 2", "()", "1 + )", "name.", "name.1", "true &&",
        ] {
            assert!(eval_expr(source, &ctx).is_err(), "{source}");
        }
        assert!(matches!(eval_expr("", &ctx).unwrap(), TemplateValue::Null));
        assert!(matches!(
            eval_expr("missing", &ctx).unwrap(),
            TemplateValue::Null
        ));
        assert!(matches!(
            eval_expr("(1 + 2) * 3", &ctx).unwrap(),
            TemplateValue::Int(9)
        ));
    }

    #[test]
    fn numeric_comparisons_preserve_large_integer_distinctions() {
        let mut ctx = TemplateContext::new();
        ctx.set("maximum", i64::MAX);
        ctx.set("upper", -(i64::MIN as f64));
        ctx.set("nan", f64::NAN);
        ctx.set("infinite", f64::INFINITY);
        ctx.set("lower", i64::MIN as f64);
        ctx.set("negative_infinite", f64::NEG_INFINITY);
        ctx.set("tiny", f64::EPSILON / 2.0);
        for source in [
            "9007199254740993 != 9007199254740992",
            "9007199254740993 > 9007199254740992",
            "9007199254740993 > 9007199254740992.0",
            "9007199254740992.0 < 9007199254740993",
            "maximum < upper",
            "infinite == infinite",
            "nan != nan",
            "-9223372036854775808 == lower",
            "-9223372036854775808 > negative_infinite",
            "0 > -0.5",
            "0 < 0.5",
            "-0.5 < 0",
            "0.5 > 0",
            "tiny != 0.0",
        ] {
            assert!(eval_condition(source, &ctx).unwrap(), "{source}");
        }
        for source in [
            "9007199254740993 == 9007199254740992",
            "maximum == upper",
            "nan == nan",
            "nan < 1",
        ] {
            assert!(!eval_condition(source, &ctx).unwrap(), "{source}");
        }
    }

    #[test]
    fn numeric_properties_check_range_and_preserve_valid_results() {
        let mut ctx = TemplateContext::new();
        ctx.set("minimum", i64::MIN);
        ctx.set("lower", i64::MIN as f64);
        ctx.set("upper", -(i64::MIN as f64));
        ctx.set("infinite", f64::INFINITY);
        ctx.set("nan", f64::NAN);
        ctx.set("fraction", -1.5);
        for source in [
            "minimum.abs",
            "upper.floor",
            "infinite.ceil",
            "nan.round",
            "infinite.abs",
        ] {
            assert!(eval_expr(source, &ctx).is_err(), "{source}");
        }
        for (source, expected) in [
            ("lower.round", i64::MIN),
            ("fraction.floor", -2),
            ("fraction.ceil", -1),
            ("fraction.round", -2),
        ] {
            assert!(
                matches!(eval_expr(source, &ctx).unwrap(), TemplateValue::Int(value) if value == expected),
                "{source}"
            );
        }
        assert!(
            matches!(eval_expr("fraction.abs", &ctx).unwrap(), TemplateValue::Float(value) if value == 1.5)
        );
    }

    #[test]
    fn floating_point_arithmetic_rejects_non_finite_results() {
        let mut ctx = TemplateContext::new();
        ctx.set("large", f64::MAX);
        ctx.set("small", f64::MIN_POSITIVE);
        ctx.set("infinite", f64::INFINITY);
        ctx.set("nan", f64::NAN);
        for source in [
            "large + large",
            "large * 2",
            "large / small",
            "infinite - infinite",
            "nan % 2",
            "-infinite",
            "-nan",
        ] {
            assert!(
                eval_expr(source, &ctx)
                    .unwrap_err()
                    .to_string()
                    .contains("non-finite floating-point arithmetic"),
                "{source}"
            );
        }
        assert!(
            matches!(eval_expr("7.5 / 2", &ctx).unwrap(), TemplateValue::Float(value) if value == 3.75)
        );
        // Preserve the established divide-by-zero result.
        assert!(matches!(
            eval_expr("7.5 / 0", &ctx).unwrap(),
            TemplateValue::Null
        ));
        assert!(matches!(
            eval_expr("7.5 % 0", &ctx).unwrap(),
            TemplateValue::Null
        ));
    }

    #[test]
    fn integer_arithmetic_is_exact_and_overflow_is_an_error() {
        let mut ctx = TemplateContext::new();
        ctx.set("minimum", i64::MIN);
        for (source, expected) in [
            ("9007199254740993 + 0", 9007199254740993),
            ("9223372036854775807 - 1", i64::MAX - 1),
            ("9007199254740993 * 1", 9007199254740993),
            ("9223372036854775807 % 2", 1),
            ("minimum % -1", 0),
        ] {
            assert!(
                matches!(eval_expr(source, &ctx).unwrap(), TemplateValue::Int(value) if value == expected),
                "{source}"
            );
        }
        for source in [
            "9223372036854775807 + 1",
            "minimum - 1",
            "9223372036854775807 * 2",
            "-minimum",
        ] {
            assert!(
                eval_expr(source, &ctx)
                    .unwrap_err()
                    .to_string()
                    .contains("integer arithmetic overflow"),
                "{source}"
            );
        }
    }

    #[test]
    fn numeric_literals_preserve_boundaries_and_reject_overflow() {
        let ctx = TemplateContext::new();
        for n in [i64::MIN, i64::MAX] {
            assert!(
                matches!(eval_expr(&n.to_string(), &ctx).unwrap(), TemplateValue::Int(value) if value == n)
            );
        }
        for source in [
            "9223372036854775808",
            "-9223372036854775809",
            "9223372036854775808 + 1",
        ] {
            assert!(eval_expr(source, &ctx)
                .unwrap_err()
                .to_string()
                .contains("integer literal out of range"));
        }
        let huge_float = format!("{}.0", "9".repeat(400));
        assert!(eval_expr(&huge_float, &ctx)
            .unwrap_err()
            .to_string()
            .contains("floating-point literal out of range"));
    }

    #[test]
    fn scientific_notation_keeps_signs_and_arithmetic_boundaries() {
        let ctx = TemplateContext::new();
        for (source, expected) in [
            ("1e3", 1000.0),
            ("2.5E-2", 0.025),
            ("-3e+2", -300.0),
            ("1e2-3", 97.0),
            ("2 * 1e-2", 0.02),
        ] {
            assert!(
                matches!(eval_expr(source,&ctx).unwrap(),TemplateValue::Float(value) if value == expected),
                "{source}"
            );
        }
    }

    #[test]
    fn malformed_and_overflowing_exponents_are_errors() {
        let ctx = TemplateContext::new();
        for source in ["1e", "1e+", "1e-", "2.5Efoo", "1e309"] {
            assert!(eval_expr(source, &ctx).is_err(), "{source}");
        }
    }
    #[test]
    fn abbreviated_decimals_and_exponents_are_numeric_literals() {
        let ctx = TemplateContext::new();
        for (source, expected) in [
            (".5", 0.5),
            ("-.5", -0.5),
            ("2.", 2.0),
            (".5e2", 50.0),
            ("2.e-1", 0.2),
            ("1.+.5", 1.5),
            ("1-.5", 0.5),
        ] {
            assert!(
                matches!(eval_expr(source,&ctx).unwrap(),TemplateValue::Float(value) if value == expected),
                "{source}"
            );
        }
        for source in [".5e", "1.e+", "1.2.3"] {
            assert!(eval_expr(source, &ctx).is_err(), "{source}");
        }
    }

    #[test]
    fn decimal_scanning_preserves_numeric_property_access() {
        let ctx = TemplateContext::new();
        assert!(matches!(
            eval_expr("42.empty", &ctx).unwrap(),
            TemplateValue::Null
        ));
        for source in ["42.abs", "42. abs", "42 .abs"] {
            assert!(
                matches!(eval_expr(source, &ctx).unwrap(), TemplateValue::Int(42)),
                "{source}"
            );
        }
        assert!(
            matches!(eval_expr("(-.5).abs",&ctx).unwrap(),TemplateValue::Float(value) if value == 0.5)
        );
    }
}

#[cfg(test)]
mod variable_name_tests {
    #[test]
    fn loop_names_match_the_expression_identifier_grammar() {
        for name in ["item", "_row", "item2", "Nullish"] {
            assert!(super::is_variable_name(name), "{name}");
        }
        for name in [
            "",
            "2item",
            "item.name",
            "item other",
            "true",
            "false",
            "null",
            "none",
            "None",
            "Null",
            "é",
            "$item",
        ] {
            assert!(!super::is_variable_name(name), "{name}");
        }
    }
}
