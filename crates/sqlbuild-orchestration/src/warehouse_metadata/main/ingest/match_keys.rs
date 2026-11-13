//! A reconcile scan sends keys first: rows the mirror holds join the scan, and only the rest travel in full.

use std::collections::HashSet;

use serde::Deserialize;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query_records::query_records};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{begin_ingestion, scoped_event_id};
use crate::warehouse_metadata::_helpers::source_keys::source_id;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{MetadataKeyPage, MetadataKeyPageResponse};

#[derive(Deserialize)]
struct Held {
    event_id: Uuid,
}

pub async fn match_keys(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    page: &MetadataKeyPage,
) -> Result<MetadataKeyPageResponse, ControlError> {
    let kind = page.kind.as_str().replace('_', " ");
    let source_table = page.kind.source_table();
    if page.generation < 1
        || page.keys.len() > MAX_HISTORY_RECORDS
        || page.scan.location.schema.trim().is_empty()
        || page.scan.complete
    {
        return Err(ControlError::Invalid(format!(
            "{kind} key pages need a positive generation, at most {MAX_HISTORY_RECORDS} keys and an incomplete scan of a named schema"
        )));
    }
    let mut ids: Vec<Uuid> = Vec::with_capacity(page.keys.len());
    for key in &page.keys {
        let (Some(source_table), Some(source_id)) = (source_table, source_id(page.kind, key)?)
        else {
            return Err(ControlError::Invalid(format!("{kind} has no key pages")));
        };
        ids.push(scoped_event_id(
            (location_id, target),
            (
                page.scan.location.database.as_deref(),
                &page.scan.location.schema,
                source_table,
            ),
            &source_id,
        )?);
    }
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (page.session_id, page.generation),
    )
    .await?;
    let mut held: HashSet<Uuid> = HashSet::new();
    if !ids.is_empty() {
        let mut parameters = vec![Parameter::Id(page.scan.scan_id)];
        parameters.extend(ids.iter().copied().map(Parameter::Id));
        let placeholders = Parameter::placeholders(2..=parameters.len());
        let rows: Vec<Held>;
        (transaction, rows) = query_records(
            transaction,
            &format!(
                "UPDATE {} SET scan_id=$1 WHERE event_id IN ({placeholders}) RETURNING event_id",
                page.kind.mirror_table()
            ),
            parameters,
        )
        .await?;
        held.extend(rows.into_iter().map(|row| row.event_id));
    }
    commit(transaction).await?;
    Ok(MetadataKeyPageResponse {
        missing: ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| (!held.contains(id)).then_some(index))
            .collect(),
    })
}
