//! Group members, the nodes they build and read, and the waits and cycle report expected.

pub(crate) type Pairs = &'static [(&'static str, &'static [&'static str])];

pub(crate) struct GroupWaitGraphTestCase {
    pub(crate) description: &'static str,
    /// Each member job and the node keys it builds.
    pub(crate) selections: Pairs,
    /// Each node key and the keys it reads.
    pub(crate) deps: Pairs,
    pub(crate) expected_edges: Pairs,
    pub(crate) expected_message: Option<&'static str>,
}

pub(crate) struct LargeGroupTestCase {
    pub(crate) description: &'static str,
    pub(crate) members: usize,
    pub(crate) nodes_per_member: usize,
    pub(crate) expected_cycles: usize,
    /// Waits across all members: each waits for every member before it, and the first, which builds the tail, for all others.
    pub(crate) expected_waits: usize,
    pub(crate) expected_limit: std::time::Duration,
}

pub(crate) struct GroupWaitsTestCase {
    pub(crate) description: &'static str,
    /// Each node and the nodes it reads.
    pub(crate) lineage: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member job and the nodes it selects.
    pub(crate) jobs: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member and the members it waits for.
    pub(crate) expected_waits: Vec<(&'static str, Vec<&'static str>)>,
}

/// Members whose nodes read each other, and the cycle error that names both waits.
pub(crate) struct GroupCycleTestCase {
    pub(crate) description: &'static str,
    pub(crate) lineage: Vec<(&'static str, Vec<&'static str>)>,
    pub(crate) jobs: Vec<(&'static str, Vec<&'static str>)>,
    pub(crate) expected_error: &'static str,
}
