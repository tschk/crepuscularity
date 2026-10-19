//! Retained, single-line editing state. Offsets in this model are UTF-8 byte
//! boundaries; the platform-facing methods explicitly convert UTF-16 ranges.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    anchor: usize,
    cursor: usize,
}

/// An editor transaction model independent of GPUI's platform/window objects.
#[derive(Clone, Debug)]
pub struct InputModel {
    state: Snapshot,
    marked: Option<Range<usize>>,
    composition_start: Option<Snapshot>,
    last_external: String,
    pending_external: Option<String>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    disabled: bool,
    readonly: bool,
}

pub(crate) fn single_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

impl InputModel {
    pub fn new(text: impl Into<String>) -> Self {
        let text = single_line(&text.into());
        Self {
            state: Snapshot {
                text: text.clone(),
                anchor: 0,
                cursor: 0,
            },
            marked: None,
            composition_start: None,
            last_external: text,
            pending_external: None,
            undo: Vec::new(),
            redo: Vec::new(),
            disabled: false,
            readonly: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.state.text
    }
    pub fn cursor(&self) -> usize {
        self.state.cursor
    }
    pub fn anchor(&self) -> usize {
        self.state.anchor
    }
    pub fn selection(&self) -> Range<usize> {
        self.state.anchor.min(self.state.cursor)..self.state.anchor.max(self.state.cursor)
    }
    pub fn selection_utf16(&self) -> Range<usize> {
        self.to_utf16(self.selection())
    }
    pub fn reversed(&self) -> bool {
        self.state.cursor < self.state.anchor
    }
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }
    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked.clone().map(|r| self.to_utf16(r))
    }
    pub fn is_composing(&self) -> bool {
        self.composition_start.is_some()
    }
    pub fn disabled(&self) -> bool {
        self.disabled
    }
    pub fn readonly(&self) -> bool {
        self.readonly
    }
    pub fn accepts_input(&self) -> bool {
        !self.disabled && !self.readonly
    }

    pub fn set_permissions(&mut self, disabled: bool, readonly: bool) {
        if (disabled || readonly) && self.is_composing() {
            self.commit_composition();
        }
        self.disabled = disabled;
        self.readonly = readonly;
    }

    /// Repeated parent renders with the same bound value do not reset editing.
    /// A different value is authoritative, but deferred until composition ends.
    /// A parent echoing the current draft acknowledges it without cancelling IME.
    pub fn sync_external(&mut self, value: &str) -> bool {
        let value = single_line(value);
        if value == self.last_external {
            return false;
        }
        self.last_external = value.clone();
        if value == self.state.text {
            self.pending_external = None;
            return false;
        }
        if self.is_composing() {
            self.pending_external = Some(value);
            false
        } else {
            self.apply_external(value);
            true
        }
    }

    fn apply_external(&mut self, text: String) {
        self.state.anchor = floor_boundary(&text, self.state.anchor);
        self.state.cursor = floor_boundary(&text, self.state.cursor);
        self.state.text = text;
        self.marked = None;
        self.composition_start = None;
        self.undo.clear();
        self.redo.clear();
    }

    fn flush_external(&mut self) {
        if let Some(text) = self.pending_external.take() {
            self.apply_external(text);
        }
    }

    pub fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        byte_to_utf16(self.text(), range.start)..byte_to_utf16(self.text(), range.end)
    }

    pub fn text_for_utf16_range(&self, range: Range<usize>) -> (String, Range<usize>) {
        let range = range_from_utf16(self.text(), range);
        (self.text()[range.clone()].to_owned(), self.to_utf16(range))
    }

    pub fn select_utf16(&mut self, range: Range<usize>) {
        if self.disabled {
            return;
        }
        let reversed = range.start > range.end;
        let range = range_from_utf16(self.text(), range);
        self.state.anchor = if reversed { range.end } else { range.start };
        self.state.cursor = if reversed { range.start } else { range.end };
    }

    pub fn select_bytes(&mut self, anchor: usize, cursor: usize) {
        if self.disabled {
            return;
        }
        self.state.anchor = floor_boundary(self.text(), anchor);
        self.state.cursor = floor_boundary(self.text(), cursor);
    }

    pub fn select_all(&mut self) {
        self.select_bytes(0, self.text().len());
    }

    fn remember(&mut self, before: Snapshot) {
        if before.text == self.state.text {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn replacement_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        range
            .map(|r| range_from_utf16(self.text(), r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection())
    }

    /// A platform insertion commits any active composition as a single undo step.
    pub fn replace_utf16(&mut self, range: Option<Range<usize>>, text: &str) {
        if !self.accepts_input() {
            return;
        }
        let before = self
            .composition_start
            .take()
            .unwrap_or_else(|| self.state.clone());
        let range = self.replacement_range(range);
        let text = single_line(text);
        self.state.text.replace_range(range.clone(), &text);
        self.state.cursor = range.start + text.len();
        self.state.anchor = self.state.cursor;
        self.marked = None;
        self.remember(before);
        self.flush_external();
    }

    /// `selected` is relative to the new marked text, in UTF-16 code units.
    pub fn replace_and_mark_utf16(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        if !self.accepts_input() {
            return;
        }
        if self.composition_start.is_none() {
            self.composition_start = Some(self.state.clone());
        }
        let range = self.replacement_range(range);
        let text = single_line(text);
        let selection = selected
            .clone()
            .map(|r| range_from_utf16(&text, r))
            .unwrap_or(text.len()..text.len());
        let reversed = selected.is_some_and(|r| r.start > r.end);
        self.state.text.replace_range(range.clone(), &text);
        self.marked = Some(range.start..range.start + text.len());
        self.state.anchor = range.start
            + if reversed {
                selection.end
            } else {
                selection.start
            };
        self.state.cursor = range.start
            + if reversed {
                selection.start
            } else {
                selection.end
            };
    }

    pub fn commit_composition(&mut self) {
        self.marked = None;
        if let Some(before) = self.composition_start.take() {
            self.remember(before);
        }
        self.flush_external();
    }

    /// Explicit Escape/cancel restores both the pre-composition text and selection.
    /// Platform `unmark_text` commits instead; the two operations are not aliases.
    pub fn cancel_composition(&mut self) {
        if let Some(before) = self.composition_start.take() {
            self.state = before;
        }
        self.marked = None;
        self.flush_external();
    }

    pub fn move_to(&mut self, cursor: usize, extend: bool) {
        if self.disabled {
            return;
        }
        self.commit_composition();
        self.state.cursor = floor_boundary(self.text(), cursor);
        if !extend {
            self.state.anchor = self.state.cursor;
        }
    }

    pub fn move_grapheme(&mut self, forward: bool, extend: bool) {
        let selected = self.selection();
        let next = if !extend && !selected.is_empty() {
            if forward {
                selected.end
            } else {
                selected.start
            }
        } else if forward {
            next_grapheme(self.text(), self.cursor())
        } else {
            previous_grapheme(self.text(), self.cursor())
        };
        self.move_to(next, extend);
    }

    pub fn move_word(&mut self, forward: bool, extend: bool) {
        let cursor = self.cursor();
        let next = if forward {
            self.text()
                .unicode_word_indices()
                .map(|(i, w)| i + w.len())
                .find(|i| *i > cursor)
                .unwrap_or(self.text().len())
        } else {
            self.text()
                .unicode_word_indices()
                .map(|(i, _)| i)
                .take_while(|i| *i < cursor)
                .last()
                .unwrap_or(0)
        };
        self.move_to(next, extend);
    }

    pub fn delete(&mut self, forward: bool) {
        if !self.accepts_input() {
            return;
        }
        self.commit_composition();
        let mut range = self.selection();
        if range.is_empty() {
            // Platform/IME selections may end at a scalar boundary inside an
            // extended grapheme. A collapsed delete still consumes that whole
            // grapheme; explicit nonempty selections retain their exact range.
            range = if let Some((start, cluster)) =
                self.text().grapheme_indices(true).find(|(start, cluster)| {
                    *start < range.start && range.start < *start + cluster.len()
                }) {
                start..start + cluster.len()
            } else if forward {
                range.start..next_grapheme(self.text(), range.end)
            } else {
                previous_grapheme(self.text(), range.start)..range.end
            };
        }
        self.replace_utf16(Some(self.to_utf16(range)), "");
    }

    pub fn undo(&mut self) {
        if !self.accepts_input() {
            return;
        }
        if self.is_composing() {
            self.cancel_composition();
            return;
        }
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, previous));
        }
    }

    pub fn redo(&mut self) {
        if !self.accepts_input() {
            return;
        }
        self.commit_composition();
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, next));
        }
    }
}

pub(crate) fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(crate) fn byte_to_utf16(text: &str, offset: usize) -> usize {
    text[..floor_boundary(text, offset)].encode_utf16().count()
}

fn utf16_to_byte(text: &str, offset: usize, round_up: bool) -> usize {
    let mut count = 0;
    for (index, ch) in text.char_indices() {
        if count == offset {
            return index;
        }
        count += ch.len_utf16();
        if count > offset {
            return if round_up {
                index + ch.len_utf8()
            } else {
                index
            };
        }
    }
    text.len()
}

pub(crate) fn range_from_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    let start = range.start.min(range.end);
    let end = range.start.max(range.end);
    let start_byte = utf16_to_byte(text, start, false);
    // An insertion in a surrogate pair snaps backward; it never deletes a scalar.
    if start == end {
        return start_byte..start_byte;
    }
    start_byte..utf16_to_byte(text, end, true)
}

fn previous_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|i| *i < offset)
        .last()
        .unwrap_or(0)
}

fn next_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .find(|i| *i > offset)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrogate_ranges_expand_and_insertions_snap_without_corrupting_utf8() {
        let mut input = InputModel::new("a😀b");
        assert_eq!(input.text_for_utf16_range(2..3), ("😀".into(), 1..3));
        input.replace_utf16(Some(2..2), "x");
        assert_eq!(input.text(), "ax😀b");
        input.replace_utf16(Some(100..200), "!");
        assert_eq!(input.text(), "ax😀b!");
    }

    #[test]
    fn reversed_selection_and_grapheme_deletion_preserve_unicode() {
        let mut input = InputModel::new("e\u{301}👨‍👩‍👧‍👦🇭🇰");
        input.move_to(input.text().len(), false);
        input.delete(false);
        assert_eq!(input.text(), "e\u{301}👨‍👩‍👧‍👦");
        input.delete(false);
        assert_eq!(input.text(), "e\u{301}");
        input.select_utf16(2..0);
        assert!(input.reversed());
        input.replace_utf16(None, "好");
        assert_eq!(input.text(), "好");
        input.undo();
        assert_eq!(input.text(), "e\u{301}");
        assert!(input.reversed());
    }

    #[test]
    fn composition_replaces_marked_range_and_is_one_undo_transaction() {
        let mut input = InputModel::new("hello");
        input.select_utf16(0..5);
        input.replace_and_mark_utf16(None, "n", Some(1..1));
        input.replace_and_mark_utf16(None, "ni", Some(2..2));
        input.replace_utf16(None, "你");
        assert_eq!(input.text(), "你");
        assert_eq!(input.marked(), None);
        input.undo();
        assert_eq!(input.text(), "hello");
        assert_eq!(input.selection_utf16(), 0..5);
        input.redo();
        assert_eq!(input.text(), "你");
    }

    #[test]
    fn collapsed_deletion_inside_a_grapheme_removes_the_whole_cluster() {
        for forward in [false, true] {
            for text in ["e\u{301}x", "👨‍👩‍👧‍👦x"] {
                let mut input = InputModel::new(text);
                input.select_bytes(
                    text.chars().next().unwrap().len_utf8(),
                    text.chars().next().unwrap().len_utf8(),
                );
                input.delete(forward);
                assert_eq!(input.text(), "x");
                input.undo();
                assert_eq!(input.text(), text);
            }
        }
    }

    #[test]
    fn cancellation_differs_from_unmark_and_external_updates_are_deferred() {
        let mut input = InputModel::new("old");
        input.select_all();
        input.replace_and_mark_utf16(None, "draft", Some(5..5));
        input.sync_external("old"); // Unchanged render must not reset IME.
        assert_eq!(input.text(), "draft");
        input.cancel_composition();
        assert_eq!(input.text(), "old");
        input.replace_and_mark_utf16(None, "新", Some(1..1));
        input.sync_external("server");
        assert_eq!(input.text(), "新");
        input.commit_composition();
        assert_eq!(input.text(), "server");
        assert_eq!(input.marked(), None);
    }

    #[test]
    fn echoed_drafts_do_not_reset_composition_and_readonly_blocks_mutation() {
        let mut input = InputModel::new("");
        input.replace_and_mark_utf16(None, "拼", Some(1..1));
        input.sync_external("拼");
        assert!(input.is_composing());
        input.set_permissions(false, true);
        assert!(!input.is_composing());
        let before = input.text().to_owned();
        input.replace_utf16(None, "blocked");
        input.delete(false);
        input.undo();
        assert_eq!(input.text(), before);
        input.select_all(); // Readonly can still select/copy.
        input.set_permissions(true, false);
        let selected = input.selection();
        input.select_bytes(0, 0);
        assert_eq!(input.selection(), selected);
    }
}
