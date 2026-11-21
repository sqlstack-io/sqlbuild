/// One set dependency of the `report` asset and the end nodes it resolves to.
pub(super) struct EndNodesTestCase {
    pub(super) description: &'static str,
    /// `(kind, name, exclude)` of each `job(...)` or `tag(...)` dependency.
    pub(super) sets: &'static [(&'static str, &'static str, Option<&'static str>)],
    /// The job groups the catalog holds, by name and members.
    pub(super) groups: &'static [(&'static str, &'static [&'static str])],
    /// Each end node as errors name it, per set.
    pub(super) expected_ends: &'static [&'static [&'static str]],
}

/// A dependency that cannot resolve, and fragments its error must carry.
pub(super) struct FailureTestCase {
    pub(super) description: &'static str,
    /// The dependant's name and kind.
    pub(super) dependant: (&'static str, &'static str),
    pub(super) sets: &'static [(&'static str, &'static str, Option<&'static str>)],
    /// The job groups the catalog holds, by name and members.
    pub(super) groups: &'static [(&'static str, &'static [&'static str])],
    pub(super) expected_fragments: &'static [&'static str],
}

/// The sets a node was last built with and has now, and the wait changes a plan lists.
pub(super) struct WaitChangesTestCase {
    pub(super) description: &'static str,
    /// `None` when the last build recorded no sets.
    pub(super) previous: Option<&'static [(&'static str, &'static [&'static str])]>,
    pub(super) current: &'static [(&'static str, &'static [&'static str])],
    pub(super) expected_lines: &'static [&'static str],
}
