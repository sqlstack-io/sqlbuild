//! A person reads the providers of the current snapshot with view on every current node, like jobs.

use std::collections::BTreeSet;

use serde::Deserialize;
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::_helpers::reads::jobs_in;
use crate::declarations::models::{ProviderRecord, ProvidersRead};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn read_person_providers(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<ProvidersRead, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let Some(snapshot) = snapshot else {
        commit(transaction).await?;
        return Ok(ProvidersRead {
            snapshot_id: None,
            providers: Vec::new(),
        });
    };
    let (transaction, documents) =
        read_snapshot_documents(transaction, snapshot.snapshot_id, Some(&["providers"])).await?;
    let (transaction, jobs) = jobs_in(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    let mut providers = match documents.get("providers") {
        Some(document) => Vec::<ProviderRecord>::deserialize(&document["providers"])
            .map_err(|error| ControlError::Invalid(format!("providers document: {error}")))?,
        None => Vec::new(),
    };
    for provider in &mut providers {
        let nodes: BTreeSet<&String> = provider.nodes.iter().collect();
        for job in &jobs {
            if job.selection.iter().any(|key| nodes.contains(key)) {
                provider.jobs.push(job.job_key.clone());
            }
        }
    }
    Ok(ProvidersRead {
        snapshot_id: Some(snapshot.snapshot_id),
        providers,
    })
}
