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

pub(crate) struct AdmissionParent {
    pub(crate) holds: &'static [&'static str],
    pub(crate) children: &'static [&'static [&'static str]],
}

pub(crate) struct GroupAdmissionTestCase {
    pub(crate) description: &'static str,
    pub(crate) limits: &'static [(&'static str, u32)],
    pub(crate) parents: &'static [AdmissionParent],
    pub(crate) expected_running: usize,
    pub(crate) reductions: &'static [&'static [(&'static str, u32)]],
}

pub(crate) struct GroupQueueTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) group_count: usize,
    pub(crate) node_count: usize,
    pub(crate) expected_max_claim_millis: u128,
}

pub(crate) struct MixedAdmissionTestCase {
    pub(crate) description: &'static str,
    pub(crate) physical_priority: i64,
    pub(crate) parent_priority: i64,
    pub(crate) expected_selections: [&'static str; 2],
}
