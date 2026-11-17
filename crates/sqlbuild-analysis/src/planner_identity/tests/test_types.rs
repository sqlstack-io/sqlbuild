use std::collections::{HashMap, HashSet};

use crate::planner_identity::models::{
    AvailableUpstreams, FunctionFingerprintFields, GraphKey, IdentityGraph, MissingUpstream,
    MissingUpstreamFixes, StaleWarning, StalenessGraph,
};
use crate::planner_identity::types::IdentityHashes;

pub(super) struct HashTestCase {
    pub(super) description: &'static str,
    pub(super) actual: String,
    pub(super) expected_hash: &'static str,
}

pub(super) struct StalenessTestCase {
    pub(super) description: &'static str,
    pub(super) graph: StalenessGraph,
    pub(super) expected_warnings: Vec<StaleWarning>,
}

pub(super) struct OrderTestCase {
    pub(super) description: &'static str,
    pub(super) upstream: Vec<(GraphKey, Vec<GraphKey>)>,
    pub(super) origins: HashMap<(GraphKey, GraphKey), String>,
    pub(super) expected_order: Result<Vec<GraphKey>, String>,
}

pub(super) struct BuildabilityTestCase {
    pub(super) description: &'static str,
    pub(super) selected: Vec<GraphKey>,
    pub(super) upstream: HashMap<GraphKey, Vec<GraphKey>>,
    pub(super) available: AvailableUpstreams,
    pub(super) origins: HashMap<(GraphKey, GraphKey), String>,
    pub(super) fixes: MissingUpstreamFixes,
    pub(super) expected_missing: Vec<MissingUpstream>,
    pub(super) expected_message: &'static str,
}

pub(super) struct WarningTextTestCase {
    pub(super) description: &'static str,
    pub(super) trigger_names: Vec<String>,
    pub(super) expected_message: &'static str,
}

pub(super) struct IdentityGraphTestCase {
    pub(super) description: &'static str,
    pub(super) graph: IdentityGraph,
    pub(super) selected: HashSet<GraphKey>,
    pub(super) base_hashes: Vec<(GraphKey, String)>,
    pub(super) expected_hashes: IdentityHashes,
    pub(super) expected_write_hashes: Vec<(GraphKey, String)>,
}

pub(super) struct IdentityGraphSizeTestCase {
    pub(super) description: &'static str,
    pub(super) layer_count: usize,
    pub(super) expected_hash_count: usize,
}

pub(super) struct FunctionSqlTestCase {
    pub(super) description: &'static str,
    pub(super) fields: FunctionFingerprintFields,
    pub(super) expected_sql: &'static str,
}

pub(super) struct DeepChainTestCase {
    pub(super) description: &'static str,
    pub(super) depth: usize,
    pub(super) expected_hash_count: usize,
    pub(super) expected_trigger_count: usize,
}
