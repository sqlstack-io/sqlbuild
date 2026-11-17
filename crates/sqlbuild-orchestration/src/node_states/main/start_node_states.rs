//! Committed change signals mark held views stale; a lost signal or a resync refreshes every held view and pushes only what changed, since clients refetch on `all`.

use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::broadcast;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::node_states::_helpers::views::schedule_refresh;
use crate::node_states::constants::NODE_STATE_UPDATES;
use crate::node_states::models::{NodeStateCache, TargetCache};
use crate::store::errors::ControlError;
use crate::store::models::{ChangeMessage, ChangeTopic, ControlStore};

/// The server's cache and the task that keeps it current; dropping the task stops pending refreshes.
pub(crate) fn start_node_states(
    store: &ControlStore,
    changes: &broadcast::Sender<ChangeMessage>,
) -> (NodeStateCache, impl Future<Output = ()> + Send + 'static) {
    let cache = NodeStateCache {
        store: store.clone(),
        targets: Arc::new(Mutex::default()),
        updates: broadcast::channel(NODE_STATE_UPDATES).0,
    };
    let mut receiver = changes.subscribe();
    let task = {
        let cache = cache.clone();
        async move {
            let mut refreshes: JoinSet<Option<ControlError>> = JoinSet::new();
            loop {
                let message = tokio::select! {
                    message = receiver.recv() => message,
                    Some(joined) = refreshes.join_next(), if !refreshes.is_empty() => {
                        if let Ok(Some(error)) = joined {
                            eprintln!("Node states refresh failed; the next change retries: {error}");
                        }
                        continue;
                    }
                };
                let (entries, announced) = match message {
                    Ok(ChangeMessage::Changed {
                        location_id,
                        target_hash,
                        topic: ChangeTopic::NodeStatus | ChangeTopic::Snapshot,
                    }) => (
                        held_targets(&cache, Some((location_id, &target_hash))),
                        true,
                    ),
                    Ok(ChangeMessage::Resync) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        (held_targets(&cache, None), false)
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return std::future::pending().await;
                    }
                };
                for entry in entries {
                    if announced {
                        entry
                            .1
                            .announce
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    schedule_refresh(&cache, &mut refreshes, entry);
                }
            }
        }
    };
    (cache, task)
}

/// The held targets a signal names, or every held target.
fn held_targets(
    cache: &NodeStateCache,
    scope: Option<(Uuid, &str)>,
) -> Vec<((Uuid, String), Arc<TargetCache>)> {
    let targets = cache.targets.lock().unwrap_or_else(PoisonError::into_inner);
    targets
        .iter()
        .filter(|((location_id, _), entry)| {
            scope.is_none_or(|(location, target_hash)| {
                *location_id == location && entry.target_hash == target_hash
            })
        })
        .map(|(scope, entry)| (scope.clone(), entry.clone()))
        .collect()
}
