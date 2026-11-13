//! Decide what `--changed` does with each selected node, from its facts, its recent executions and the policy.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use crate::consumption::_helpers::history::Window;
use crate::consumption::constants::{
    DEFINITION_PLANNER_CHANGES, NODE_INPUT, OPAQUE_INPUT, SOURCE_INPUT,
};
use crate::consumption::models::{
    ChangeKind, ChangedOutcome, ChangedPolicy, ConsumptionHistory, ForcedRun, HeldBack, InputFact,
    NodeFacts, NodeKey, Reach, RelationKey, RunReason,
};

/// The state table key of an input: its kind and name.
pub(crate) fn input_key(input: &InputFact) -> (&'static str, String) {
    match input {
        InputFact::Source { name, .. } => (SOURCE_INPUT, name.clone()),
        InputFact::Node(key) => (NODE_INPUT, node_input_name(key)),
        InputFact::Opaque(name) => (OPAQUE_INPUT, name.clone()),
    }
}

/// The recorded name of an upstream node input: `<kind>:<name>`.
pub(crate) fn node_input_name(key: &NodeKey) -> String {
    format!("{}:{}", key.kind, key.name)
}

/// One reason a node's own records give for running, its kind (None always counts) and its origin.
struct Change {
    reason: RunReason,
    kind: Option<ChangeKind>,
    origin: Option<NodeKey>,
}

/// Where a node stands after its own records and its upstreams are considered.
enum Status<'a> {
    Unaffected,
    /// It runs; `groups` holds, for each change that reaches it, the triggered jobs holding that change.
    Runs {
        reason: RunReason,
        groups: BTreeSet<Vec<&'a str>>,
    },
    Held {
        why: HeldBack,
        groups: BTreeSet<Vec<&'a str>>,
    },
}

/// What the decision reads besides each node's own facts.
struct Context<'a> {
    by_key: HashMap<&'a NodeKey, &'a NodeFacts>,
    by_input_name: HashMap<String, &'a NodeFacts>,
    triggered: HashMap<&'a NodeKey, Vec<&'a str>>,
    history: &'a ConsumptionHistory,
    counts: &'a BTreeSet<ChangeKind>,
    by_relation: HashMap<&'a RelationKey, &'a NodeKey>,
    moved: RefCell<HashMap<(RelationKey, String, String), Moved>>,
}

/// Every relation where a rebuild's definition changed (empty: none did; None: unknown).
type Moved = Option<BTreeSet<RelationKey>>;

/// Each selected node in input order with what `--changed` does with it.
pub(crate) fn decide(
    nodes: &[NodeFacts],
    history: &ConsumptionHistory,
    policy: &ChangedPolicy,
) -> Vec<(NodeKey, ChangedOutcome)> {
    let mut triggered: HashMap<&NodeKey, Vec<&str>> = HashMap::new();
    if let Reach::Job(jobs) = &policy.reach {
        for job in jobs {
            for node in &job.nodes {
                triggered.entry(node).or_default().push(job.name.as_str());
            }
        }
        for names in triggered.values_mut() {
            names.sort_unstable();
            names.dedup();
        }
    }
    let context: Context<'_> = Context {
        by_key: nodes.iter().map(|node| (&node.key, node)).collect(),
        by_input_name: nodes
            .iter()
            .map(|node| (node_input_name(&node.key), node))
            .collect(),
        triggered,
        history,
        counts: &policy.counts,
        by_relation: nodes
            .iter()
            .map(|node| (&node.relation, &node.key))
            .collect(),
        moved: RefCell::new(HashMap::new()),
    };
    let mut state: HashMap<&NodeKey, Status<'_>> = HashMap::new();
    for node in topological(nodes, &context.by_key) {
        let status: Status<'_> = node_status(node, &context, &state);
        state.insert(&node.key, status);
    }
    nodes
        .iter()
        .filter(|node| node.selected)
        .map(|node| {
            let outcome: ChangedOutcome = match state.remove(&node.key) {
                Some(Status::Runs { reason, .. }) => ChangedOutcome::Runs(reason),
                Some(Status::Held { why, .. }) => ChangedOutcome::HeldBack(why),
                Some(Status::Unaffected) | None => ChangedOutcome::Unchanged,
            };
            (node.key.clone(), outcome)
        })
        .collect()
}

/// Runs when it or an upstream changed, unless held back by its jobs or a held-back upstream.
fn node_status<'a>(
    node: &'a NodeFacts,
    context: &Context<'a>,
    state: &HashMap<&'a NodeKey, Status<'a>>,
) -> Status<'a> {
    let own: Vec<Change> = own_changes(node, context);
    let mut groups: BTreeSet<Vec<&'a str>> = BTreeSet::new();
    let mut running: Option<&NodeKey> = None;
    let mut held: Option<&NodeKey> = None;
    for input in &node.inputs {
        let InputFact::Node(upstream) = input else {
            continue;
        };
        match state.get(upstream) {
            Some(Status::Runs {
                groups: carried, ..
            }) => {
                running.get_or_insert(upstream);
                groups.extend(carried.iter().cloned());
            }
            Some(Status::Held {
                groups: carried, ..
            }) => {
                held.get_or_insert(upstream);
                groups.extend(carried.iter().cloned());
            }
            Some(Status::Unaffected) | None => {}
        }
    }
    for change in &own {
        groups.insert(holding_jobs(node, change, context));
    }
    let reason: Option<RunReason> = match (own.into_iter().next(), running) {
        (Some(change), _) => Some(change.reason),
        (None, Some(upstream)) => Some(RunReason::UpstreamRuns(upstream.clone())),
        (None, None) if held.is_some() => None,
        (None, None) => return Status::Unaffected,
    };
    if let Some(jobs) = context.triggered.get(&node.key)
        && groups.iter().all(|holding| outside(holding, jobs))
    {
        return Status::Held {
            why: HeldBack::RefreshedBy(jobs.iter().map(|job| (*job).to_owned()).collect()),
            groups,
        };
    }
    match (held, reason) {
        (Some(upstream), _) => Status::Held {
            why: HeldBack::ReadsHeldBack(upstream.clone()),
            groups,
        },
        (None, Some(reason)) => Status::Runs { reason, groups },
        (None, None) => Status::Unaffected,
    }
}

/// The triggered jobs of the node a change came from, which hold that change.
fn holding_jobs<'a>(node: &'a NodeFacts, change: &Change, context: &Context<'a>) -> Vec<&'a str> {
    let origin: &NodeKey = change
        .origin
        .as_ref()
        .and_then(|key| context.by_key.get(key).map(|facts| &facts.key))
        .unwrap_or(&node.key);
    context.triggered.get(origin).cloned().unwrap_or_default()
}

/// Every reason the node's own facts or records give that always counts or has a counted kind.
fn own_changes(node: &NodeFacts, context: &Context<'_>) -> Vec<Change> {
    let mut changes: Vec<Change> = node
        .forced
        .iter()
        .map(|forced| Change {
            reason: RunReason::Forced(forced.clone()),
            kind: forced_kind(forced),
            origin: None,
        })
        .collect();
    let history: &ConsumptionHistory = context.history;
    match history.window(&node.relation) {
        None => changes.push(always(RunReason::NoRecord)),
        Some(window) if window.ambiguous => changes.push(always(RunReason::AmbiguousOrder)),
        Some(window) => changes.extend(execution_changes(node, context, &window)),
    }
    changes.retain(|change| counted(change.kind, context.counts));
    changes
}

/// A planner change of the definition itself counts as Definition; every other forced run always counts.
fn forced_kind(forced: &ForcedRun) -> Option<ChangeKind> {
    match forced {
        ForcedRun::PlannerChange(kind) if DEFINITION_PLANNER_CHANGES.contains(&kind.as_str()) => {
            Some(ChangeKind::Definition)
        }
        _ => None,
    }
}

/// True when none of the jobs holding a change refresh the node.
fn outside(holding: &[&str], jobs: &[&str]) -> bool {
    holding.iter().all(|job| !jobs.contains(job))
}

fn always(reason: RunReason) -> Change {
    Change {
        reason,
        kind: None,
        origin: None,
    }
}

fn counted(kind: Option<ChangeKind>, counts: &BTreeSet<ChangeKind>) -> bool {
    kind.is_none_or(|kind| counts.contains(&kind))
}

/// Every difference between what the node reads now and what each execution in the window read.
fn execution_changes(node: &NodeFacts, context: &Context<'_>, window: &Window<'_>) -> Vec<Change> {
    let history: &ConsumptionHistory = context.history;
    let expected: BTreeSet<(&str, String)> = node.inputs.iter().map(input_key).collect();
    let mut changes: Vec<Change> = Vec::new();
    for execution in window.executions {
        if !execution.complete {
            return vec![always(RunReason::Unfinished)];
        }
        if execution.version_hash.as_deref() != Some(node.version_hash.as_str()) {
            changes.push(definition(RunReason::DefinitionChanged));
        }
        let recorded: BTreeSet<(&str, String)> = execution
            .inputs
            .iter()
            .map(|input| (input.kind.as_str(), input.name.clone()))
            .collect();
        if recorded != expected {
            changes.push(definition(RunReason::InputsChanged));
            continue;
        }
        for input in &node.inputs {
            let (kind, name) = input_key(input);
            let read: Option<&str> = execution
                .inputs
                .iter()
                .find(|recorded| recorded.kind == kind && recorded.name == name)
                .map(|recorded| recorded.version.as_str());
            match input {
                InputFact::Opaque(_) => return vec![always(RunReason::InputUnknown(name))],
                InputFact::Source { version: None, .. } => changes.push(Change {
                    reason: RunReason::InputUnknown(name),
                    kind: Some(ChangeKind::SourceData),
                    origin: None,
                }),
                InputFact::Source {
                    version: Some(current),
                    ..
                } => {
                    if read != Some(current.as_str()) {
                        changes.push(Change {
                            reason: RunReason::InputChanged(name),
                            kind: Some(ChangeKind::SourceData),
                            origin: None,
                        });
                    }
                }
                InputFact::Node(upstream) => {
                    let Some(facts) = context.by_key.get(upstream).copied() else {
                        changes.push(always(RunReason::InputUnknown(name)));
                        continue;
                    };
                    let Some(current) = history.latest_complete(&facts.relation) else {
                        changes.push(Change {
                            reason: RunReason::InputUnknown(name),
                            kind: None,
                            origin: Some(upstream.clone()),
                        });
                        continue;
                    };
                    let Some(read) = read.filter(|read| *read != current) else {
                        continue;
                    };
                    match definition_moved(context, &facts.relation, (read, current)) {
                        Some(moved) if !moved.is_empty() => {
                            changes.extend(moved.iter().map(|changed| {
                                Change {
                                    reason: RunReason::UpstreamDefinitionChanged(name.clone()),
                                    kind: Some(ChangeKind::Definition),
                                    origin: Some(
                                        context
                                            .by_relation
                                            .get(changed)
                                            .map_or_else(|| upstream.clone(), |key| (*key).clone()),
                                    ),
                                }
                            }));
                        }
                        moved => changes.push(Change {
                            reason: RunReason::InputChanged(name),
                            kind: moved.map(|_| ChangeKind::SourceData),
                            origin: Some(upstream.clone()),
                        }),
                    }
                }
            }
        }
    }
    changes
}

fn definition(reason: RunReason) -> Change {
    Change {
        reason,
        kind: Some(ChangeKind::Definition),
        origin: None,
    }
}

/// Where execution `new` of `relation` or its reads changed definition since execution `old`.
fn definition_moved(
    context: &Context<'_>,
    relation: &RelationKey,
    (old, new): (&str, &str),
) -> Moved {
    if old == new {
        return Some(BTreeSet::new());
    }
    let memo_key: (RelationKey, String, String) =
        (relation.clone(), old.to_owned(), new.to_owned());
    let known: Option<Moved> = context.moved.borrow().get(&memo_key).cloned();
    if let Some(known) = known {
        return known;
    }
    context.moved.borrow_mut().insert(memo_key.clone(), None);
    let moved: Moved = compare_executions(context, relation, (old, new));
    context.moved.borrow_mut().insert(memo_key, moved.clone());
    moved
}

fn compare_executions(
    context: &Context<'_>,
    relation: &RelationKey,
    (old, new): (&str, &str),
) -> Moved {
    let history: &ConsumptionHistory = context.history;
    let before = history.execution(relation, old)?;
    let after = history.execution(relation, new)?;
    if !before.complete || !after.complete {
        return None;
    }
    let mut changed: BTreeSet<RelationKey> = BTreeSet::new();
    if before.version_hash != after.version_hash {
        changed.insert(relation.clone());
    }
    if after.inputs.iter().any(|input| input.kind == OPAQUE_INPUT) {
        return (!changed.is_empty()).then_some(changed);
    }
    let mut unknown: bool = false;
    let before_nodes: usize = before
        .inputs
        .iter()
        .filter(|input| input.kind == NODE_INPUT)
        .count();
    let mut after_nodes: usize = 0;
    for input in after.inputs.iter().filter(|input| input.kind == NODE_INPUT) {
        after_nodes += 1;
        let Some(read_before) = before
            .inputs
            .iter()
            .find(|earlier| earlier.kind == NODE_INPUT && earlier.name == input.name)
        else {
            changed.insert(relation.clone());
            continue;
        };
        let Some(upstream) = context.by_input_name.get(&input.name).copied() else {
            unknown = true;
            continue;
        };
        match definition_moved(
            context,
            &upstream.relation,
            (&read_before.version, &input.version),
        ) {
            Some(moved) => changed.extend(moved),
            None => unknown = true,
        }
    }
    if after_nodes != before_nodes {
        changed.insert(relation.clone());
    }
    (!changed.is_empty() || !unknown).then_some(changed)
}

/// Nodes with every upstream that has facts before it.
fn topological<'a>(
    nodes: &'a [NodeFacts],
    by_key: &HashMap<&NodeKey, &'a NodeFacts>,
) -> Vec<&'a NodeFacts> {
    let mut ordered: Vec<&'a NodeFacts> = Vec::with_capacity(nodes.len());
    let mut placed: BTreeSet<&NodeKey> = BTreeSet::new();
    let mut stack: Vec<(&'a NodeFacts, usize)> = Vec::new();
    for root in nodes {
        if placed.contains(&root.key) {
            continue;
        }
        let mut entered: BTreeSet<&NodeKey> = BTreeSet::from([&root.key]);
        stack.push((root, 0));
        while let Some((node, next)) = stack.pop() {
            let upstream: Option<&'a NodeFacts> =
                node.inputs.get(next).and_then(|input| match input {
                    InputFact::Node(key) => by_key.get(key).copied(),
                    _ => None,
                });
            if next < node.inputs.len() {
                stack.push((node, next + 1));
                if let Some(upstream) = upstream
                    && !placed.contains(&upstream.key)
                    && entered.insert(&upstream.key)
                {
                    stack.push((upstream, 0));
                }
                continue;
            }
            if placed.insert(&node.key) {
                ordered.push(node);
            }
        }
    }
    ordered
}
