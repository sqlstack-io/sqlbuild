//! Commit-scoped invalidations; authoritative records remain in their existing stores.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use tokio::sync::broadcast;

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{
    Change, ChangeMessage, ChangeTopic, ChangedScope, ControlTransaction, Parameter,
};

/// Weak entries do not keep deleted test databases or idle subscribers alive.
pub(crate) fn sqlite_changes(
    path: &Path,
) -> Result<broadcast::Sender<ChangeMessage>, ControlError> {
    static CHANNELS: OnceLock<Mutex<HashMap<PathBuf, broadcast::WeakSender<ChangeMessage>>>> =
        OnceLock::new();
    let path = path.canonicalize()?;
    let mut channels = CHANNELS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    channels.retain(|_, sender| sender.strong_count() > 0);
    if let Some(sender) = channels.get(&path).and_then(broadcast::WeakSender::upgrade) {
        return Ok(sender);
    }
    let (sender, _) = broadcast::channel(256);
    channels.insert(path, sender.downgrade());
    Ok(sender)
}

pub(crate) async fn resolve_changes(
    mut transaction: ControlTransaction,
) -> Result<(ControlTransaction, BTreeSet<ChangeMessage>), ControlError> {
    let mut messages: BTreeSet<ChangeMessage> = BTreeSet::new();
    for change in std::mem::take(&mut transaction.changes) {
        if change == Change::Credentials {
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_authentication_revision SET revision=revision+1 WHERE singleton=1",
                vec![],
            )
            .await?;
            messages.insert(ChangeMessage::CredentialsChanged);
            continue;
        }
        let scope = match &change {
            Change::Snapshot(location_id, target) | Change::NodeStatus(location_id, target) => {
                ChangedScope {
                    location_id: *location_id,
                    target: target.clone(),
                }
            }
            Change::Run(id) | Change::Trigger(id) => {
                let sql = if matches!(change, Change::Run(_)) {
                    "SELECT location_id, target FROM sqb_runs WHERE run_id=$1"
                } else {
                    "SELECT location_id, target FROM sqb_triggers WHERE trigger_id=$1"
                };
                let rows;
                (transaction, rows) = query(transaction, sql, vec![Parameter::Id(*id)]).await?;
                let Some(row) = rows.into_iter().next() else {
                    continue;
                };
                decode_row(row)?
            }
            Change::Credentials => continue,
        };
        let topics = match change {
            Change::Run(id) => vec![
                ChangeTopic::Runs,
                ChangeTopic::Run(id),
                ChangeTopic::NodeStatus,
            ],
            Change::Trigger(_) => vec![ChangeTopic::Ticks],
            Change::Snapshot(..) => vec![ChangeTopic::Snapshot, ChangeTopic::NodeStatus],
            Change::NodeStatus(..) => vec![ChangeTopic::NodeStatus],
            Change::Credentials => continue,
        };
        for topic in topics {
            messages.insert(ChangeMessage::Changed {
                location_id: scope.location_id,
                target_hash: sha256_hex(scope.target.as_bytes()),
                topic,
            });
        }
    }
    Ok((transaction, messages))
}
