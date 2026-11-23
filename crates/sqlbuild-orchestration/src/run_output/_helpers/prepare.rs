//! A worker batch checked and laid out as one chunk: its text, zstd form and each segment's place.

use uuid::Uuid;

use crate::run_output::_helpers::chunks::{compress, line_count, marked_lines};
use crate::run_output::constants::{
    MARKED_LINES, MAX_BATCH_SEGMENTS, MAX_BATCH_TEXT_BYTES, RUN_OUTPUT_NODE,
};
use crate::run_output::models::{OutputBatch, PreparedChunk, PreparedSegment};
use crate::store::errors::ControlError;

/// Refuse a batch past its limits, a segment that is empty or ends mid-line, and a node key that is not `<kind>:<name>`.
pub(crate) fn checked(batch: &OutputBatch) -> Result<(), ControlError> {
    let bytes: usize = batch
        .segments
        .iter()
        .map(|segment| segment.text.len())
        .sum();
    if batch.segments.len() > MAX_BATCH_SEGMENTS || bytes > MAX_BATCH_TEXT_BYTES {
        return Err(ControlError::Invalid(
            "run output batch exceeds its limit".into(),
        ));
    }
    for segment in &batch.segments {
        if !segment.text.ends_with('\n') {
            return Err(ControlError::Invalid(
                "run output segments hold complete lines".into(),
            ));
        }
        if segment.node_key.as_deref().is_some_and(|key| {
            !key.split_once(':')
                .is_some_and(|(kind, name)| !kind.is_empty() && !name.is_empty())
        }) {
            return Err(ControlError::Invalid(
                "run output node keys are <kind>:<name>".into(),
            ));
        }
    }
    Ok(())
}

/// The chunk of `batch` and its zstd form, written under `object_key`.
pub(crate) fn prepared(
    (run_id, claim_id): (Uuid, Uuid),
    batch: &OutputBatch,
    object_key: String,
) -> Result<(PreparedChunk, Vec<u8>), ControlError> {
    let mut text: String = String::new();
    let mut segments: Vec<PreparedSegment> = Vec::with_capacity(batch.segments.len());
    let mut dropped: u64 = 0;
    for segment in &batch.segments {
        let marks = marked_lines(&segment.text, MARKED_LINES);
        segments.push(PreparedSegment {
            node_key: segment
                .node_key
                .clone()
                .unwrap_or_else(|| RUN_OUTPUT_NODE.into()),
            stream: segment.stream,
            line_count: line_count(&segment.text),
            text_offset: counter(text.len())?,
            text_bytes: counter(segment.text.len())?,
            marks,
        });
        dropped = dropped.saturating_add(segment.dropped_bytes);
        text.push_str(&segment.text);
    }
    let stored: Vec<u8> = compress(text.as_bytes())?;
    Ok((
        PreparedChunk {
            run_id,
            claim_id,
            batch: batch.batch,
            object_key,
            text_bytes: counter(text.len())?,
            stored_bytes: counter(stored.len())?,
            dropped_bytes: i64::try_from(dropped).unwrap_or(i64::MAX),
            segments,
        },
        stored,
    ))
}

fn counter(value: usize) -> Result<i64, ControlError> {
    i64::try_from(value)
        .map_err(|_| ControlError::Invalid("run output counter is out of range".into()))
}
