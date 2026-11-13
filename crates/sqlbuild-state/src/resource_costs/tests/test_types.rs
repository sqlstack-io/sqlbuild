use crate::resource_costs::models::RunCostAttempt;

pub(crate) struct RefreshTestCase {
    pub(crate) description: &'static str,
    pub(crate) latest: Vec<RunCostAttempt>,
    pub(crate) expected_runs: Vec<&'static str>,
}

pub(crate) struct ObservationIdTestCase {
    pub(crate) description: &'static str,
    pub(crate) attempt: i64,
    pub(crate) cluster_number: Option<i64>,
    pub(crate) expected_same_as_base: bool,
}
