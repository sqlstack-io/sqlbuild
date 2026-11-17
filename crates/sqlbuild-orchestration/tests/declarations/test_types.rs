pub(crate) struct NodeWaitCycleTestCase {
    pub(crate) description: &'static str,
    /// Each node and the nodes it reads.
    pub(crate) lineage: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member job and the nodes it selects.
    pub(crate) jobs: Vec<(&'static str, Vec<&'static str>)>,
    /// The members that wait node by node; the others wait for whole jobs.
    pub(crate) node_waits: Vec<&'static str>,
    /// The cycle message, or `None` when the group publishes.
    pub(crate) expected_cycle: Option<&'static str>,
}

pub(crate) struct GroupWaitsTestCase {
    pub(crate) description: &'static str,
    /// Each node and the nodes it reads.
    pub(crate) lineage: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member job and the nodes it selects.
    pub(crate) jobs: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member and the members it waits for.
    pub(crate) expected: Vec<(&'static str, Vec<&'static str>)>,
}
