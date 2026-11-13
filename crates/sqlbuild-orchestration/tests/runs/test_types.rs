//! Expected queue outcomes for each real control-store backend.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::BuiltinRole;
use crate::runs::models::{RerunRequest, RunStatus};

pub(crate) use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct PersonRunPermissionTestCase {
    pub(crate) description: &'static str,
    pub(crate) role: BuiltinRole,
    pub(crate) settings_range: std::ops::Range<usize>,
    pub(crate) expected_allowed: bool,
}

pub(crate) struct QueueStateTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_claim_order: [u128; 3],
}

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

pub(crate) struct RerunExecutionTestCase {
    pub(crate) description: &'static str,
    pub(crate) request: RerunRequest,
    pub(crate) expected_applied: bool,
    pub(crate) expected_selection: BTreeSet<String>,
    pub(crate) expected_jobs: &'static [&'static str],
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
}

pub(crate) struct GroupChildLimitTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_claimed_together: usize,
    pub(crate) expected_jobs: BTreeSet<&'static str>,
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

/// Who runs an ad-hoc case: Admin, Developer, Operator, or a person with no role.
#[derive(Clone, Copy)]
pub(crate) enum AdhocPerson {
    Admin,
    Developer,
    Operator,
    Stranger,
}

/// The people the ad-hoc fixture provisions.
pub(crate) struct AdhocPeople {
    pub(crate) admin: Uuid,
    pub(crate) developer: Uuid,
    pub(crate) operator: Uuid,
    pub(crate) stranger: Uuid,
}

impl AdhocPeople {
    pub(crate) fn of(&self, person: AdhocPerson) -> Uuid {
        match person {
            AdhocPerson::Admin => self.admin,
            AdhocPerson::Developer => self.developer,
            AdhocPerson::Operator => self.operator,
            AdhocPerson::Stranger => self.stranger,
        }
    }
}

/// One person's command: what its dry run decides and what launching it does.
pub(crate) struct AdhocCommandTestCase {
    pub(crate) description: &'static str,
    pub(crate) person: AdhocPerson,
    pub(crate) command: &'static [&'static str],
    pub(crate) priority: i64,
    pub(crate) reason: &'static str,
    pub(crate) expected_check: serde_json::Value,
    pub(crate) expected_launch: Result<bool, u16>,
}

/// The CLI's planner reduced to exact node keys, `--full-refresh`, `--no-load-sources` and `--warehouse NAME`.
pub(crate) struct ExactKeyPlanner;

impl crate::runs::types::CommandPlanner for ExactKeyPlanner {
    fn plan(
        &self,
        command: &[String],
        (resources, jobs): (
            &[crate::runs::models::PlannerResource],
            &crate::runs::types::PublishedJobs,
        ),
    ) -> Result<crate::runs::models::CommandPlan, String> {
        let mut plan = crate::runs::models::CommandPlan {
            verb: command.first().cloned().unwrap_or_default(),
            selection: Default::default(),
            settings: Default::default(),
        };
        let mut excluded: std::collections::BTreeSet<String> = Default::default();
        let mut excluding = false;
        let mut tokens = command.iter().skip(1);
        while let Some(token) = tokens.next() {
            let keys: Vec<String> = match token.as_str() {
                "--full-refresh" => {
                    plan.settings
                        .insert("full_refresh".into(), serde_json::json!(true));
                    continue;
                }
                "--no-load-sources" => {
                    plan.settings
                        .insert("load_sources".into(), serde_json::json!(false));
                    continue;
                }
                "--warehouse" => {
                    let name = tokens.next().ok_or("--warehouse needs a name")?;
                    plan.settings
                        .insert("warehouse".into(), serde_json::json!(name));
                    continue;
                }
                "--select" | "--exclude" => {
                    excluding = token == "--exclude";
                    continue;
                }
                key if resources.iter().any(|resource| resource.key == key) => vec![key.to_owned()],
                other => match other.strip_prefix("job:").and_then(|name| jobs.get(name)) {
                    Some(keys) => keys.iter().cloned().collect(),
                    None => return Err(format!("{other} is not available for ad-hoc runs")),
                },
            };
            if excluding {
                excluded.extend(keys);
            } else {
                plan.selection.extend(keys);
            }
        }
        if plan.selection.is_empty() && !excluded.is_empty() {
            plan.selection = resources
                .iter()
                .map(|resource| resource.key.clone())
                .collect();
        }
        plan.selection.retain(|key| !excluded.contains(key));
        Ok(plan)
    }
}

pub(crate) struct NarrowingDenialTestCase {
    pub(crate) description: &'static str,
    pub(crate) select: &'static [&'static str],
    pub(crate) exclude: &'static [&'static str],
    pub(crate) expected_denied: bool,
}

pub(crate) struct JobSubsetTestCase {
    pub(crate) description: &'static str,
    pub(crate) job_key: &'static str,
    pub(crate) select: &'static [&'static str],
    pub(crate) exclude: &'static [&'static str],
    /// The frozen selection, or `None` when the request is refused as invalid.
    pub(crate) expected_selection: Option<&'static [&'static str]>,
}

pub(crate) struct NamedRerunTestCase {
    pub(crate) description: &'static str,
    pub(crate) nodes: &'static [&'static str],
    pub(crate) from_failure: bool,
    /// The member holds view and run.rerun on `orders` only; otherwise the administrator asks.
    pub(crate) by_member: bool,
    /// The rerun's frozen selection, or `None` when it is refused.
    pub(crate) expected_selection: Option<&'static [&'static str]>,
    pub(crate) expected_denied: bool,
}

pub(crate) struct RunNodeErrorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_message: &'static str,
}

pub(crate) struct JobDetailTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_order: [&'static str; 2],
}

#[derive(Debug)]
pub(crate) struct RetryChainTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Attempt numbers of the retries claimed, in order, while every run fails.
    pub(crate) expected_attempts: Vec<u8>,
    /// The wait a one-hour retry delay shows while the retry is queued.
    pub(crate) expected_delayed_attempt: u8,
}

pub(crate) struct RunLogRetentionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_artifacts: i64,
    pub(crate) expected_runs: i64,
}
