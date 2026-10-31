//! Expected queue outcomes for each real control-store backend.

use sqlbuild_orchestration::runs::models::RunStatus;

pub(crate) use crate::fault_types::TransactionFault;
use crate::store_test_types::Backend;

pub(crate) struct ScopedClaimTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_selected_run: u128,
    pub(crate) expected_unscoped_run: u128,
}

pub(crate) struct PriorityClaimsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_initial_claims: [u128; 2],
    pub(crate) expected_claim_events: usize,
    pub(crate) expected_terminal_events: usize,
}

pub(crate) struct GroupLimitTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_unblocked_run: u128,
}

pub(crate) struct BacklogClaimTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_independent_run: u128,
}

pub(crate) struct CoalescedRunTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_runs: usize,
}

pub(crate) struct ClaimFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_run: u128,
}

pub(crate) struct RunLifecycleTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: RunStatus,
}

pub(crate) struct RunMonitorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_statuses: [RunStatus; 5],
}

pub(crate) struct MonitorBacklogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) run_count: usize,
    pub(crate) expected_batches: [usize; 3],
}
