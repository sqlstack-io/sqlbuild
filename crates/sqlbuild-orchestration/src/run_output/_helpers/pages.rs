//! Numbered lines of stored segments, read from their chunks.

use crate::run_output::_helpers::chunks::lines;
use crate::run_output::_helpers::objects::read_chunk;
use crate::run_output::models::{OutputLine, OutputSearch, OutputStore, StoredSegment};
use crate::store::errors::ControlError;

/// The text of one segment within its chunk's text.
pub(crate) fn segment_text<'a>(
    chunk: &'a str,
    segment: &StoredSegment,
) -> Result<&'a str, ControlError> {
    let start: usize = usize::try_from(segment.text_offset).map_err(|_| invalid())?;
    let end: usize = start + usize::try_from(segment.text_bytes).map_err(|_| invalid())?;
    chunk.get(start..end).ok_or_else(invalid)
}

/// The text of each segment in order, reading each chunk once.
pub(crate) async fn segment_texts(
    output: &OutputStore,
    segments: &[StoredSegment],
) -> Result<Vec<String>, ControlError> {
    let mut texts: Vec<String> = Vec::with_capacity(segments.len());
    for segment in segments {
        let capacity: usize = usize::try_from(segment.chunk_text_bytes).map_err(|_| invalid())?;
        let chunk: String = read_chunk(output, &segment.object_key, capacity).await?;
        texts.push(segment_text(&chunk, segment)?.to_owned());
    }
    Ok(texts)
}

/// The lines numbered `from` to `to`, each counting the lines a size cap left out just before it.
pub(crate) fn numbered_lines(
    segments: &[StoredSegment],
    texts: &[String],
    (from, to): (i64, i64),
) -> Vec<OutputLine> {
    let mut found: Vec<OutputLine> = Vec::new();
    let mut expected: i64 = from;
    for (segment, text) in segments.iter().zip(texts) {
        for (number, line) in (segment.first_line..).zip(lines(text)) {
            if number < from || number > to {
                continue;
            }
            found.push(OutputLine {
                number,
                text: line.to_owned(),
                omitted_before: number - expected,
            });
            expected = number + 1;
        }
    }
    found
}

fn invalid() -> ControlError {
    ControlError::Invalid("run output index points outside its chunk".into())
}

/// The lines holding `needle`, lowercase, up to `limit`; a search that stops early says after which line to continue.
pub(crate) fn matching_lines(
    lines: Vec<OutputLine>,
    (needle, limit): (&str, usize),
    (after, more): (i64, bool),
) -> OutputSearch {
    let mut matches: Vec<OutputLine> = Vec::new();
    let mut scanned: i64 = after;
    for line in lines {
        scanned = line.number;
        if line.text.to_lowercase().contains(needle) {
            matches.push(OutputLine {
                omitted_before: 0,
                ..line
            });
            if matches.len() == limit {
                return OutputSearch {
                    matches,
                    continue_after: Some(scanned),
                };
            }
        }
    }
    OutputSearch {
        matches,
        continue_after: more.then_some(scanned),
    }
}
