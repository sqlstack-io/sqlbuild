//! Simulated worlds, the oracle's view of their data, and broken decision rules.

use crate::consumption::models::ConsumptionAppend;
use crate::consumption::models::ConsumptionRow;

/// SplitMix64, so every case replays from its seed.
pub(crate) struct Random(pub(crate) u64);

/// The broken writer variant: whether `started` rows are appended at all.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Writer {
    Shipped,
    NoStartedRow,
}

/// What a relation holds: nothing, a partial write, or data derived from known inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Content {
    Missing,
    Partial,
    Data(Derivation),
}

/// Exactly what one successful write read: its definition, each input's content and a nonce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Derivation {
    pub(crate) definition: u32,
    pub(crate) inputs: Vec<(Input, u64)>,
    pub(crate) nonce: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Input {
    Source(usize),
    Node(usize),
}

pub(crate) struct Source {
    pub(crate) version: u64,
    /// Whether its freshness signal exists; without one the version is unknown.
    pub(crate) observed: bool,
    /// Monotonic versions only increase; others may revert to any earlier value.
    pub(crate) monotonic: bool,
}

pub(crate) struct Node {
    pub(crate) inputs: Vec<Input>,
    pub(crate) definition: u32,
    pub(crate) schema: usize,
    pub(crate) nondeterministic: bool,
    pub(crate) enumerable: bool,
    pub(crate) content: Content,
    /// The planner's recorded definition: written best effort after a successful write.
    pub(crate) fingerprint: Option<u32>,
}

/// One simulated project and warehouse.
pub(crate) struct World {
    pub(crate) sources: Vec<Source>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) rows: Vec<ConsumptionRow>,
    pub(crate) clock: i64,
    pub(crate) nonce: u64,
    pub(crate) flushed: Vec<Vec<ConsumptionAppend>>,
    pub(crate) trace: Vec<String>,
}

/// A skip the oracle rejects.
#[derive(Debug)]
pub(crate) struct Miss {
    pub(crate) step: usize,
    pub(crate) node: usize,
    pub(crate) why: &'static str,
}

/// Compares only source inputs, as if transitive sources were all a model recorded.
pub(crate) struct TransitiveOnly;

/// Times an execution by its latest row, so a late duplicate looks newest.
pub(crate) struct LatestRowTime;

/// Ignores stale upstreams, selected or not.
pub(crate) struct NoUpstreamRecursion;

/// Ignores the planner's own change detection.
pub(crate) struct NoPlannerGuard;

pub(crate) struct SimulationTestCase {
    pub(crate) description: &'static str,
    pub(crate) miss: Option<(u64, String, Vec<String>)>,
    pub(crate) expected_miss: bool,
}
