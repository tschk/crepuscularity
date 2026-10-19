//! Authored Svelte character-reference semantics (Svelte 5.57.1).
#[path = "entities_data.rs"]
mod data;
#[path = "entities_extra.rs"]
mod extra;

const WINDOWS_1252: [u32; 32] = [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 141, 381, 143, 144,
    8216, 8217, 8220, 8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 157, 382, 376,
];

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn validate(code: u32, attribute: bool) -> u32 {
    match code {
        10 if !attribute => 32,
        0..=127 => code,
        128..=159 => WINDOWS_1252[(code - 128) as usize],
        160..=55295 | 57344..=196607 | 917504..=917631 | 917760..=917999 => code,
        _ => 0,
    }
}

/// Decode authored Svelte text or attribute character references in one pass.
pub fn decode_entities(source: &str, attribute_value: bool) -> String {
    decode(source, attribute_value, false)
}

/// Decode literal HTML entities without changing the shared parser's raw newlines.
pub(crate) fn decode_html_entities(source: &str, attribute_value: bool) -> String {
    decode(source, attribute_value, true)
}

/// Static HTML text is decoded by the browser, rather than Svelte's dynamic-text decoder.
pub fn decode_static_dom(source: &str) -> String {
    decode(
        &source.replace("\r\n", "\n").replace('\r', "\n"),
        false,
        true,
    )
}

/// Decode an authored HTML attribute without interpreting JavaScript string values.
pub fn decode_static_attribute(source: &str) -> String {
    decode(
        &source.replace("\r\n", "\n").replace('\r', "\n"),
        true,
        true,
    )
}

fn decode(source: &str, attribute_value: bool, html: bool) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('&') {
        let at = cursor + relative;
        output.push_str(&source[cursor..at]);
        let tail = &source[at + 1..];
        let matched = if let Some(number) = tail.strip_prefix('#') {
            let (digits, radix, prefix) = if number.starts_with(['x', 'X']) {
                (&number[1..], 16u32, 2)
            } else {
                (number, 10u32, 1)
            };
            let count = digits
                .bytes()
                .take_while(|b| {
                    if radix == 16 {
                        b.is_ascii_hexdigit()
                    } else {
                        b.is_ascii_digit()
                    }
                })
                .count();
            if count == 0 {
                None
            } else {
                let code = digits[..count].bytes().try_fold(0u32, |value, digit| {
                    value
                        .checked_mul(radix)?
                        .checked_add((digit as char).to_digit(radix)?)
                });
                let end = prefix + count;
                Some((
                    end + usize::from(tail.as_bytes().get(end) == Some(&b';')),
                    code,
                    0,
                ))
            }
        } else {
            // Table order is the official regexp alternative order, including
            // longer spellings before their legacy semicolonless prefixes.
            data::ENTITIES.iter().find_map(|&(name, code)| {
                if !tail.starts_with(name) {
                    return None;
                }
                let next = tail.as_bytes().get(name.len()).copied();
                if attribute_value
                    && !name.ends_with(';')
                    && next.is_some_and(|b| {
                        (if html {
                            b.is_ascii_alphanumeric()
                        } else {
                            word(b)
                        }) || b == b'='
                    })
                {
                    return None;
                }
                let second = if html {
                    extra::SECOND_SCALARS
                        .iter()
                        .find_map(|&(key, value)| (key == name).then_some(value))
                        .unwrap_or(0)
                } else {
                    0
                };
                Some((name.len(), Some(code), second))
            })
        };
        if let Some((length, code, second)) = matched {
            if code == Some(0) && !html {
                output.push_str(&source[at..at + 1 + length]);
            } else {
                let code = code.unwrap_or(u32::MAX);
                let validated = if html {
                    match code {
                        0 | 0xD800..=0xDFFF | 0x110000..=u32::MAX => 0xFFFD,
                        128..=159 => WINDOWS_1252[(code - 128) as usize],
                        _ => code,
                    }
                } else {
                    validate(code, attribute_value)
                };
                output.push(char::from_u32(validated).unwrap_or('\0'));
                if second != 0 {
                    output.push(char::from_u32(second).expect("entity scalar"));
                }
            }
            cursor = at + 1 + length;
        } else {
            output.push('&');
            cursor = at + 1;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::decode_entities as decode;

    #[test]
    fn static_html_uses_full_browser_entities_and_source_newline_normalization() {
        use super::decode_static_dom as html;
        assert_eq!(
            html("&NewLine;&NotEqualTilde;&ampx &amp;copy;"),
            "\n≂\u{338}&x &copy;"
        );
        assert_eq!(html("a\r\nb\rc&#13;"), "a\nb\nc\r");
        assert_eq!(
            html("&#0;&#xD800;&#x110000;&#x10FFFF;&#128;"),
            "���\u{10FFFF}€"
        );
        assert_eq!(html("&#999999999999999999999999; &unknown;"), "� &unknown;");
    }

    #[test]
    fn named_references_keep_official_scalar_table_and_single_pass() {
        assert_eq!(
            decode("界&NewLine;&Tab;&NotEqualTilde;&unknown;", false),
            "界 \t≂&unknown;"
        );
        assert_eq!(
            decode("&NewLine;&amp;copy; &amp;amp;", true),
            "\n&copy; &amp;"
        );
        assert_eq!(decode("&ampx &amp= &amp_ &amp!", false), "&x &= &_ &!");
        assert_eq!(
            decode("&ampx &amp= &amp_ &amp! &amp界", true),
            "&ampx &amp= &amp_ &! &界"
        );
    }

    #[test]
    fn numeric_references_follow_svelte_validation_and_optional_semicolons() {
        assert_eq!(
            decode("&#X41 &#65; &#0; &#x0 &#128;", false),
            "A A &#0; &#x0 €"
        );
        assert_eq!(
            decode("&#10;&#13;&#xD800;&#x10FFFF;&#x1F600;", false),
            " \r\0\0😀"
        );
        assert_eq!(decode("&#10;&#13;", true), "\n\r");
        assert_eq!(
            decode(
                "&#99999999999999999999999; &#000000000000000000000; &#x;",
                false
            ),
            "\0 &#000000000000000000000; &#x;"
        );
    }
}
