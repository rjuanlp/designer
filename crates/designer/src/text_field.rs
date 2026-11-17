//! Window-independent single-line editing with UTF-16 platform ranges.

use std::ops::Range;

/// Editable text and a reversible editing session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditState {
    text: String,
    original: String,
    anchor: usize,
    caret: usize,
    marked: Option<Range<usize>>,
}

impl EditState {
    /// Begin editing with the caret at the end. Offsets are UTF-8 internally.
    pub fn new(text: String) -> Self {
        let caret = text.len();
        Self {
            original: text.clone(),
            text,
            anchor: caret,
            caret,
            marked: None,
        }
    }

    /// Current text, including any in-progress composition.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Selection in UTF-8 bytes, always on character boundaries.
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    /// Selection in the UTF-16 offsets used by GPUI's input handler.
    pub fn selection_utf16(&self) -> Range<usize> {
        let range = self.selection();
        self.text[..range.start].encode_utf16().count()
            ..self.text[..range.end].encode_utf16().count()
    }

    /// Marked composition range in UTF-16 offsets.
    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| {
            self.text[..range.start].encode_utf16().count()
                ..self.text[..range.end].encode_utf16().count()
        })
    }

    fn byte_offset(&self, offset: usize) -> usize {
        let mut units = 0;
        for (byte, ch) in self.text.char_indices() {
            if units + ch.len_utf16() > offset {
                return byte;
            }
            units += ch.len_utf16();
        }
        self.text.len()
    }

    /// Replace a platform range, or the composition/selection when absent.
    /// Newlines are ignored because this is a single-line field.
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str, composing: bool) {
        let range = range
            .map(|range| {
                let a = self.byte_offset(range.start);
                let b = self.byte_offset(range.end);
                a.min(b)..a.max(b)
            })
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        let text: String = text.chars().filter(|ch| !ch.is_control()).collect();
        self.text.replace_range(range.clone(), &text);
        self.caret = range.start + text.len();
        self.anchor = self.caret;
        self.marked = composing.then_some(range.start..self.caret);
    }

    /// End composition without changing its text.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Select all current text.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
        self.unmark();
    }

    /// Move by one Unicode scalar, optionally extending the selection.
    pub fn move_caret(&mut self, right: bool, extend: bool) {
        self.unmark();
        let selection = self.selection();
        self.caret = if !extend && !selection.is_empty() {
            if right {
                selection.end
            } else {
                selection.start
            }
        } else if right {
            self.text[self.caret..]
                .chars()
                .next()
                .map_or(self.caret, |ch| self.caret + ch.len_utf8())
        } else {
            self.text[..self.caret]
                .char_indices()
                .next_back()
                .map_or(0, |(byte, _)| byte)
        };
        if !extend {
            self.anchor = self.caret;
        }
    }

    /// Delete the selection or the adjacent character.
    pub fn delete(&mut self, forward: bool) {
        if self.selection().is_empty() {
            self.move_caret(forward, true);
        }
        self.unmark();
        self.replace(None, "", false);
    }

    /// Accept the current value as the next cancellation baseline.
    pub fn commit(&mut self) -> String {
        self.unmark();
        self.original = self.text.clone();
        self.text.clone()
    }

    /// Restore the most recently committed value.
    pub fn cancel(&mut self) {
        *self = Self::new(self.original.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_keys_replace_marked_text_and_cancel_restores_baseline() {
        let mut state = EditState::new("Espa".into());
        state.replace(None, "~", true);
        assert_eq!(state.marked_utf16(), Some(4..5));
        state.replace(None, "ñ", false);
        state.replace(None, "a", false);
        assert_eq!(state.text(), "España");
        assert_eq!(state.commit(), "España");
        state.select_all();
        state.replace(None, "otro", false);
        state.cancel();
        assert_eq!(state.text(), "España");
    }

    #[test]
    fn movement_selection_and_deletion_preserve_unicode_boundaries() {
        let mut state = EditState::new("á😀z".into());
        state.move_caret(false, false);
        state.move_caret(false, true);
        assert_eq!(state.selection_utf16(), 1..3);
        state.delete(false);
        assert_eq!(state.text(), "áz");
        state.delete(true);
        state.delete(false);
        state.delete(false);
        assert_eq!(state.text(), "");
    }

    #[test]
    fn platform_ranges_are_clamped_without_splitting_surrogate_pairs() {
        let mut state = EditState::new("a😀b".into());
        state.replace(Some(2..usize::MAX), "é\r\n", true);
        assert_eq!(state.text(), "aé");
        assert_eq!(state.marked_utf16(), Some(1..2));
        state.replace(None, "è", false);
        assert_eq!(state.text(), "aè");
        assert_eq!(state.marked_utf16(), None);
    }
}
