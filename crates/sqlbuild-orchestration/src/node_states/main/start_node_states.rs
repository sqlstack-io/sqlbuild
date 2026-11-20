//! Committed change signals refresh the views change streams watch and drop the others, which the next read loads again; a lost signal or a resync covers every held view, and only what changed is pushed, since clients refetch on `all`.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::broadcast;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::node_states::_helpers::checks::drop_checks;
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
                        watched_targets(&cache, Some((location_id, &target_hash))),
                        true,
                    ),
                    Ok(ChangeMessage::Resync) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        (watched_targets(&cache, None), false)
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

/// The watched targets a signal names, or every watched target, after dropping the check list of every target it names; the unwatched ones it names are dropped once no refresh is pending or due, since one still pushes the old entry's versions.
fn watched_targets(
    cache: &NodeStateCache,
    scope: Option<(Uuid, &str)>,
) -> Vec<((Uuid, String), Arc<TargetCache>)> {
    let mut targets = cache.targets.lock().unwrap_or_else(PoisonError::into_inner);
    let named = |(location_id, _): &(Uuid, String), entry: &TargetCache| {
        scope.is_none_or(|(location, target_hash)| {
            *location_id == location && entry.target_hash == target_hash
        })
    };
    // Every named target's check list is dropped, watched or not: any result may have changed.
    for (key, entry) in targets.iter() {
        if named(key, entry) {
            drop_checks(entry);
        }
    }
    targets.retain(|key, entry| {
        !named(key, entry)
            || entry.watchers.load(Ordering::SeqCst) > 0
            || entry.dirty.load(Ordering::SeqCst)
            || entry.scheduled.load(Ordering::SeqCst)
    });
    targets
        .iter()
        .filter(|(key, entry)| named(key, entry))
        .map(|(key, entry)| (key.clone(), entry.clone()))
        .collect()
}
