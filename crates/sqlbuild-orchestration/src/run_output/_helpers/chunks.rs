//! A chunk's text: its lines, which of them look like errors or warnings, and its zstd form.

use crate::run_output::constants::{MARKED_LINES, ZSTD_LEVEL};
use crate::store::errors::ControlError;

const ERROR_WORDS: [&str; 4] = ["error", "exception", "traceback", "fatal"];
const WARNING_WORDS: [&str; 1] = ["warn"];

/// How many lines `text` holds; every line ends in a newline.
pub(crate) fn line_count(text: &str) -> i64 {
    i64::try_from(bytecount(text)).unwrap_or(i64::MAX)
}

fn bytecount(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count()
}

/// The lines of `text`, without their newlines.
pub(crate) fn lines(text: &str) -> Vec<&str> {
    text.split_terminator('\n').collect()
}

/// The numbers of `text`'s lines that look like errors and like warnings, `text` starting at `first_line`; each list stops at `room`.
pub(crate) fn marked_lines(
    text: &str,
    first_line: i64,
    room: (usize, usize),
) -> (Vec<i64>, Vec<i64>) {
    let (mut errors, mut warnings): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
    for (number, line) in (first_line..).zip(lines(text)) {
        let lower: String = line.to_lowercase();
        if ERROR_WORDS.iter().any(|word| lower.contains(word)) {
            if errors.len() < room.0 {
                errors.push(number);
            }
        } else if WARNING_WORDS.iter().any(|word| lower.contains(word)) && warnings.len() < room.1 {
            warnings.push(number);
        }
    }
    (errors, warnings)
}

/// `kept` then `added`, at most `MARKED_LINES` numbers.
pub(crate) fn appended(kept: Vec<i64>, added: Vec<i64>) -> Vec<i64> {
    let mut all: Vec<i64> = kept;
    all.extend(added);
    all.truncate(MARKED_LINES);
    all
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
