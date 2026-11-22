//! A page of a node's output, or of the whole run's, forwards, backwards or from the end, with similar lines folded on request; each read needs view.evidence and is audited.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::pages::{page_neighbours, page_rows, page_start};
use crate::run_output::_helpers::views::view_line_count;
use crate::run_output::constants::{DEFAULT_PAGE_ROWS, MAX_PAGE_ROWS};
use crate::run_output::models::{OutputPage, OutputPageQuery, OutputStore, OutputView, ViewReader};
use crate::run_output::types::PersonRunRead;
use crate::runs::constants::REPEATED_READ_AUDIT;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target; output nobody printed is an empty page.
pub async fn read_person_output_page(
    store: &ControlStore,
    output: &OutputStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    query: &OutputPageQuery,
) -> Result<Option<OutputPage>, ControlError> {
    let view = OutputView {
        run_id: run.2,
        node: query.node.clone(),
        stream: query.stream,
    };
    let limit: usize = usize::try_from(
        query
            .limit
            .unwrap_or(DEFAULT_PAGE_ROWS)
            .clamp(1, MAX_PAGE_ROWS),
    )
    .unwrap_or(1);
    let Some(line_count) = read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        ((
            "output.page",
            async |transaction, _| view_line_count(transaction, &view).await,
            |_: &i64| json!({"node": query.node, "stream": query.stream, "from": query.from, "before": query.before, "tail": query.tail}),
        ), Some(REPEATED_READ_AUDIT)),
    )
    .await?
    else {
        return Ok(None);
    };
    let start = page_start(query, line_count)?;
    let mut reader = ViewReader::open((store, output), &view, start).await?;
    let page = page_rows(&mut reader, limit, (query.collapse, query.around)).await?;
    let (from, to) = (
        page.low.unwrap_or(start.1),
        page.high.map_or(start.1, |high| high + 1),
    );
    let (next, previous) = page_neighbours(store, &view, (from, to)).await?;
    Ok(Some(OutputPage {
        rows: page.rows,
        line_count,
        from,
        to,
        next,
        previous,
    }))
}
