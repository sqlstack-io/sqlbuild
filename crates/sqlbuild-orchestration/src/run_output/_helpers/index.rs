//! The control store's index of a run's output: chunks, segments, stream totals and what a cap dropped.

use uuid::Uuid;

use crate::run_output::constants::{HEAD_PART, MARKED_LINES, TAIL_PART};
use crate::run_output::models::{
    PreparedChunk, PreparedSegment, StoredNodeLines, StoredTotals, TailChunk,
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

/// Index one chunk, numbering each segment's lines after the run's last and after its node's last; past `half` of a capped run's bytes the oldest later chunks go, and their object keys come back for deletion.
pub(crate) async fn record_chunk(
    transaction: ControlTransaction,
    chunk: &PreparedChunk,
    half: Option<i64>,
) -> Result<(ControlTransaction, Vec<String>), ControlError> {
    let (mut transaction, totals) = totals(transaction, chunk.run_id).await?;
    let StoredTotals {
        next_chunk: number,
        line_count: mut run_lines,
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
        transaction =
            record_segment(transaction, (chunk.run_id, number, run_lines + 1), segment).await?;
        run_lines += segment.line_count;
    }
    (transaction, _) = query(
        transaction,
        "UPDATE sqb_run_output SET line_count=$2 WHERE run_id=$1",
        vec![Parameter::Id(chunk.run_id), Parameter::Integer(run_lines)],
    )
    .await?;
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
        "INSERT INTO sqb_run_output (run_id, next_chunk, line_count, head_bytes, tail_bytes, stored_bytes, omitted_bytes, omitted_lines, dropped_bytes) \
         VALUES ($1,0,0,0,0,0,0,0,0) ON CONFLICT (run_id) DO NOTHING",
        vec![Parameter::Id(run_id)]).await?;
    let (transaction, rows): (ControlTransaction, Vec<StoredTotals>) = query_records(
        transaction,
        "SELECT next_chunk, line_count, head_bytes, tail_bytes FROM sqb_run_output WHERE run_id=$1",
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
    (run_id, chunk, run_line): (Uuid, i64, i64),
    segment: &PreparedSegment,
) -> Result<ControlTransaction, ControlError> {
    let stream: &str = segment.stream.as_str();
    let node: Parameter = Parameter::Text(segment.node_key.clone());
    let (transaction, rows): (ControlTransaction, Vec<StoredNodeLines>) = query_records(transaction,
        "SELECT CAST(COALESCE(SUM(line_count), 0) AS BIGINT) AS node_lines, \
         CAST(COALESCE(SUM(CASE WHEN stream=$3 THEN mark_count ELSE 0 END), 0) AS BIGINT) AS stream_marks \
         FROM sqb_run_output_streams WHERE run_id=$1 AND node_key=$2",
        vec![Parameter::Id(run_id), node.clone(), Parameter::Text(stream.into())]).await?;
    let before: StoredNodeLines = rows.into_iter().next().unwrap_or_default();
    let node_line: i64 = before.node_lines + 1;
    let (mut transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output_segments (run_id, chunk, node_key, stream, run_line, node_line, line_count, text_offset, text_bytes) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        vec![Parameter::Id(run_id), Parameter::Integer(chunk), node.clone(), Parameter::Text(stream.into()),
            Parameter::Integer(run_line), Parameter::Integer(node_line), Parameter::Integer(segment.line_count),
            Parameter::Integer(segment.text_offset), Parameter::Integer(segment.text_bytes)]).await?;
    let room: usize =
        MARKED_LINES.saturating_sub(usize::try_from(before.stream_marks).unwrap_or(MARKED_LINES));
    let marks = &segment.marks[..segment.marks.len().min(room)];
    for (offset, level, text) in marks {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_run_output_marks (run_id, node_key, node_line, stream, level, text) VALUES ($1,$2,$3,$4,$5,$6)",
            vec![Parameter::Id(run_id), node.clone(), Parameter::Integer(node_line + offset),
                Parameter::Text(stream.into()), Parameter::Text(level.as_str().into()), Parameter::Text(text.clone())]).await?;
    }
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_output_streams (run_id, node_key, stream, line_count, text_bytes, mark_count) \
         VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (run_id, node_key, stream) DO UPDATE SET \
         line_count=sqb_run_output_streams.line_count+$4, text_bytes=sqb_run_output_streams.text_bytes+$5, \
         mark_count=sqb_run_output_streams.mark_count+$6",
        vec![Parameter::Id(run_id), node, Parameter::Text(stream.into()),
            Parameter::Integer(segment.line_count), Parameter::Integer(segment.text_bytes),
            Parameter::Integer(i64::try_from(marks.len()).unwrap_or(i64::MAX))]).await?;
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
            "DELETE FROM sqb_run_output_marks WHERE run_id=$1 AND EXISTS (SELECT 1 FROM sqb_run_output_segments s \
             WHERE s.run_id=$1 AND s.chunk=$2 AND s.node_key=sqb_run_output_marks.node_key \
             AND sqb_run_output_marks.node_line>=s.node_line AND sqb_run_output_marks.node_line<s.node_line+s.line_count)",
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

fn integer(value: u64) -> Result<i64, ControlError> {
    i64::try_from(value)
        .map_err(|_| ControlError::Invalid("run output counter is out of range".into()))
}
