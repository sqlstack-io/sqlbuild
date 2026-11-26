//! A node's whole output as text, both streams in the order printed or one of them, read a batch of segments at a time so a large log never sits in memory; the download is audited.

use futures_util::Stream;
use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::pages::text_after;
use crate::run_output::_helpers::views::view_line_count;
use crate::run_output::models::{OutputDownloadQuery, OutputStore, OutputView};
use crate::run_output::types::PersonRunRead;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::runs::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target.
pub async fn download_person_output(
    store: &ControlStore,
    output: &OutputStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    query: &OutputDownloadQuery,
) -> Result<Option<impl Stream<Item = Result<String, ControlError>> + Send + 'static>, ControlError>
{
    let view = OutputView {
        run_id: run.2,
        node: Some(query.node.clone()),
        stream: query.stream,
    };
    let Some(_) = read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            (
                "output.download",
                async |transaction, _| view_line_count(transaction, &view).await,
                |lines: &i64| json!({"node": query.node, "stream": query.stream, "lines": lines}),
            ),
            None,
        ),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(futures_util::stream::unfold(
        (store.clone(), output.clone(), view, Some(1_i64)),
        async |(store, output, view, bound)| {
            let read = text_after((&store, &output), &view, bound?).await;
            match read {
                Ok(Some((text, next))) => Some((Ok(text), (store, output, view, Some(next)))),
                Ok(None) => None,
                Err(error) => Some((Err(error), (store, output, view, None))),
            }
        },
    )))
}
