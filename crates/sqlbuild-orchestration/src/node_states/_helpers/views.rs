//! Views are replaced, never edited: a refresh diffs the store's states against the held view and pushes the difference.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::api::models::ChangeNotice;
use crate::node_states::_helpers::reads::read_target;
use crate::node_states::constants::MAX_PUSHED_NODE_STATES;
use crate::node_states::models::{
    NodeState, NodeStateCache, NodeStatesWatch, NodeStatusUpdate, TargetCache, TargetView,
};
use crate::store::errors::ControlError;

/// The target's cache entry, created empty on first use.
pub(crate) fn target_cache(cache: &NodeStateCache, scope: (Uuid, &str)) -> Arc<TargetCache> {
    held_entry(cache, scope, false)
}

/// Watch the target's entry; counted under the targets lock, so a signal never drops an entry a stream holds.
pub(crate) fn watch_target(cache: &NodeStateCache, scope: (Uuid, &str)) -> NodeStatesWatch {
    NodeStatesWatch {
        entry: held_entry(cache, scope, true),
    }
}

fn held_entry(
    cache: &NodeStateCache,
    (location_id, target): (Uuid, &str),
    watch: bool,
) -> Arc<TargetCache> {
    let mut targets = cache.targets.lock().unwrap_or_else(PoisonError::into_inner);
    let entry = targets
        .entry((location_id, target.to_owned()))
        .or_insert_with(|| {
            Arc::new(TargetCache {
                target_hash: sha256_hex(target.as_bytes()),
                ..TargetCache::default()
            })
        });
    if watch {
        entry.watchers.fetch_add(1, Ordering::SeqCst);
    }
    entry.clone()
}

pub(crate) fn held_view(entry: &TargetCache) -> Option<Arc<TargetView>> {
    entry
        .view
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The held view, read from the store only on first use or when it is not of the `expected` snapshot.
pub(crate) async fn current_view(
    cache: &NodeStateCache,
    scope: (Uuid, &str),
    expected: Option<Option<Uuid>>,
) -> Result<Arc<TargetView>, ControlError> {
    let entry = target_cache(cache, scope);
    let usable = |view: &Arc<TargetView>| {
        expected.is_none_or(|snapshot_id| view.catalog.snapshot_id == snapshot_id)
    };
    if let Some(view) = held_view(&entry).filter(usable) {
        return Ok(view);
    }
    let _refreshing = entry.refresh.lock().await;
    if let Some(view) = held_view(&entry).filter(usable) {
        return Ok(view);
    }
    refresh_view(cache, scope, (&entry, false)).await
}

/// Replace the view when states or the changed build differ and push the difference, or an empty one when `announced`; a new changed build is pushed as `full`, so clients read it with the states. The caller holds `entry.refresh`.
pub(crate) async fn refresh_view(
    cache: &NodeStateCache,
    (location_id, target): (Uuid, &str),
    (entry, announced): (&TargetCache, bool),
) -> Result<Arc<TargetView>, ControlError> {
    let previous = held_view(entry);
    let (catalog, states, changed_build) = read_target(
        &cache.store,
        (location_id, target),
        previous.as_ref().map(|view| view.catalog.clone()),
    )
    .await?;
    let previous_version = previous.as_ref().map_or(0, |previous| previous.version);
    let changed = match &previous {
        Some(previous)
            if previous.catalog.snapshot_id == catalog.snapshot_id
                && previous.changed_build.as_deref() == changed_build.as_ref() =>
        {
            Some(changed_states(&previous.states, &states))
        }
        _ => None,
    };
    let (view, notice) = match (previous, changed) {
        (Some(previous), Some(changed)) if changed.is_empty() => {
            let notice = announced.then(|| ChangeNotice {
                version: Some(previous.version),
                ..state_notice(target, previous.version, Some(changed))
            });
            (previous, notice)
        }
        (previous, changed) => {
            let pushed = changed.filter(|changed| changed.len() <= MAX_PUSHED_NODE_STATES);
            let notice = (announced || previous.is_some())
                .then(|| state_notice(target, previous_version, pushed));
            let view = Arc::new(TargetView {
                version: previous_version + 1,
                catalog,
                states: Arc::new(states),
                changed_build: changed_build.map(Arc::new),
                states_body: Default::default(),
            });
            *entry.view.write().unwrap_or_else(PoisonError::into_inner) = Some(view.clone());
            (view, notice)
        }
    };
    if let Some(notice) = notice {
        let data = serde_json::to_string(&notice)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let _ = cache.updates.send(NodeStatusUpdate {
            location_id,
            target_hash: entry.target_hash.clone(),
            data: data.into(),
        });
    }
    Ok(view)
}

/// The next version's event: the changed `states`, or `full` when they are not pushed.
fn state_notice(
    target: &str,
    previous_version: u64,
    states: Option<BTreeMap<String, NodeState>>,
) -> ChangeNotice {
    ChangeNotice {
        topic: format!("node_status:{target}"),
        version: Some(previous_version + 1),
        previous_version: Some(previous_version),
        full: states.is_none().then_some(true),
        states,
    }
}

/// Nodes whose state differs, including nodes new to the map.
fn changed_states(
    previous: &BTreeMap<String, NodeState>,
    current: &BTreeMap<String, NodeState>,
) -> BTreeMap<String, NodeState> {
    current
        .iter()
        .filter(|(key, state)| previous.get(*key) != Some(*state))
        .map(|(key, state)| (key.clone(), state.clone()))
        .collect()
}

/// Mark the target changed and start its debounced refresh unless one is already pending; only a target a store signal named announces an unchanged refresh.
pub(crate) fn schedule_refresh(
    cache: &NodeStateCache,
    refreshes: &mut tokio::task::JoinSet<Option<ControlError>>,
    ((location_id, target), entry): ((Uuid, String), Arc<TargetCache>),
) {
    entry.dirty.store(true, Ordering::SeqCst);
    if entry.scheduled.swap(true, Ordering::SeqCst) {
        return;
    }
    let cache = cache.clone();
    refreshes.spawn(async move {
        let mut failure: Option<ControlError> = None;
        loop {
            tokio::time::sleep(crate::node_states::constants::NODE_STATES_DEBOUNCE).await;
            entry.dirty.store(false, Ordering::SeqCst);
            let announced: bool = entry.announce.swap(false, Ordering::SeqCst);
            {
                let _refreshing = entry.refresh.lock().await;
                if let Err(error) =
                    refresh_view(&cache, (location_id, &target), (&entry, announced)).await
                {
                    failure = Some(error);
                }
            }
            entry.scheduled.store(false, Ordering::SeqCst);
            if !entry.dirty.load(Ordering::SeqCst) || entry.scheduled.swap(true, Ordering::SeqCst) {
                return failure;
            }
        }
    });
}
