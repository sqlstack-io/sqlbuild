//! Search a node's output for text, case-insensitively, from a line forwards or backwards; each search needs view.evidence and is audited.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::reads::pages::{search_part, search_whole};
use crate::run_output::_helpers::reads::views::view_chunk_count;
use crate::run_output::constants::SEARCH_CHUNKS;
use crate::run_output::models::{
    OutputSearch, OutputSearchQuery, OutputStore, OutputView, SearchDirection, ViewReader,
};
use crate::run_output::types::PersonRunRead;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::runs::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target; output small enough is searched whole, with a count, and a larger one a bounded part at a time.
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
    let view = OutputView {
        run_id: run.2,
        node: Some(query.node.clone()),
        stream: query.stream,
    };
    let Some(chunks) = read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            (
                "output.search",
                async |transaction, _| view_chunk_count(transaction, &view).await,
                |_: &i64| json!({"node": query.node, "stream": query.stream}),
            ),
            None,
        ),
    )
    .await?
    else {
        return Ok(None);
    };
    let (from, direction) = (
        query.from.unwrap_or(1).max(1),
        query.direction.unwrap_or_default(),
    );
    if chunks <= i64::try_from(SEARCH_CHUNKS).unwrap_or(i64::MAX) {
        let mut reader =
            ViewReader::open((store, output), &view, (SearchDirection::Forward, 1)).await?;
        return Ok(Some(
            search_whole(&mut reader, &needle, (from, direction)).await?,
        ));
    }
    let bound: i64 = match direction {
        SearchDirection::Forward => from,
        SearchDirection::Backward => from + 1,
    };
    let mut reader = ViewReader::open((store, output), &view, (direction, bound)).await?;
    Ok(Some(search_part(&mut reader, &needle).await?))
}
