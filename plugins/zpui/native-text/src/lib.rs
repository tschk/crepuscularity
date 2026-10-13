//! Checked UTF-8/UTF-16 and UAX #29 grapheme helpers for the Zig editor.
use std::{slice, str};
use unicode_segmentation::UnicodeSegmentation;
const INVALID: usize = usize::MAX;

/// Input points to `len` readable bytes and remains borrowed during the call.
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    if ptr.is_null() || len > isize::MAX as usize {
        return None;
    }
    str::from_utf8(unsafe { slice::from_raw_parts(ptr, len) }).ok()
}

/// Return the previous/next extended grapheme boundary, or usize::MAX for invalid UTF-8.
/// # Safety
/// `ptr` must reference `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_grapheme(
    ptr: *const u8,
    len: usize,
    index: usize,
    forward: bool,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    if index > len || !s.is_char_boundary(index) {
        return INVALID;
    }
    if forward {
        s.grapheme_indices(true)
            .map(|(i, g)| i + g.len())
            .find(|&i| i > index)
            .unwrap_or(len)
    } else {
        s.grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|&i| i < index)
            .last()
            .unwrap_or(0)
    }
}

/// Translate UTF-16 offsets, rounding a split surrogate outward as requested.
/// # Safety
/// `ptr` must reference `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_byte_offset(
    ptr: *const u8,
    len: usize,
    index: usize,
    round_up: bool,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    let mut units = 0;
    for (i, ch) in s.char_indices() {
        if units >= index {
            return i;
        }
        units += ch.len_utf16();
        if units > index {
            return if round_up { i + ch.len_utf8() } else { i };
        }
    }
    len
}

/// Translate a UTF-8 boundary to a UTF-16 offset.
/// # Safety
/// `ptr` must reference `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_utf16_offset(
    ptr: *const u8,
    len: usize,
    index: usize,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    if index > len || !s.is_char_boundary(index) {
        return INVALID;
    }
    s[..index].encode_utf16().count()
}

/// Unicode default upper/lower casing. Query required length with out=null, cap=0.
/// # Safety
/// Input must reference `len` readable bytes. A non-null `out` must reference `cap`
/// writable bytes and not overlap input. Only `mode` 0 (upper) or 1 (lower) is accepted.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_case(
    ptr: *const u8,
    len: usize,
    mode: u8,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    let mapped = match mode {
        0 => s.to_uppercase(),
        1 => s.to_lowercase(),
        _ => return INVALID,
    };
    if !out.is_null() && cap >= mapped.len() {
        unsafe {
            std::ptr::copy_nonoverlapping(mapped.as_ptr(), out, mapped.len());
        }
    }
    mapped.len()
}

/// Mask extended graphemes in one pass, preserving multiline line breaks.
/// # Safety
/// Input references `len` readable bytes; non-null output references `cap`
/// writable bytes and does not overlap input. Query length with null output.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_mask(
    ptr: *const u8,
    len: usize,
    multiline: bool,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    let mut masked = String::new();
    for g in s.graphemes(true) {
        masked.push(if multiline && matches!(g, "\n" | "\r" | "\r\n") {
            '\n'
        } else {
            '•'
        });
    }
    if !out.is_null() && cap >= masked.len() {
        unsafe {
            std::ptr::copy_nonoverlapping(masked.as_ptr(), out, masked.len());
        }
    }
    masked.len()
}

/// Map between secure-display and model byte offsets, rounding into a cluster down.
/// # Safety
/// `ptr` references `len` readable bytes. `reverse` maps display to model.
#[no_mangle]
pub unsafe extern "C" fn crepus_text_mask_offset(
    ptr: *const u8,
    len: usize,
    index: usize,
    multiline: bool,
    reverse: bool,
) -> usize {
    let Some(s) = (unsafe { text(ptr, len) }) else {
        return INVALID;
    };
    let mut masked = 0;
    for (byte, g) in s.grapheme_indices(true) {
        if index <= if reverse { masked } else { byte } {
            return if reverse { byte } else { masked };
        }
        let width = if multiline && matches!(g, "\n" | "\r" | "\r\n") {
            1
        } else {
            "•".len()
        };
        if index
            < if reverse {
                masked + width
            } else {
                byte + g.len()
            }
        {
            return if reverse { byte } else { masked };
        }
        masked += width;
    }
    if reverse {
        len
    } else {
        masked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletes_one_extended_grapheme() {
        for value in ["e\u{301}", "👩‍👩‍👧‍👦", "🇦🇺", "👍🏽", "क्‍ष", "\r\n"]
        {
            unsafe {
                assert_eq!(
                    crepus_text_grapheme(value.as_ptr(), value.len(), value.len(), false),
                    0,
                    "{value}"
                );
                assert_eq!(
                    crepus_text_grapheme(value.as_ptr(), value.len(), 0, true),
                    value.len()
                );
            }
        }
    }
    #[test]
    fn utf16_round_trip_and_surrogate_splits() {
        let s = "a😀e\u{301}";
        unsafe {
            assert_eq!(crepus_text_byte_offset(s.as_ptr(), s.len(), 2, false), 1);
            assert_eq!(crepus_text_byte_offset(s.as_ptr(), s.len(), 2, true), 5);
            assert_eq!(crepus_text_utf16_offset(s.as_ptr(), s.len(), 5), 3);
            assert_eq!(crepus_text_byte_offset(s.as_ptr(), s.len(), 3, false), 5);
        }
    }
    #[test]
    fn rejects_invalid_input_and_non_boundaries() {
        unsafe {
            assert_eq!(crepus_text_grapheme([0xff].as_ptr(), 1, 0, true), INVALID);
            assert_eq!(crepus_text_utf16_offset("😀".as_ptr(), 4, 1), INVALID);
            assert_eq!(
                crepus_text_byte_offset(std::ptr::null(), 1, 0, false),
                INVALID
            );
        }
    }
    #[test]
    fn secure_mask_preserves_lines_and_maps_cluster_offsets() {
        let s = "a😀e\u{301}\r\nB";
        unsafe {
            let n = crepus_text_mask(s.as_ptr(), s.len(), true, std::ptr::null_mut(), 0);
            let mut out = vec![0; n];
            assert_eq!(
                crepus_text_mask(s.as_ptr(), s.len(), true, out.as_mut_ptr(), n),
                n
            );
            assert_eq!(str::from_utf8(&out).unwrap(), "•••\n•");
            assert_eq!(
                crepus_text_mask_offset(s.as_ptr(), s.len(), 5, true, false),
                6
            );
            assert_eq!(
                crepus_text_mask_offset(s.as_ptr(), s.len(), 10, true, true),
                10
            );
            assert_eq!(
                crepus_text_mask_offset(s.as_ptr(), s.len(), 4, true, true),
                1
            );
        }
    }
    #[test]
    fn unicode_casing_expands_and_handles_final_sigma() {
        for (s, mode, expected) in [("Straße", 0, "STRASSE"), ("ΟΣ", 1, "ος")] {
            unsafe {
                let n = crepus_text_case(s.as_ptr(), s.len(), mode, std::ptr::null_mut(), 0);
                let mut out = vec![0; n];
                assert_eq!(
                    crepus_text_case(s.as_ptr(), s.len(), mode, out.as_mut_ptr(), n),
                    n
                );
                assert_eq!(str::from_utf8(&out).unwrap(), expected);
            }
        }
    }
}
