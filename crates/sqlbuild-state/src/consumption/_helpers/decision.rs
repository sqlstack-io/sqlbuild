//! Decide which selected nodes `--changed` runs, from their facts and their recent executions.

use std::collections::{BTreeSet, HashMap};

use crate::consumption::_helpers::history::Window;
use crate::consumption::constants::{NODE_INPUT, OPAQUE_INPUT, SOURCE_INPUT};
use crate::consumption::models::{ConsumptionHistory, InputFact, NodeFacts, NodeKey, RunReason};

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

/// Each selected node in input order with the reason it runs, or None when it is skipped.
pub(crate) fn decide(
    nodes: &[NodeFacts],
    history: &ConsumptionHistory,
) -> Vec<(NodeKey, Option<RunReason>)> {
    let by_key: HashMap<&NodeKey, &NodeFacts> =
        nodes.iter().map(|node| (&node.key, node)).collect();
    let mut stale: HashMap<&NodeKey, Option<RunReason>> = HashMap::new();
    for node in topological(nodes, &by_key) {
        let reason: Option<RunReason> = match own_reason(node, &by_key, history) {
            Some(reason) => Some(reason),
            None => stale_upstream(node, &stale),
        };
        stale.insert(&node.key, reason);
    }
    nodes
        .iter()
        .filter(|node| node.selected)
        .map(|node| (node.key.clone(), stale.remove(&node.key).flatten()))
        .collect()
}

/// The first upstream of `node` that runs or is stale.
fn stale_upstream(
    node: &NodeFacts,
    stale: &HashMap<&NodeKey, Option<RunReason>>,
) -> Option<RunReason> {
    for input in &node.inputs {
        if let InputFact::Node(upstream) = input
            && matches!(stale.get(upstream), Some(Some(_)))
        {
            return Some(RunReason::UpstreamRuns(upstream.clone()));
        }
    }
    None
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

/// Why the node's own facts or records make it run, ignoring its upstreams' staleness.
fn own_reason(
    node: &NodeFacts,
    by_key: &HashMap<&NodeKey, &NodeFacts>,
    history: &ConsumptionHistory,
) -> Option<RunReason> {
    if let Some(forced) = node.forced.first() {
        return Some(RunReason::Forced(forced.clone()));
    }
    let Some(window) = history.window(&node.relation) else {
        return Some(RunReason::NoRecord);
    };
    if window.ambiguous {
        return Some(RunReason::AmbiguousOrder);
    }
    execution_reason(node, (by_key, history), &window)
}

fn execution_reason(
    node: &NodeFacts,
    (by_key, history): (&HashMap<&NodeKey, &NodeFacts>, &ConsumptionHistory),
    window: &Window<'_>,
) -> Option<RunReason> {
    let expected: BTreeSet<(&str, String)> = node.inputs.iter().map(input_key).collect();
    for execution in window.executions {
        if !execution.complete {
            return Some(RunReason::Unfinished);
        }
        if execution.version_hash.as_deref() != Some(node.version_hash.as_str()) {
            return Some(RunReason::DefinitionChanged);
        }
        let recorded: BTreeSet<(&str, String)> = execution
            .inputs
            .iter()
            .map(|input| (input.kind.as_str(), input.name.clone()))
            .collect();
        if recorded != expected {
            return Some(RunReason::InputsChanged);
        }
        for input in &node.inputs {
            let (kind, name) = input_key(input);
            let current: Option<&str> = match input {
                InputFact::Source { version, .. } => version.as_deref(),
                InputFact::Node(upstream) => by_key
                    .get(upstream)
                    .and_then(|facts| history.latest_complete(&facts.relation)),
                InputFact::Opaque(_) => None,
            };
            let Some(current) = current else {
                return Some(RunReason::InputUnknown(name));
            };
            let read: Option<&str> = execution
                .inputs
                .iter()
                .find(|recorded| recorded.kind == kind && recorded.name == name)
                .map(|recorded| recorded.version.as_str());
            if read != Some(current) {
                return Some(RunReason::InputChanged(name));
            }
        }
    }
    None
}
