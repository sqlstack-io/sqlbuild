//! Search a node's stream for text, case-insensitively, a bounded number of chunks at a time; each search is audited.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::index::segments_between;
use crate::run_output::_helpers::pages::{matching_lines, numbered_lines, segment_texts};
use crate::run_output::constants::{MAX_SEARCH_MATCHES, SEARCH_CHUNKS};
use crate::run_output::models::{OutputSearch, OutputSearchQuery, OutputStore, StoredSegment};
use crate::run_output::types::PersonRunRead;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target; a search that stops early says after which line to continue.
pub async fn search_person_output(
    store: &ControlStore,
    output: &OutputStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    query: &OutputSearchQuery,
) -> Result<Option<OutputSearch>, ControlError> {
    let needle: String = query.text.to_lowercase();
    if needle.is_empty() {
        return Err(ControlError::Invalid("search text is empty".into()));
    }
    let run_id = run.2;
    let after: i64 = query.after.unwrap_or(0).max(0);
    let limit: usize = usize::try_from(
        query
            .limit
            .unwrap_or(MAX_SEARCH_MATCHES)
            .clamp(1, MAX_SEARCH_MATCHES),
    )
    .unwrap_or(1);
    let (node, stream): (&str, &str) = (&query.node, query.stream.as_str());
    let Some(mut segments) = read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.search",
            async move |transaction, _| {
                segments_between(transaction, (run_id, node, stream), (after + 1, i64::MAX)).await
            },
            |_: &Vec<StoredSegment>| json!({"node": node, "stream": stream}),
        ),
    )
    .await?
    else {
        return Ok(None);
    };
    let more: bool = segments.len() > SEARCH_CHUNKS;
    segments.truncate(SEARCH_CHUNKS);
    let texts: Vec<String> = segment_texts(output, &segments).await?;
    Ok(Some(matching_lines(
        numbered_lines(&segments, &texts, (after + 1, i64::MAX)),
        (&needle, limit),
        (after, more),
    )))
}
