//! A node's whole stream as text, read one chunk at a time so a large log never sits in memory; the download is audited.

use futures_util::Stream;
use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::index::segments_between;
use crate::run_output::_helpers::pages::segment_texts;
use crate::run_output::models::{OutputDownloadQuery, OutputStore, PersonRunRead, StoredSegment};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::sensitive_run_read;

/// `None` when the run is not in this location and target.
pub async fn download_person_output(
    store: &ControlStore,
    output: &OutputStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    query: &OutputDownloadQuery,
) -> Result<Option<impl Stream<Item = Result<String, ControlError>> + Send + 'static>, ControlError>
{
    let run_id = run.2;
    let (node, stream): (&str, &str) = (&query.node, query.stream.as_str());
    let Some(segments) = sensitive_run_read(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.download",
            async move |transaction, _| segments_between(transaction, (run_id, node, stream), (1, i64::MAX)).await,
            |segments: &Vec<StoredSegment>| json!({"node": node, "stream": stream, "segments": segments.len()}),
        ),
    )
    .await?
    else {
        return Ok(None);
    };
    let output: OutputStore = output.clone();
    Ok(Some(futures_util::stream::unfold(
        (output, segments.into_iter()),
        async |(output, mut remaining)| {
            let segment: StoredSegment = remaining.next()?;
            let text: Result<String, ControlError> =
                segment_texts(&output, std::slice::from_ref(&segment))
                    .await
                    .map(|mut texts| texts.pop().unwrap_or_default());
            Some((text, (output, remaining)))
        },
    )))
}
