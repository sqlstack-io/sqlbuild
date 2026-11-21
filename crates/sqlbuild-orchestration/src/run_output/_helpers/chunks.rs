//! A chunk's text: its lines, which of them look like errors or warnings, how lines fold, and its zstd form.

use crate::run_output::constants::{MARK_TEXT_BYTES, ZSTD_LEVEL};
use crate::run_output::models::OutputLevel;
use crate::store::errors::ControlError;

const ERROR_WORDS: [&str; 4] = ["error", "exception", "traceback", "fatal"];
const WARNING_WORDS: [&str; 1] = ["warn"];

/// How many lines `text` holds; every line ends in a newline.
pub(crate) fn line_count(text: &str) -> i64 {
    i64::try_from(text.bytes().filter(|byte| *byte == b'\n').count()).unwrap_or(i64::MAX)
}

/// The lines of `text`, without their newlines.
pub(crate) fn lines(text: &str) -> Vec<&str> {
    text.split_terminator('\n').collect()
}

/// Whether `line` reads as an error or a warning; error words win.
pub(crate) fn level_of(line: &str) -> Option<OutputLevel> {
    let lower: String = line.to_lowercase();
    if ERROR_WORDS.iter().any(|word| lower.contains(word)) {
        Some(OutputLevel::Error)
    } else if WARNING_WORDS.iter().any(|word| lower.contains(word)) {
        Some(OutputLevel::Warning)
    } else {
        None
    }
}

/// The first `room` of `text`'s lines that read as errors or warnings: each one's offset within `text`, level and text.
pub(crate) fn marked_lines(text: &str, room: usize) -> Vec<(i64, OutputLevel, String)> {
    let mut marks: Vec<(i64, OutputLevel, String)> = Vec::new();
    for (offset, line) in (0_i64..).zip(lines(text)) {
        if marks.len() == room {
            break;
        }
        if let Some(level) = level_of(line) {
            marks.push((offset, level, mark_text(line)));
        }
    }
    marks
}

/// `line` cut to a mark's limit at a character boundary.
fn mark_text(line: &str) -> String {
    let end: usize = (0..=line.len().min(MARK_TEXT_BYTES))
        .rev()
        .find(|index| line.is_char_boundary(*index))
        .unwrap_or(0);
    line[..end].to_owned()
}

/// What lines that are the same but for their numbers share: the line with each run of digits replaced by one `#`.
pub(crate) fn similar_key(line: &str) -> String {
    let mut key: String = String::with_capacity(line.len());
    let mut in_digits: bool = false;
    for character in line.chars() {
        if character.is_ascii_digit() {
            if !in_digits {
                key.push('#');
            }
            in_digits = true;
        } else {
            key.push(character);
            in_digits = false;
        }
    }
    key
}

pub(crate) fn compress(text: &[u8]) -> Result<Vec<u8>, ControlError> {
    Ok(zstd::bulk::compress(text, ZSTD_LEVEL)?)
}

/// The text of a chunk, which is at most `capacity` bytes once decompressed.
pub(crate) fn decompress(stored: &[u8], capacity: usize) -> Result<String, ControlError> {
    let bytes: Vec<u8> = zstd::bulk::decompress(stored, capacity)?;
    String::from_utf8(bytes)
        .map_err(|error| ControlError::Invalid(format!("run output chunk: {error}")))
}

/// Whether a count is zero, so a line with nothing left out before it serializes without the field.
pub(crate) const fn is_zero(value: &i64) -> bool {
    *value == 0
}
