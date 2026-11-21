//! The control store's index of a run's output: chunks, segments, stream totals and what a cap dropped.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::run_output::_helpers::chunks::appended;
use crate::run_output::constants::{HEAD_PART, TAIL_PART};
use crate::run_output::models::{
    PreparedChunk, PreparedSegment, StoredSegment, StoredStream, StoredTotals, TailChunk,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{Change, ControlTransaction, Parameter};

/// Whether this claim already stored `batch`.
pub(crate) async fn batch_stored(
    transaction: ControlTransaction,
    (run_id, claim_id): (Uuid, Uuid),
    batch: u64,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT chunk FROM sqb_run_output_chunks WHERE run_id=$1 AND claim_id=$2 AND batch=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(claim_id),
            Parameter::Integer(integer(batch)?),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}

/// Index one chunk, numbering each segment's lines after its stream's last; past `half` of a capped run's bytes the oldest later chunks go, and their object keys come back for deletion.
pub(crate) async fn record_chunk(
    transaction: ControlTransaction,
    chunk: &PreparedChunk,
    half: Option<i64>,
) -> Result<(ControlTransaction, Vec<String>), ControlError> {
    let (mut transaction, totals) = totals(transaction, chunk.run_id).await?;
    let StoredTotals {
        next_chunk: number,
        head_bytes: head,
        tail_bytes: tail,
    } = totals;
    let part: &str = match half {
        Some(half) if tail > 0 || head + chunk.text_bytes > half => TAIL_PART,
        _ => HEAD_PART,
    };
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output_chunks (run_id, chunk, claim_id, batch, object_key, text_bytes, part) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![Parameter::Id(chunk.run_id), Parameter::Integer(number), Parameter::Id(chunk.claim_id),
            Parameter::Integer(integer(chunk.batch)?), Parameter::Text(chunk.object_key.clone()),
            Parameter::Integer(chunk.text_bytes), Parameter::Text(part.into())]).await?;
    (transaction, _) = query(transaction,
        "UPDATE sqb_run_output SET next_chunk=next_chunk+1, head_bytes=head_bytes+$2, tail_bytes=tail_bytes+$3, \
         stored_bytes=stored_bytes+$4, dropped_bytes=dropped_bytes+$5 WHERE run_id=$1",
        vec![Parameter::Id(chunk.run_id),
            Parameter::Integer(if part == HEAD_PART { chunk.text_bytes } else { 0 }),
            Parameter::Integer(if part == TAIL_PART { chunk.text_bytes } else { 0 }),
            Parameter::Integer(chunk.stored_bytes), Parameter::Integer(chunk.dropped_bytes)]).await?;
    for segment in &chunk.segments {
        transaction = record_segment(transaction, (chunk.run_id, number), segment).await?;
    }
    transaction.changes.insert(Change::Run(chunk.run_id));
    match half {
        Some(half) if part == TAIL_PART => {
            drop_oldest_tail(transaction, (chunk.run_id, number), half).await
        }
        _ => Ok((transaction, Vec::new())),
    }
}

/// Start the run's totals once, and read them.
async fn totals(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, StoredTotals), ControlError> {
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output (run_id, next_chunk, head_bytes, tail_bytes, stored_bytes, omitted_bytes, omitted_lines, dropped_bytes) \
         VALUES ($1,0,0,0,0,0,0,0) ON CONFLICT (run_id) DO NOTHING",
        vec![Parameter::Id(run_id)]).await?;
    let (transaction, rows): (ControlTransaction, Vec<StoredTotals>) = query_records(
        transaction,
        "SELECT next_chunk, head_bytes, tail_bytes FROM sqb_run_output WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let found: StoredTotals = rows
        .into_iter()
        .next()
        .ok_or_else(|| ControlError::Invalid("run output totals are missing".into()))?;
    Ok((transaction, found))
}

async fn record_segment(
    transaction: ControlTransaction,
    (run_id, chunk): (Uuid, i64),
    segment: &PreparedSegment,
) -> Result<ControlTransaction, ControlError> {
    let stream: &str = segment.stream.as_str();
    let (transaction, rows): (ControlTransaction, Vec<StoredStream>) = query_records(transaction,
        "SELECT node_key, stream, line_count, text_bytes, error_lines, warning_lines FROM sqb_run_output_streams \
         WHERE run_id=$1 AND node_key=$2 AND stream=$3",
        vec![Parameter::Id(run_id), Parameter::Text(segment.node_key.clone()), Parameter::Text(stream.into())]).await?;
    let existing: Option<StoredStream> = rows.into_iter().next();
    let before: i64 = existing.as_ref().map_or(0, |found| found.line_count);
    let first_line: i64 = before + 1;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output_segments (run_id, chunk, node_key, stream, first_line, line_count, text_offset, text_bytes) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        vec![Parameter::Id(run_id), Parameter::Integer(chunk), Parameter::Text(segment.node_key.clone()),
            Parameter::Text(stream.into()), Parameter::Integer(first_line), Parameter::Integer(segment.line_count),
            Parameter::Integer(segment.text_offset), Parameter::Integer(segment.text_bytes)]).await?;
    let shifted = |offsets: &[i64]| -> Vec<i64> {
        offsets.iter().map(|offset| offset + first_line).collect()
    };
    let (errors, warnings): (Vec<i64>, Vec<i64>) = match &existing {
        Some(found) => (
            appended(
                numbers(&found.error_lines)?,
                shifted(&segment.error_offsets),
            ),
            appended(
                numbers(&found.warning_lines)?,
                shifted(&segment.warning_offsets),
            ),
        ),
        None => (
            appended(Vec::new(), shifted(&segment.error_offsets)),
            appended(Vec::new(), shifted(&segment.warning_offsets)),
        ),
    };
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output_streams (run_id, node_key, stream, line_count, text_bytes, error_lines, warning_lines) \
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (run_id, node_key, stream) DO UPDATE SET line_count=excluded.line_count, \
         text_bytes=sqb_run_output_streams.text_bytes+$8, error_lines=excluded.error_lines, warning_lines=excluded.warning_lines",
        vec![Parameter::Id(run_id), Parameter::Text(segment.node_key.clone()), Parameter::Text(stream.into()),
            Parameter::Integer(before + segment.line_count), Parameter::Integer(segment.text_bytes),
            Parameter::Json(json!(errors)), Parameter::Json(json!(warnings)), Parameter::Integer(segment.text_bytes)]).await?;
    Ok(transaction)
}

/// Drop the oldest later chunks, never the newest, until the run's last part fits in `half`.
async fn drop_oldest_tail(
    mut transaction: ControlTransaction,
    (run_id, newest): (Uuid, i64),
    half: i64,
) -> Result<(ControlTransaction, Vec<String>), ControlError> {
    let mut dropped: Vec<String> = Vec::new();
    loop {
        let rows: Vec<TailChunk>;
        (transaction, rows) = query_records(transaction,
            "SELECT c.chunk, c.text_bytes, c.object_key, CAST(COALESCE((SELECT SUM(s.line_count) FROM sqb_run_output_segments s \
             WHERE s.run_id=c.run_id AND s.chunk=c.chunk), 0) AS BIGINT) AS lines FROM sqb_run_output_chunks c JOIN sqb_run_output o ON o.run_id=c.run_id \
             WHERE c.run_id=$1 AND c.part='tail' AND c.chunk<$2 AND o.tail_bytes>$3 ORDER BY c.chunk LIMIT 1",
            vec![Parameter::Id(run_id), Parameter::Integer(newest), Parameter::Integer(half)]).await?;
        let Some(TailChunk {
            chunk,
            text_bytes: bytes,
            object_key: key,
            lines,
        }) = rows.into_iter().next()
        else {
            return Ok((transaction, dropped));
        };
        for sql in [
            "DELETE FROM sqb_run_output_segments WHERE run_id=$1 AND chunk=$2",
            "DELETE FROM sqb_run_output_chunks WHERE run_id=$1 AND chunk=$2",
        ] {
            (transaction, _) = query(
                transaction,
                sql,
                vec![Parameter::Id(run_id), Parameter::Integer(chunk)],
            )
            .await?;
        }
        (transaction, _) = query(transaction,
            "UPDATE sqb_run_output SET tail_bytes=tail_bytes-$2, omitted_bytes=omitted_bytes+$2, omitted_lines=omitted_lines+$3 WHERE run_id=$1",
            vec![Parameter::Id(run_id), Parameter::Integer(bytes), Parameter::Integer(lines)]).await?;
        dropped.push(key);
    }
}

/// The stored segments of one stream that hold lines `from` to `to`, in order.
pub(crate) async fn segments_between(
    transaction: ControlTransaction,
    (run_id, node_key, stream): (Uuid, &str, &str),
    (from, to): (i64, i64),
) -> Result<(ControlTransaction, Vec<StoredSegment>), ControlError> {
    query_records(transaction,
        "SELECT s.first_line, s.text_offset, s.text_bytes, c.object_key, c.text_bytes AS chunk_text_bytes \
         FROM sqb_run_output_segments s JOIN sqb_run_output_chunks c ON c.run_id=s.run_id AND c.chunk=s.chunk \
         WHERE s.run_id=$1 AND s.node_key=$2 AND s.stream=$3 AND s.first_line<=$5 AND s.first_line+s.line_count>$4 \
         ORDER BY s.first_line",
        vec![Parameter::Id(run_id), Parameter::Text(node_key.into()), Parameter::Text(stream.into()),
            Parameter::Integer(from), Parameter::Integer(to)]).await
}

/// One stream's totals, if it printed anything.
pub(crate) async fn stored_stream(
    transaction: ControlTransaction,
    (run_id, node_key, stream): (Uuid, &str, &str),
) -> Result<(ControlTransaction, Option<StoredStream>), ControlError> {
    let (transaction, rows): (ControlTransaction, Vec<StoredStream>) = query_records(transaction,
        "SELECT node_key, stream, line_count, text_bytes, error_lines, warning_lines FROM sqb_run_output_streams \
         WHERE run_id=$1 AND node_key=$2 AND stream=$3",
        vec![Parameter::Id(run_id), Parameter::Text(node_key.into()), Parameter::Text(stream.into())]).await?;
    Ok((transaction, rows.into_iter().next()))
}

/// Line numbers kept as a JSON array, which SQLite returns as text.
pub(crate) fn numbers(value: &Value) -> Result<Vec<i64>, ControlError> {
    let parsed: Value = match value {
        Value::String(text) => {
            serde_json::from_str(text).map_err(|error| ControlError::Invalid(error.to_string()))?
        }
        other => other.clone(),
    };
    serde_json::from_value(parsed).map_err(|error| ControlError::Invalid(error.to_string()))
}

fn integer(value: u64) -> Result<i64, ControlError> {
    i64::try_from(value)
        .map_err(|_| ControlError::Invalid("run output counter is out of range".into()))
}
