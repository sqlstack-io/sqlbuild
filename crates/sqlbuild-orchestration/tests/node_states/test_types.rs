pub(crate) struct HeldChecksTestCase {
    pub(crate) description: &'static str,
    pub(crate) signals_during_read: usize,
    pub(crate) expected_held: Option<&'static str>,
}

pub(crate) struct NodeStatusTestCase {
    pub(crate) description: &'static str,
    pub(crate) node_key: &'static str,
    pub(crate) expected_status: crate::node_states::models::NodeBuildStatus,
    /// Hours after the fixture's start, or none.
    pub(crate) expected_status_at: Option<i64>,
}

pub(crate) struct LoaderAttemptTestCase {
    pub(crate) description: &'static str,
    /// Each attempt: the node key it was recorded under, its status and hours after the start.
    pub(crate) attempts: &'static [(&'static str, &'static str, i64)],
    pub(crate) expected_attempt: Option<(&'static str, i64)>,
}
