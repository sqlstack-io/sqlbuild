//! One page of a node's stream by line number, or its last lines; each read needs view.evidence and is audited.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::index::{segments_between, stored_stream};
use crate::run_output::_helpers::pages::{numbered_lines, segment_texts};
use crate::run_output::constants::{DEFAULT_PAGE_LINES, MAX_PAGE_LINES};
use crate::run_output::models::{OutputPage, OutputPageQuery, OutputStore, StoredSegment};
use crate::run_output::types::PersonRunRead;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target; a stream that printed nothing is an empty page.
pub async fn read_person_output_page(
    store: &ControlStore,
    output: &OutputStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    query: &OutputPageQuery,
) -> Result<Option<OutputPage>, ControlError> {
    let run_id = run.2;
    let limit: i64 = i64::from(
        query
            .limit
            .unwrap_or(DEFAULT_PAGE_LINES)
            .clamp(1, MAX_PAGE_LINES),
    );
    let stream: &str = query.stream.as_str();
    let node: &str = &query.node;
    let Some((line_count, (from, to), segments)) = read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.page",
            async move |transaction, _| {
                let (transaction, found) = stored_stream(transaction, (run_id, node, stream)).await?;
                let line_count: i64 = found.map_or(0, |found| found.line_count);
                let from: i64 = if query.tail {
                    (line_count - limit + 1).max(1)
                } else {
                    query.from.unwrap_or(1).max(1)
                };
                let to: i64 = from + limit - 1;
                let (transaction, segments): (_, Vec<StoredSegment>) =
                    segments_between(transaction, (run_id, node, stream), (from, to)).await?;
                Ok((transaction, (line_count, (from, to), segments)))
            },
            |(_, (from, to), _): &(i64, (i64, i64), Vec<StoredSegment>)| json!({"node": node, "stream": stream, "from": from, "to": to}),
        ),
    )
    .await?
    else {
        return Ok(None);
    };
    let texts: Vec<String> = segment_texts(output, &segments).await?;
    Ok(Some(OutputPage {
        lines: numbered_lines(&segments, &texts, (from, to)),
        line_count,
        next: (to < line_count).then_some(to + 1),
        previous: (from > 1).then_some((from - limit).max(1)),
    }))
}
