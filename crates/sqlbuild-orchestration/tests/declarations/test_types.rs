pub(crate) struct GroupWaitsTestCase {
    pub(crate) description: &'static str,
    /// Each node and the nodes it reads.
    pub(crate) lineage: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member job and the nodes it selects.
    pub(crate) jobs: Vec<(&'static str, Vec<&'static str>)>,
    /// Each member and the members it waits for.
    pub(crate) expected: Vec<(&'static str, Vec<&'static str>)>,
}
