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
