//! Backend-agnostic Tailwind length tokens.
//!
//! The GPUI-backed targets (`crepuscularity-runtime`'s styler and the `view!`
//! proc macro) both need `w-1/2`, `p-4`, `max-w-[32rem]` and friends lowered to
//! GPUI length types. Only the *parse* half lives here; each target lowers a
//! [`LengthToken`] to its own representation.

/// A Tailwind length token, resolved but not yet lowered to a backend type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthToken {
    /// `auto`.
    Auto,
    /// A proportion of the parent: `full`, `screen`, `1/2`, `[50%]`.
    Fraction(f32),
    /// Absolute pixels: `px`, `[12px]`, `[12]`.
    Px(f32),
    /// Root-relative ems: `[1.5rem]`, and the numeric spacing scale (`4` → `1rem`).
    Rems(f32),
}

/// Parse a full Tailwind length: keywords, fractions, the numeric scale, or an
/// arbitrary `[...]` value.
pub fn parse_length_token(value: &str) -> Option<LengthToken> {
    if let Some(inner) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return parse_arbitrary_length_token(inner);
    }
    match value {
        "full" | "screen" => return Some(LengthToken::Fraction(1.0)),
        "auto" => return Some(LengthToken::Auto),
        "px" => return Some(LengthToken::Px(1.0)),
        _ => {}
    }
    if let Some((num, den)) = value.split_once('/') {
        let n = parse_finite(num)?;
        let d = parse_finite(den)?;
        if d == 0.0 {
            return None;
        }
        let fraction = n / d;
        return fraction
            .is_finite()
            .then_some(LengthToken::Fraction(fraction));
    }
    // Same multipliers as every other target; 4 px = 0.25 rem at the 16 px root.
    super::parse::parse_spacing_pt(value).map(|px| LengthToken::Rems(px / 16.0))
}

/// Parse the inside of an arbitrary `[...]` value: `12px`, `1.5rem`, `50%`, `12`.
pub fn parse_arbitrary_length_token(inner: &str) -> Option<LengthToken> {
    if let Some(rest) = inner.strip_suffix("px") {
        return parse_finite(rest).map(LengthToken::Px);
    }
    if let Some(rest) = inner.strip_suffix("rem") {
        return parse_finite(rest).map(LengthToken::Rems);
    }
    if let Some(rest) = inner.strip_suffix('%') {
        return parse_finite(rest).map(|n| LengthToken::Fraction(n / 100.0));
    }
    parse_finite(inner).map(LengthToken::Px)
}

fn parse_finite(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonfinite_values_and_fraction_overflow() {
        for value in [
            "NaN",
            "inf",
            "-inf",
            "1e100",
            "NaN/2",
            "1/inf",
            "3e38/1e-38",
        ] {
            assert_eq!(parse_length_token(value), None, "{value}");
        }
        for value in ["NaNpx", "infrem", "-inf%", "1e100px", "NaN"] {
            assert_eq!(parse_arbitrary_length_token(value), None, "{value}");
            assert_eq!(parse_length_token(&format!("[{value}]")), None);
        }
        assert_eq!(parse_length_token("-2"), None);
        assert_eq!(
            parse_arbitrary_length_token("-12px"),
            Some(LengthToken::Px(-12.0))
        );
    }

    #[test]
    fn keywords() {
        assert_eq!(parse_length_token("auto"), Some(LengthToken::Auto));
        assert_eq!(parse_length_token("full"), Some(LengthToken::Fraction(1.0)));
        assert_eq!(
            parse_length_token("screen"),
            Some(LengthToken::Fraction(1.0))
        );
        assert_eq!(parse_length_token("px"), Some(LengthToken::Px(1.0)));
    }

    #[test]
    fn numeric_scale_is_rem_based() {
        assert_eq!(parse_length_token("4"), Some(LengthToken::Rems(1.0)));
        assert_eq!(parse_length_token("0"), Some(LengthToken::Rems(0.0)));
        assert_eq!(parse_length_token("0.5"), Some(LengthToken::Rems(0.125)));
        assert_eq!(parse_length_token("4.5"), Some(LengthToken::Rems(1.125)));
        assert_eq!(parse_length_token("0.25"), Some(LengthToken::Rems(0.0625)));
        for value in ["0.3", "1.50", "01", "1e1"] {
            assert_eq!(parse_length_token(value), None, "{value}");
        }
    }

    #[test]
    fn fractions() {
        assert_eq!(parse_length_token("1/2"), Some(LengthToken::Fraction(0.5)));
        assert_eq!(parse_length_token("3/4"), Some(LengthToken::Fraction(0.75)));
        assert_eq!(parse_length_token("1/0"), None);
        assert_eq!(parse_length_token("a/2"), None);
    }

    #[test]
    fn arbitrary() {
        assert_eq!(parse_length_token("[12px]"), Some(LengthToken::Px(12.0)));
        assert_eq!(parse_length_token("[1.5rem]"), Some(LengthToken::Rems(1.5)));
        assert_eq!(
            parse_length_token("[50%]"),
            Some(LengthToken::Fraction(0.5))
        );
        assert_eq!(parse_length_token("[12]"), Some(LengthToken::Px(12.0)));
        assert_eq!(parse_length_token("[abc]"), None);
    }

    #[test]
    fn rejects_unknown_units() {
        assert_eq!(parse_arbitrary_length_token("10em"), None);
        assert_eq!(parse_arbitrary_length_token("10vh"), None);
        assert_eq!(parse_arbitrary_length_token("px"), None);
        assert_eq!(parse_arbitrary_length_token("rem"), None);
        assert_eq!(parse_arbitrary_length_token("10.5.5px"), None);
        assert_eq!(parse_arbitrary_length_token("10pxrem"), None);
        assert_eq!(parse_arbitrary_length_token(""), None);
    }
}
