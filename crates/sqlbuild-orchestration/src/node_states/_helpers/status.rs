//! Each node's one build status, worked out once over a target's states so every page shows the same.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};

use crate::node_states::models::{NodeBuildStatus, NodeState};

/// What a node's status depends on apart from its own state.
pub(crate) struct StatusNode<'a> {
    pub(crate) key: &'a str,
    /// The snapshot node kind, such as `model`, `source` or `asset`.
    pub(crate) kind: &'a str,
    /// Whether it builds a warehouse object whose builds are fingerprinted.
    pub(crate) has_table: bool,
    /// A view reads its upstream when queried, so it is never behind.
    pub(crate) view: bool,
    /// The listed nodes it reads.
    pub(crate) upstream: Vec<&'a str>,
}

/// A status and when it was reached.
type Status = (NodeBuildStatus, Option<DateTime<Utc>>);

const SOURCE_KIND: &str = "source";
const FAILED_OUTCOME: &str = "failed";
const SUCCEEDED_OUTCOMES: [&str; 2] = ["success", "warn"];

/// When the data each node reads last changed, worked out once per node; a cycle reads as no data.
struct DataTimes<'a> {
    nodes: HashMap<&'a str, &'a StatusNode<'a>>,
    states: &'a BTreeMap<String, NodeState>,
    known: RefCell<HashMap<&'a str, Option<DateTime<Utc>>>>,
}

impl<'a> DataTimes<'a> {
    /// A node's own last build, or for a view the newest data behind it.
    fn at(&self, key: &'a str) -> Option<DateTime<Utc>> {
        if let Some(known) = self.known.borrow().get(key) {
            return *known;
        }
        self.known.borrow_mut().insert(key, None);
        let time = match self.nodes.get(key) {
            Some(node) if node.view => node.upstream.iter().filter_map(|up| self.at(up)).max(),
            _ => self
                .states
                .get(key)
                .and_then(|state| state.last_build.as_ref())
                .map(|build| build.observed_at),
        };
        self.known.borrow_mut().insert(key, time);
        time
    }

    /// Whether an upstream node has newer data than this node's last build; views and unbuilt nodes never are.
    fn behind(&self, node: &'a StatusNode<'a>, state: &NodeState) -> bool {
        let Some(build) = state
            .last_build
            .as_ref()
            .filter(|_| node.has_table && !node.view)
        else {
            return false;
        };
        node.upstream
            .iter()
            .any(|upstream| self.at(upstream).is_some_and(|at| at > build.observed_at))
    }
}

/// `states` with every node's `status` and `status_at` set.
pub(crate) fn apply_statuses(
    nodes: &[StatusNode<'_>],
    states: BTreeMap<String, NodeState>,
) -> BTreeMap<String, NodeState> {
    let statuses: HashMap<String, Status> = {
        let times = DataTimes {
            nodes: nodes.iter().map(|node| (node.key, node)).collect(),
            states: &states,
            known: RefCell::default(),
        };
        nodes
            .iter()
            .filter_map(|node| {
                let state = states.get(node.key)?;
                Some((
                    node.key.to_owned(),
                    status_of(node, state, times.behind(node, state)),
                ))
            })
            .collect()
    };
    states
        .into_iter()
        .map(|(key, state)| {
            let (status, status_at) = statuses
                .get(&key)
                .copied()
                .unwrap_or((state.status, state.status_at));
            (
                key,
                NodeState {
                    status,
                    status_at,
                    ..state
                },
            )
        })
        .collect()
}

/// A node's status and when it was reached, given whether it is behind its upstream.
fn status_of(node: &StatusNode<'_>, state: &NodeState, behind: bool) -> Status {
    if node.kind == SOURCE_KIND {
        return (NodeBuildStatus::Source, None);
    }
    let outcome = state.last_outcome.as_ref();
    if let Some(failed) = outcome.filter(|outcome| outcome.status == FAILED_OUTCOME) {
        return (NodeBuildStatus::Failed, Some(failed.at));
    }
    if !node.has_table {
        return outcome
            .filter(|outcome| SUCCEEDED_OUTCOMES.contains(&outcome.status.as_str()))
            .map_or((NodeBuildStatus::NotBuilt, None), |outcome| {
                (NodeBuildStatus::Current, Some(outcome.at))
            });
    }
    let Some(build) = &state.last_build else {
        return (NodeBuildStatus::NotBuilt, None);
    };
    let status = if state.definition_changed {
        NodeBuildStatus::Changed
    } else if behind {
        NodeBuildStatus::Behind
    } else {
        NodeBuildStatus::Current
    };
    (status, Some(build.observed_at))
}
