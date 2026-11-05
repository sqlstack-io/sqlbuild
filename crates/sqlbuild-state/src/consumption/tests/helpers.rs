//! A seeded simulation of builds against a warehouse, with an oracle that knows what each model read.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::consumption::_helpers::decision::decide;
use crate::consumption::_helpers::history::build_history;
use crate::consumption::constants::RECENT_EXECUTIONS;
use crate::consumption::main::consumed_inputs::consumed_inputs;
use crate::consumption::main::started_append::started_append;
use crate::consumption::main::succeeded_appends::succeeded_appends;
use crate::consumption::models::{
    ConsumptionAppend, ConsumptionHistory, ConsumptionRow, ForcedRun, InputFact, NodeFacts,
    NodeKey, RelationKey, RunReason,
};
use crate::consumption::tests::test_types::{
    Content, Derivation, Input, Miss, Node, Random, Source, World, Writer,
};

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z: u64 = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const SECOND: i64 = 1_000_000;
const SIMULATED_CASES: u64 = 400;
const SIMULATED_BUILDS: usize = 14;

impl World {
    pub(crate) fn new(random: &mut Random) -> Self {
        let source_count: usize = 2 + usize::try_from(random.below(2)).unwrap_or(0);
        let node_count: usize = 5 + usize::try_from(random.below(5)).unwrap_or(0);
        let sources: Vec<Source> = (0..source_count)
            .map(|_| Source {
                version: 1,
                observed: random.chance(85),
                monotonic: random.chance(50),
            })
            .collect();
        let mut nodes: Vec<Node> = Vec::new();
        for index in 0..node_count {
            let mut inputs: BTreeSet<Input> = BTreeSet::new();
            for source in 0..source_count {
                if random.chance(35) {
                    inputs.insert(Input::Source(source));
                }
            }
            for upstream in 0..index {
                if random.chance(40) {
                    inputs.insert(Input::Node(upstream));
                }
            }
            nodes.push(Node {
                inputs: inputs.into_iter().collect(),
                definition: 1,
                schema: usize::try_from(random.below(2)).unwrap_or(0),
                nondeterministic: random.chance(25),
                enumerable: random.chance(92),
                content: Content::Missing,
                fingerprint: None,
            });
        }
        Self {
            sources,
            nodes,
            rows: Vec::new(),
            clock: 1_000 * SECOND,
            nonce: 0,
            flushed: Vec::new(),
            trace: Vec::new(),
        }
    }

    fn key(index: usize) -> NodeKey {
        NodeKey {
            kind: "model".to_owned(),
            name: format!("m{index}"),
        }
    }

    fn relation(&self, index: usize) -> RelationKey {
        RelationKey {
            database: None,
            schema: format!("s{}", self.nodes[index].schema),
            name: format!("m{index}"),
        }
    }

    fn content_id(&self, input: Input) -> u64 {
        match input {
            Input::Source(source) => self.sources[source].version,
            Input::Node(node) => match &self.nodes[node].content {
                Content::Missing => 0,
                Content::Partial => 1,
                Content::Data(derivation) => 1_000 + derivation.nonce,
            },
        }
    }

    fn fresh_own(&self, index: usize) -> bool {
        let node: &Node = &self.nodes[index];
        match &node.content {
            Content::Data(derivation) => {
                derivation.definition == node.definition
                    && derivation
                        .inputs
                        .iter()
                        .all(|(input, read)| self.content_id(*input) == *read)
            }
            _ => false,
        }
    }

    /// Fresh and every upstream, transitively, fresh: what a skip requires.
    fn fresh_closure(&self, index: usize) -> bool {
        self.fresh_own(index)
            && self.nodes[index].inputs.iter().all(|input| match input {
                Input::Node(upstream) => self.fresh_closure(*upstream),
                Input::Source(_) => true,
            })
    }

    fn change_sources(&mut self, random: &mut Random) {
        for (index, source) in self.sources.iter_mut().enumerate() {
            if !random.chance(30) {
                continue;
            }
            let previous: u64 = source.version;
            source.version = if source.monotonic || random.chance(50) {
                source.version + 1
            } else {
                1 + random.below(source.version.max(1))
            };
            self.trace
                .push(format!("source s{index}: {previous} -> {}", source.version));
        }
    }

    fn change_definitions(&mut self, random: &mut Random) {
        for (index, node) in self.nodes.iter_mut().enumerate() {
            if random.chance(12) {
                node.definition += 1;
                self.trace
                    .push(format!("definition m{index} -> {}", node.definition));
            }
        }
    }

    /// The reader's SQL: rows of the newest executions per relation by earliest row time.
    fn read_rows(&self) -> Vec<ConsumptionRow> {
        let mut started: BTreeMap<(RelationKey, String), i64> = BTreeMap::new();
        for row in &self.rows {
            let entry = started
                .entry((row.row.relation.clone(), row.row.execution_id.clone()))
                .or_insert(row.recorded_at);
            *entry = (*entry).min(row.recorded_at);
        }
        let mut by_relation: BTreeMap<RelationKey, Vec<(i64, String)>> = BTreeMap::new();
        for ((relation, execution), time) in started {
            by_relation
                .entry(relation)
                .or_default()
                .push((time, execution));
        }
        let mut kept: BTreeSet<String> = BTreeSet::new();
        for executions in by_relation.values_mut() {
            executions.sort_by(|left, right| right.cmp(left));
            kept.extend(
                executions
                    .iter()
                    .take(RECENT_EXECUTIONS)
                    .map(|(_, execution)| execution.clone()),
            );
        }
        self.rows
            .iter()
            .filter(|row| kept.contains(&row.row.execution_id))
            .cloned()
            .collect()
    }

    /// The janitor's SQL: keep the newest `retain` executions per relation.
    fn prune(&mut self, retain: usize) {
        let mut started: BTreeMap<(RelationKey, String), i64> = BTreeMap::new();
        for row in &self.rows {
            let entry = started
                .entry((row.row.relation.clone(), row.row.execution_id.clone()))
                .or_insert(row.recorded_at);
            *entry = (*entry).min(row.recorded_at);
        }
        let mut by_relation: BTreeMap<RelationKey, Vec<(i64, String)>> = BTreeMap::new();
        for ((relation, execution), time) in started {
            by_relation
                .entry(relation)
                .or_default()
                .push((time, execution));
        }
        let mut dropped: BTreeSet<String> = BTreeSet::new();
        for executions in by_relation.values_mut() {
            executions.sort_by(|left, right| right.cmp(left));
            dropped.extend(
                executions
                    .iter()
                    .skip(retain)
                    .map(|(_, execution)| execution.clone()),
            );
        }
        self.rows
            .retain(|row| !dropped.contains(&row.row.execution_id));
        self.trace.push(format!("prune to {retain}"));
    }

    fn append(&mut self, rows: &[ConsumptionAppend]) {
        for row in rows {
            self.clock += SECOND / 10;
            self.rows.push(ConsumptionRow {
                row: row.clone(),
                recorded_at: self.clock,
            });
        }
    }

    fn facts(
        &self,
        selected: &BTreeSet<usize>,
        observed: &[Option<u64>],
        full_refresh: bool,
    ) -> Vec<NodeFacts> {
        (0..self.nodes.len())
            .map(|index| {
                let node: &Node = &self.nodes[index];
                let mut forced: Vec<ForcedRun> = Vec::new();
                if full_refresh {
                    forced.push(ForcedRun::FullRefresh);
                }
                if node.content == Content::Missing {
                    forced.push(ForcedRun::RelationMissing);
                } else if node.fingerprint != Some(node.definition) {
                    forced.push(ForcedRun::PlannerChange("query_changed".to_owned()));
                }
                if !node.enumerable {
                    forced.push(ForcedRun::InputsNotEnumerable);
                }
                NodeFacts {
                    key: Self::key(index),
                    relation: self.relation(index),
                    version_hash: format!("d{}", node.definition),
                    selected: selected.contains(&index),
                    forced,
                    inputs: node
                        .inputs
                        .iter()
                        .map(|input| match input {
                            Input::Source(source) => InputFact::Source {
                                name: format!("s{source}"),
                                version: observed[*source].map(|version| format!("v{version}")),
                            },
                            Input::Node(upstream) => InputFact::Node(Self::key(*upstream)),
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// One build; under `--changed` the oracle checks every skip before anything executes.
    pub(crate) fn build(
        &mut self,
        writer: Writer,
        random: &mut Random,
        step: usize,
    ) -> Result<(), Miss> {
        self.clock += i64::try_from(random.below(20)).unwrap_or(0) * SECOND;
        self.late_duplicate(random);
        self.change_sources(random);
        self.change_definitions(random);
        let changed: bool = random.chance(60);
        let selected: BTreeSet<usize> = if random.chance(50) {
            (0..self.nodes.len()).collect()
        } else {
            (0..self.nodes.len())
                .filter(|_| random.chance(50))
                .collect()
        };
        let full_refresh: bool = random.chance(5);
        let observed: Vec<Option<u64>> = self
            .sources
            .iter()
            .map(|source| source.observed.then_some(source.version))
            .collect();
        let facts: Vec<NodeFacts> = self.facts(&selected, &observed, full_refresh);
        let history: ConsumptionHistory = build_history(&self.read_rows(), RECENT_EXECUTIONS);
        let decided: Vec<(NodeKey, Option<RunReason>)> = decide(&facts, &history);
        let run: Vec<usize> = (0..self.nodes.len())
            .filter(|index| selected.contains(index))
            .filter(|index| {
                !changed
                    || decided
                        .iter()
                        .any(|(key, reason)| *key == Self::key(*index) && reason.is_some())
            })
            .collect();
        self.trace.push(format!(
            "step {step}: {} select {:?} run {:?}{}",
            if changed { "--changed" } else { "plain" },
            selected,
            run,
            if full_refresh { " full refresh" } else { "" }
        ));
        if changed {
            for index in &selected {
                if run.contains(index) {
                    continue;
                }
                if !self.fresh_closure(*index) {
                    return Err(Miss {
                        step,
                        node: *index,
                        why: "skipped a model whose data or an upstream's is stale",
                    });
                }
                if self.nodes[*index].fingerprint != Some(self.nodes[*index].definition) {
                    return Err(Miss {
                        step,
                        node: *index,
                        why: "skipped a model with a pending replay",
                    });
                }
            }
        }
        self.execute((&facts, &history), &run, (writer, random));
        if random.chance(15) {
            let retain: usize = 1 + usize::try_from(random.below(3)).unwrap_or(0);
            self.prune(retain);
        }
        Ok(())
    }

    fn execute(
        &mut self,
        (facts, history): (&[NodeFacts], &ConsumptionHistory),
        run: &[usize],
        (writer, random): (Writer, &mut Random),
    ) {
        let by_key: HashMap<NodeKey, NodeFacts> = facts
            .iter()
            .map(|facts| (facts.key.clone(), facts.clone()))
            .collect();
        let executions: HashMap<NodeKey, String> = run
            .iter()
            .map(|index| {
                self.nonce += 1;
                (Self::key(*index), format!("e{}", self.nonce))
            })
            .collect();
        let mut failed_start: BTreeSet<usize> = BTreeSet::new();
        for schema in 0..2 {
            let batch: Vec<ConsumptionAppend> = run
                .iter()
                .filter(|index| self.nodes[**index].schema == schema)
                .map(|index| started_append(&facts[*index], &executions[&Self::key(*index)], "run"))
                .collect();
            if batch.is_empty() || writer == Writer::NoStartedRow {
                continue;
            }
            if random.chance(4) {
                self.trace
                    .push(format!("started append fails in s{schema}"));
                failed_start.extend(
                    run.iter()
                        .filter(|index| self.nodes[**index].schema == schema),
                );
                continue;
            }
            self.append(&batch);
        }
        let mut done: BTreeSet<usize> = BTreeSet::new();
        let mut pending: Vec<Vec<ConsumptionAppend>> = vec![Vec::new(), Vec::new()];
        for index in run.iter().copied() {
            let blocked: bool = self.nodes[index].inputs.iter().any(|input| match input {
                Input::Node(upstream) => run.contains(upstream) && !done.contains(upstream),
                Input::Source(_) => false,
            });
            if blocked || failed_start.contains(&index) {
                continue;
            }
            if random.chance(5) {
                self.trace.push(format!("crash while writing m{index}"));
                self.nodes[index].content = Content::Partial;
                return;
            }
            if random.chance(6) {
                self.trace.push(format!("m{index} fails"));
                if random.chance(50) {
                    self.nodes[index].content = Content::Partial;
                }
                continue;
            }
            self.late_load(random);
            self.nonce += 1;
            let derivation = Derivation {
                definition: self.nodes[index].definition,
                inputs: self.nodes[index]
                    .inputs
                    .iter()
                    .map(|input| (*input, self.content_id(*input)))
                    .collect(),
                nonce: if self.nodes[index].nondeterministic {
                    self.nonce
                } else {
                    self.deterministic_nonce(index)
                },
            };
            self.nodes[index].content = Content::Data(derivation);
            if random.chance(85) {
                self.nodes[index].fingerprint = Some(self.nodes[index].definition);
            } else {
                self.trace
                    .push(format!("fingerprint write fails for m{index}"));
            }
            if random.chance(4) {
                self.trace.push(format!("crash after writing m{index}"));
                return;
            }
            done.insert(index);
            let key: NodeKey = Self::key(index);
            let inputs = consumed_inputs(&facts[index], (&by_key, history), &executions);
            pending[self.nodes[index].schema].extend(succeeded_appends(
                &facts[index],
                (&executions[&key], "run"),
                &inputs,
            ));
            if random.chance(40) && !self.flush(&mut pending, random) {
                return;
            }
        }
        self.flush(&mut pending, random);
    }

    /// A retried append of a recent batch lands again, later, with the same rows.
    fn late_duplicate(&mut self, random: &mut Random) {
        if self.flushed.is_empty() || !random.chance(15) {
            return;
        }
        let recent: u64 = self.flushed.len().min(3) as u64;
        let batch: usize =
            self.flushed.len() - 1 - usize::try_from(random.below(recent)).unwrap_or(0);
        let rows: Vec<ConsumptionAppend> = self.flushed[batch].clone();
        let retained: bool = rows.iter().all(|duplicate| {
            self.rows
                .iter()
                .any(|row| row.row.execution_id == duplicate.execution_id)
        });
        if retained {
            self.trace.push(format!("late duplicate of batch {batch}"));
            self.append(&rows);
        }
    }

    /// A deterministic model's content depends only on what it read.
    fn deterministic_nonce(&self, index: usize) -> u64 {
        let node: &Node = &self.nodes[index];
        let mut value: u64 = u64::from(node.definition) * 1_000_003 + index as u64;
        for input in &node.inputs {
            value = value.wrapping_mul(31).wrapping_add(self.content_id(*input));
        }
        value % 1_000_000_000
    }

    /// A monotonic source may load new data between observation and a model's read.
    fn late_load(&mut self, random: &mut Random) {
        for (index, source) in self.sources.iter_mut().enumerate() {
            if source.monotonic && random.chance(3) {
                source.version += 1;
                self.trace
                    .push(format!("late load of s{index} -> {}", source.version));
            }
        }
    }

    /// Append each schema's pending rows as one statement; a crash loses that statement whole.
    fn flush(&mut self, pending: &mut [Vec<ConsumptionAppend>], random: &mut Random) -> bool {
        for batch in pending.iter_mut() {
            if batch.is_empty() {
                continue;
            }
            if random.chance(3) {
                self.trace
                    .push("crash during the succeeded append".to_owned());
                return false;
            }
            let rows: Vec<ConsumptionAppend> = std::mem::take(batch);
            self.append(&rows);
            self.flushed.push(rows);
        }
        true
    }
}

/// Run `cases` seeded worlds of `steps` builds and return the first miss, with its seed and trace.
pub(crate) fn first_miss(
    writer: Writer,
    (cases, steps): (u64, usize),
) -> Option<(u64, Miss, Vec<String>)> {
    (0..cases).find_map(|seed| world_miss(writer, seed, steps))
}

/// The first miss of one seeded world.
fn world_miss(writer: Writer, seed: u64, steps: usize) -> Option<(u64, Miss, Vec<String>)> {
    let mut random = Random::new(seed);
    let mut world = World::new(&mut random);
    let miss: Miss = (0..steps).find_map(|step| world.build(writer, &mut random, step).err())?;
    Some((seed, miss, world.trace))
}

/// The first miss of `writer` over the standard cases, rendered for an assertion message.
pub(crate) fn found(writer: Writer) -> Option<(u64, String, Vec<String>)> {
    first_miss(writer, (SIMULATED_CASES, SIMULATED_BUILDS)).map(|(seed, miss, trace)| {
        (
            seed,
            format!("step {} m{}: {}", miss.step, miss.node, miss.why),
            trace,
        )
    })
}
