//! Indexed reads of one view of a run's output: its segments either side of a line, its size, its gap and its node summaries and marks.

use uuid::Uuid;

use crate::run_output::models::{
    NodeLogLine, NodeOutputSummary, OutputMark, OutputView, RunOutputIndex, SearchDirection,
    StoredGap, StoredOutputTotals, StoredSegment, ViewCount,
};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

/// The view's run, its node or the run's own key, and its stream filter; a run view filters on no node.
fn view_parameters(view: &OutputView) -> Vec<Parameter> {
    vec![
        Parameter::Id(view.run_id),
        Parameter::OptionalText(view.node.clone()),
        Parameter::OptionalText(view.stream.map(|stream| stream.as_str().to_owned())),
    ]
}

/// Up to `limit` of the view's segments holding lines from `bound` onwards, or before `bound` backwards, nearest first.
pub(crate) async fn view_segments(
    transaction: ControlTransaction,
    view: &OutputView,
    (direction, bound): (SearchDirection, i64),
    limit: i64,
) -> Result<(ControlTransaction, Vec<StoredSegment>), ControlError> {
    let sql: &str = match (view.node.is_some(), direction) {
        (true, SearchDirection::Forward) => {
            "SELECT s.node_line AS first_line, s.line_count, s.node_key, s.stream, s.text_offset, s.text_bytes, \
             c.object_key, c.text_bytes AS chunk_text_bytes, COALESCE(s.read_at, c.received_at) AS read_at FROM sqb_run_output_segments s \
             JOIN sqb_run_output_chunks c ON c.run_id=s.run_id AND c.chunk=s.chunk \
             WHERE s.run_id=$1 AND s.node_key=$2 AND ($3 IS NULL OR s.stream=$3) AND s.node_line+s.line_count>$4 \
             ORDER BY s.node_line ASC LIMIT $5"
        }
        (true, SearchDirection::Backward) => {
            "SELECT s.node_line AS first_line, s.line_count, s.node_key, s.stream, s.text_offset, s.text_bytes, \
             c.object_key, c.text_bytes AS chunk_text_bytes, COALESCE(s.read_at, c.received_at) AS read_at FROM sqb_run_output_segments s \
             JOIN sqb_run_output_chunks c ON c.run_id=s.run_id AND c.chunk=s.chunk \
             WHERE s.run_id=$1 AND s.node_key=$2 AND ($3 IS NULL OR s.stream=$3) AND s.node_line<$4 \
             ORDER BY s.node_line DESC LIMIT $5"
        }
        (false, SearchDirection::Forward) => {
            "SELECT s.run_line AS first_line, s.line_count, s.node_key, s.stream, s.text_offset, s.text_bytes, \
             c.object_key, c.text_bytes AS chunk_text_bytes, COALESCE(s.read_at, c.received_at) AS read_at FROM sqb_run_output_segments s \
             JOIN sqb_run_output_chunks c ON c.run_id=s.run_id AND c.chunk=s.chunk \
             WHERE s.run_id=$1 AND $2 IS NULL AND ($3 IS NULL OR s.stream=$3) AND s.run_line+s.line_count>$4 \
             ORDER BY s.run_line ASC LIMIT $5"
        }
        (false, SearchDirection::Backward) => {
            "SELECT s.run_line AS first_line, s.line_count, s.node_key, s.stream, s.text_offset, s.text_bytes, \
             c.object_key, c.text_bytes AS chunk_text_bytes, COALESCE(s.read_at, c.received_at) AS read_at FROM sqb_run_output_segments s \
             JOIN sqb_run_output_chunks c ON c.run_id=s.run_id AND c.chunk=s.chunk \
             WHERE s.run_id=$1 AND $2 IS NULL AND ($3 IS NULL OR s.stream=$3) AND s.run_line<$4 \
             ORDER BY s.run_line DESC LIMIT $5"
        }
    };
    let mut parameters: Vec<Parameter> = view_parameters(view);
    parameters.extend([Parameter::Integer(bound), Parameter::Integer(limit)]);
    query_records(transaction, sql, parameters).await
}

/// The view's highest line number: the node's line count, or the run's.
pub(crate) async fn view_line_count(
    transaction: ControlTransaction,
    view: &OutputView,
) -> Result<(ControlTransaction, i64), ControlError> {
    let (transaction, rows): (ControlTransaction, Vec<ViewCount>) = match &view.node {
        Some(node) => {
            query_records(
                transaction,
                "SELECT CAST(COALESCE(SUM(line_count), 0) AS BIGINT) AS count FROM sqb_run_output_streams \
                 WHERE run_id=$1 AND node_key=$2",
                vec![Parameter::Id(view.run_id), Parameter::Text(node.clone())],
            )
            .await?
        }
        None => {
            query_records(
                transaction,
                "SELECT line_count AS count FROM sqb_run_output WHERE run_id=$1",
                vec![Parameter::Id(view.run_id)],
            )
            .await?
        }
    };
    Ok((transaction, rows.first().map_or(0, |row| row.count)))
}

/// How many chunks hold a node view's lines.
pub(crate) async fn view_chunk_count(
    transaction: ControlTransaction,
    view: &OutputView,
) -> Result<(ControlTransaction, i64), ControlError> {
    let (transaction, rows): (ControlTransaction, Vec<ViewCount>) = query_records(
        transaction,
        "SELECT CAST(COUNT(DISTINCT s.chunk) AS BIGINT) AS count FROM sqb_run_output_segments s \
         WHERE s.run_id=$1 AND s.node_key=$2 AND ($3 IS NULL OR s.stream=$3)",
        view_parameters(view),
    )
    .await?;
    Ok((transaction, rows.first().map_or(0, |row| row.count)))
}

/// Where a size cap left a gap in the view: its first line after the gap and how many lines of the view were left out.
pub(crate) async fn view_gap(
    transaction: ControlTransaction,
    view: &OutputView,
) -> Result<(ControlTransaction, Option<(i64, i64)>), ControlError> {
    let sql: &str = if view.node.is_some() {
        "SELECT (SELECT MIN(s.node_line) FROM sqb_run_output_segments s JOIN sqb_run_output_chunks c \
         ON c.run_id=s.run_id AND c.chunk=s.chunk WHERE s.run_id=$1 AND s.node_key=$2 \
         AND ($3 IS NULL OR s.stream=$3) AND c.part='tail') AS first_tail, \
         (SELECT MAX(s.node_line+s.line_count) FROM sqb_run_output_segments s JOIN sqb_run_output_chunks c \
         ON c.run_id=s.run_id AND c.chunk=s.chunk WHERE s.run_id=$1 AND s.node_key=$2 \
         AND ($3 IS NULL OR s.stream=$3) AND c.part='head') AS after_head, \
         (SELECT omitted_lines FROM sqb_run_output WHERE run_id=$1) AS omitted_lines"
    } else {
        "SELECT (SELECT MIN(s.run_line) FROM sqb_run_output_segments s JOIN sqb_run_output_chunks c \
         ON c.run_id=s.run_id AND c.chunk=s.chunk WHERE s.run_id=$1 AND $2 IS NULL \
         AND ($3 IS NULL OR s.stream=$3) AND c.part='tail') AS first_tail, \
         (SELECT MAX(s.run_line+s.line_count) FROM sqb_run_output_segments s JOIN sqb_run_output_chunks c \
         ON c.run_id=s.run_id AND c.chunk=s.chunk WHERE s.run_id=$1 AND $2 IS NULL \
         AND ($3 IS NULL OR s.stream=$3) AND c.part='head') AS after_head, \
         (SELECT omitted_lines FROM sqb_run_output WHERE run_id=$1) AS omitted_lines"
    };
    let (transaction, rows): (ControlTransaction, Vec<StoredGap>) =
        query_records(transaction, sql, view_parameters(view)).await?;
    let gap = rows.first().and_then(|row| gap_of(*row));
    Ok((transaction, gap))
}

/// The gap a stored gap row describes: lines left out before the view's first kept tail line.
fn gap_of(row: StoredGap) -> Option<(i64, i64)> {
    let first: i64 = row.first_tail?;
    let left_out: i64 = first - row.after_head.unwrap_or(1);
    (row.omitted_lines.unwrap_or_default() > 0 && left_out > 0).then_some((first, left_out))
}

/// Each node's line and byte counts, by node key.
async fn node_summaries(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, Vec<NodeOutputSummary>), ControlError> {
    query_records(transaction,
        "SELECT node_key, CAST(SUM(line_count) AS BIGINT) AS line_count, \
         CAST(SUM(CASE WHEN stream='stdout' THEN line_count ELSE 0 END) AS BIGINT) AS stdout_lines, \
         CAST(SUM(CASE WHEN stream='stderr' THEN line_count ELSE 0 END) AS BIGINT) AS stderr_lines, \
         CAST(SUM(text_bytes) AS BIGINT) AS text_bytes FROM sqb_run_output_streams WHERE run_id=$1 \
         GROUP BY node_key ORDER BY node_key",
        vec![Parameter::Id(run_id)]).await
}

/// One node's error and warning lines, by line.
async fn node_marks(
    transaction: ControlTransaction,
    (run_id, node): (Uuid, &str),
) -> Result<(ControlTransaction, Vec<OutputMark>), ControlError> {
    query_records(
        transaction,
        "SELECT node_line AS line, stream, level, text FROM sqb_run_output_marks \
         WHERE run_id=$1 AND node_key=$2 ORDER BY node_line",
        vec![Parameter::Id(run_id), Parameter::Text(node.into())],
    )
    .await
}

/// The run's output index, with `node`'s marks when one is named.
pub(crate) async fn output_index(
    transaction: ControlTransaction,
    (run_id, node): (Uuid, Option<&str>),
) -> Result<(ControlTransaction, RunOutputIndex), ControlError> {
    let (transaction, nodes) = node_summaries(transaction, run_id).await?;
    let (transaction, totals): (ControlTransaction, Vec<StoredOutputTotals>) = query_records(
        transaction,
        "SELECT line_count, omitted_bytes, omitted_lines, dropped_bytes FROM sqb_run_output WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let (transaction, logs): (ControlTransaction, Vec<NodeLogLine>) = query_records(transaction,
        "SELECT node_key, level, message, logged_at FROM sqb_run_node_logs WHERE run_id=$1 ORDER BY logged_at, event_id",
        vec![Parameter::Id(run_id)]).await?;
    let (transaction, marks) = match node {
        Some(node) => node_marks(transaction, (run_id, node)).await?,
        None => (transaction, Vec::new()),
    };
    let totals: StoredOutputTotals = totals.into_iter().next().unwrap_or_default();
    Ok((
        transaction,
        RunOutputIndex {
            nodes,
            line_count: totals.line_count,
            omitted_bytes: totals.omitted_bytes,
            omitted_lines: totals.omitted_lines,
            dropped_bytes: totals.dropped_bytes,
            logs,
            marks,
        },
    ))
}
