//! Header keys around a string: whole-word key matches and enclosing `meta` values.

use crate::header_strings::constants::{DESCRIPTION_KEY, META_KEY, TRIPLE_QUOTE_LENGTH};
use crate::header_strings::errors::TripleQuoteError;
use crate::header_strings::main::opens_triple_quote::opens_triple_quote;
use crate::header_strings::main::prose_end::prose_end;
use crate::header_strings::main::triple_quoted_end::triple_quoted_end;
use crate::header_strings::models::DescriptionKeys;

/// Whether `unit` can be part of a header key.
fn is_key_unit<T: Into<u32>>(unit: T) -> bool {
    char::from_u32(unit.into())
        .is_some_and(|character| character.is_alphanumeric() || character == '_')
}

/// Whether `unit` is whitespace between a header key and its value.
pub(crate) fn is_header_space<T: Copy + PartialEq + From<u8>>(unit: T) -> bool {
    b" \t\r\n".iter().any(|byte| unit == T::from(*byte))
}

/// Whether `text` holds `key`, in any letter case, as a whole word ending at `end`.
pub(crate) fn key_ends_at<T: Copy + PartialEq + From<u8> + Into<u32>>(
    text: &[T],
    end: usize,
    key: &str,
) -> bool {
    let Some(start) = end.checked_sub(key.len()) else {
        return false;
    };
    let matches_key: bool = key.bytes().enumerate().all(|(offset, byte)| {
        let unit: T = text[start + offset];
        unit == T::from(byte) || unit == T::from(byte.to_ascii_uppercase())
    });
    matches_key && (start == 0 || !is_key_unit(text[start - 1]))
}

impl<'a, T: Copy + PartialEq + From<u8> + Into<u32>> DescriptionKeys<'a, T> {
    /// The `description` keys of `text`, none read yet.
    #[must_use]
    pub fn new(text: &'a [T]) -> Self {
        Self {
            text,
            read_to: 0,
            open_parens: Vec::new(),
            open_meta: 0,
        }
    }

    /// Whether the string at `start` directly follows a `description` key, in any letter case.
    pub fn follows(&mut self, start: usize) -> bool {
        let text: &[T] = self.text;
        let mut key_end: usize = start;
        while key_end > 0 && is_header_space(text[key_end - 1]) {
            key_end -= 1;
        }
        key_end < start
            && key_ends_at(text, key_end, DESCRIPTION_KEY)
            && !self.inside_meta(key_end - DESCRIPTION_KEY.len())
    }

    /// The offset just past the header string opening at `start`, or `None` when it never closes.
    pub fn quoted_end(&mut self, start: usize) -> Option<usize> {
        let text: &[T] = self.text;
        if self.follows(start)
            && let Ok(end) = prose_end(text, start)
        {
            return Some(end);
        }
        if opens_triple_quote(text, start) {
            return match triple_quoted_end(text, start) {
                Ok((_, end)) => Some(end),
                Err(TripleQuoteError::Inside(index)) => Some(index + TRIPLE_QUOTE_LENGTH),
                Err(TripleQuoteError::Unclosed) => None,
            };
        }
        let quote: T = text[start];
        let mut index: usize = start + 1;
        while index < text.len() {
            if text[index] == T::from(b'\\') {
                index += 2;
                continue;
            }
            if text[index] == quote {
                return Some(index + 1);
            }
            index += 1;
        }
        None
    }

    /// Whether a key ending before `end` is inside `meta (...)`; offsets read in order scan once.
    pub(crate) fn inside_meta(&mut self, end: usize) -> bool {
        if end < self.read_to {
            self.read_to = 0;
            self.open_parens.clear();
            self.open_meta = 0;
        }
        for index in self.read_to..end {
            let unit: T = self.text[index];
            if unit == T::from(b'(') {
                let mut key_end: usize = index;
                while key_end > 0 && is_header_space(self.text[key_end - 1]) {
                    key_end -= 1;
                }
                let meta: bool = key_ends_at(self.text, key_end, META_KEY);
                self.open_meta += usize::from(meta);
                self.open_parens.push(meta);
            } else if unit == T::from(b')')
                && let Some(meta) = self.open_parens.pop()
            {
                self.open_meta -= usize::from(meta);
            }
        }
        self.read_to = end;
        self.open_meta > 0
    }
}
