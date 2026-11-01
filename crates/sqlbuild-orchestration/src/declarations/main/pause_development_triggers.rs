//! Pause retained triggers before a development scheduler can acquire leadership.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::declarations::_helpers::development::pause_triggers;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn pause_development_triggers(
    store: &ControlStore,
    scope: (Uuid, &str),
) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    commit(pause_triggers(transaction, scope, &BTreeSet::new(), Uuid::now_v7()).await?).await
}
