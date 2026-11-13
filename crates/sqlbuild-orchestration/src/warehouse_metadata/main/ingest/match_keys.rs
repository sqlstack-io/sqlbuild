//! A reconcile scan sends keys first: rows the mirror holds join the scan, and only the rest travel in full.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::events::{key_page_ids, tag_held};
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{MetadataKeyPage, MetadataKeyPageResponse};

pub async fn match_keys(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    page: &MetadataKeyPage,
) -> Result<MetadataKeyPageResponse, ControlError> {
    if page.generation < 1
        || page.keys.len() > MAX_HISTORY_RECORDS
        || page.scan.location.schema.trim().is_empty()
        || page.scan.complete
    {
        return Err(ControlError::Invalid(format!(
            "{} key pages need a positive generation, at most {MAX_HISTORY_RECORDS} keys and an incomplete scan of a named schema",
            page.kind.as_str().replace('_', " ")
        )));
    }
    let ids = key_page_ids((location_id, target), page)?;
    let transaction = begin_ingestion(
        store,
        (location_id, target),
        (page.session_id, page.generation),
    )
    .await?;
    let (transaction, held) = tag_held(
        transaction,
        (page.kind.mirror_table(), page.scan.scan_id),
        &ids,
    )
    .await?;
    commit(transaction).await?;
    Ok(MetadataKeyPageResponse {
        missing: ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| (!held.contains(id)).then_some(index))
            .collect(),
    })
}
