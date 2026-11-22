//! Expected queue outcomes for each real control-store backend.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::BuiltinRole;
use crate::runs::models::{RerunRequest, RunOrigin, RunStatus};

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

/// Every test's publications share one cache; snapshot IDs and scopes are unique per test.
static EXACT_KEY_PUBLICATIONS: crate::runs::models::PublicationCache =
    crate::runs::models::PublicationCache::new();

impl crate::runs::types::CommandPlanner for ExactKeyPlanner {
    fn prepare(
        &self,
        resources: Vec<crate::runs::models::PlannerResource>,
        jobs: crate::runs::models::PublishedJobs,
    ) -> std::sync::Arc<dyn crate::runs::types::PreparedPublication> {
        std::sync::Arc::new(ExactKeyPublication { resources, jobs })
    }

    fn publications(&self) -> &crate::runs::models::PublicationCache {
        &EXACT_KEY_PUBLICATIONS
    }
}

/// One publication as [`ExactKeyPlanner`] prepared it.
#[derive(Debug)]
pub(crate) struct ExactKeyPublication {
    resources: Vec<crate::runs::models::PlannerResource>,
    jobs: crate::runs::models::PublishedJobs,
}

impl crate::runs::types::PreparedPublication for ExactKeyPublication {
    /// Exact keys only: each token names one published key, and an unknown one is refused with its position.
    fn evaluate(
        &self,
        select: &str,
        exclude: &str,
    ) -> Result<Vec<String>, Box<crate::runs::models::SelectionProblem>> {
        let known = |field: crate::runs::models::SelectionField, text: &str| {
            let mut keys: std::collections::BTreeSet<String> = Default::default();
            for token in text.split(' ').filter(|token| !token.is_empty()) {
                if !self.resources.iter().any(|resource| resource.key == token) {
                    let start = text.find(token).unwrap_or_default();
                    return Err(Box::new(crate::runs::models::SelectionProblem {
                        code: Some("S007".into()),
                        message: format!("unknown selector name '{token}'"),
                        help: None,
                        location: Some(crate::runs::models::SelectionLocation {
                            field,
                            token: token.to_owned(),
                            start: u32::try_from(start).unwrap_or_default(),
                            end: u32::try_from(start + token.len()).unwrap_or_default(),
                        }),
                        suggestions: Vec::new(),
                        refused: None,
                    }));
                }
                keys.insert(token.to_owned());
            }
            Ok(keys)
        };
        let selected = known(crate::runs::models::SelectionField::Select, select)?;
        let excluded = known(crate::runs::models::SelectionField::Exclude, exclude)?;
        let selected = if select.trim().is_empty() {
            self.resources
                .iter()
                .map(|resource| resource.key.clone())
                .collect()
        } else {
            selected
        };
        Ok(selected.difference(&excluded).cloned().collect())
    }

    /// One entry per published kind.
    fn vocabulary(&self) -> crate::runs::models::SelectionVocabulary {
        let mut kinds: std::collections::BTreeMap<String, u64> = Default::default();
        for resource in &self.resources {
            let kind = resource.key.split_once(':').map_or("", |(kind, _)| kind);
            *kinds.entry(kind.to_owned()).or_default() += 1;
        }
        crate::runs::models::SelectionVocabulary {
            kinds: kinds
                .into_iter()
                .map(|(value, count)| crate::runs::models::VocabularyEntry { value, count })
                .collect(),
            ..Default::default()
        }
    }

    fn plan(&self, command: &[String]) -> Result<crate::runs::models::CommandPlan, String> {
        let (resources, jobs) = (&self.resources, &self.jobs);
        let mut plan = crate::runs::models::CommandPlan {
            verb: command.first().cloned().unwrap_or_default(),
            selection: Default::default(),
            settings: Default::default(),
            jobs: Default::default(),
            job_settings: Default::default(),
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
                "--load" | "--reload" => {
                    let name = if token == "--load" {
                        "load_sources"
                    } else {
                        "reload"
                    };
                    plan.settings.insert(name.into(), serde_json::json!(true));
                    continue;
                }
                "--vars" => {
                    let text = tokens.next().ok_or("--vars needs a JSON object")?;
                    let vars: serde_json::Value =
                        serde_json::from_str(text).map_err(|error| error.to_string())?;
                    plan.settings.insert("vars".into(), vars);
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
                other => match other
                    .strip_prefix("job:")
                    .and_then(|name| jobs.selections.get(name))
                {
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
        // As the CLI's planner: a load is a build of its sources with loading on and no tests or audits.
        if plan.verb == "load" {
            for name in ["run_tests", "run_audits"] {
                plan.settings.insert(name.into(), serde_json::json!(false));
            }
            if !plan.settings.contains_key("reload") {
                plan.settings
                    .insert("load_sources".into(), serde_json::json!(true));
            }
        }
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

/// A rerun of an ad-hoc build that pinned `nightly` to orders and inventory and `weekly` to inventory.
pub(crate) struct PinnedRerunTestCase {
    pub(crate) description: &'static str,
    /// The source run's ID, unique per case.
    pub(crate) source_id: u128,
    pub(crate) nodes: &'static [&'static str],
    /// Each pinned job's nodes in the rerun's frozen request.
    pub(crate) expected_jobs: &'static [(&'static str, &'static [&'static str])],
}

/// A member's permissions, the jobs an ad-hoc build pins with the build-wide settings they give, and what launching it needs.
pub(crate) struct PinnedPermissionTestCase {
    pub(crate) description: &'static str,
    /// The roles document generation that grants the member `permissions`.
    pub(crate) generation: i64,
    pub(crate) permissions: &'static [&'static str],
    /// Each pinned job: its name, nodes and settings as JSON.
    pub(crate) jobs: &'static [(&'static str, &'static [&'static str], &'static str)],
    pub(crate) job_settings: &'static str,
    /// The dry run's `[action, allowed, reason]` for each permission, as JSON.
    pub(crate) expected_permissions: &'static str,
    /// The launch, or the reason it is refused.
    pub(crate) expected_launch: Result<(), &'static str>,
}

/// The build-wide settings a pinned job gave an ad-hoc build of `model:customers`, and what its rerun writes.
pub(crate) struct PinnedWriteSetTestCase {
    pub(crate) description: &'static str,
    pub(crate) source_id: u128,
    pub(crate) job_settings: &'static str,
    pub(crate) expected_write_set: &'static [&'static str],
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

/// What a member of a group does once the job it depends on has failed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MemberReaction {
    /// It runs, and the worker skips these nodes, each because of the failed node named.
    Runs(Vec<(&'static str, &'static str)>),
    /// The group ends it without running: its nodes are skipped because of the failed node.
    Skipped,
}

pub(crate) struct GroupDependencyTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The group's `dependency_failure` document: each member whose policy is not `partial`.
    pub(crate) dependency_failure: serde_json::Value,
    pub(crate) expected_marts: MemberReaction,
    pub(crate) expected_inventory: MemberReaction,
}

pub(crate) struct GroupMemberRetryTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The failing member's `retry_delay_seconds`.
    pub(crate) delay_seconds: u32,
    /// The member events the group run's timeline shows, in order.
    pub(crate) expected_messages: &'static [&'static str],
}

pub(crate) struct RoundTripGroupTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each member's jobs it waits for, as publication freezes them.
    pub(crate) expected_waits: serde_json::Value,
    /// The order the group launches its members in.
    pub(crate) expected_order: &'static [&'static str],
}

pub(crate) struct ProjectLimitTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) project_limit: u32,
    pub(crate) server_cap: Option<u32>,
    pub(crate) expected_running_per_location: usize,
}

pub(crate) struct WaitReasonTestCase {
    pub(crate) description: &'static str,
    /// Each node's upstream nodes.
    pub(crate) deps: &'static [(&'static str, &'static [&'static str])],
    pub(crate) jobs: serde_json::Value,
    pub(crate) waits: serde_json::Value,
    pub(crate) expected_reasons: serde_json::Value,
}

pub(crate) struct DeclarationCheckTestCase {
    pub(crate) description: &'static str,
    pub(crate) jobs: serde_json::Value,
    pub(crate) groups: serde_json::Value,
    pub(crate) expected_error: &'static str,
}

/// Lookups of `(location and target, snapshot)` by small numbers, and which ones built their value.
pub(crate) struct CacheLookupTestCase {
    pub(crate) description: &'static str,
    pub(crate) lookups: &'static [(u8, u8)],
    pub(crate) expected_built: &'static [bool],
}

/// Two first lookups of one snapshot, the first still building when the second arrives.
pub(crate) struct ConcurrentLookupTestCase {
    pub(crate) description: &'static str,
    pub(crate) first_value: u8,
    pub(crate) second_value: u8,
    /// The value each lookup got, how many builds ran and whether both share one value.
    pub(crate) expected_outcome: (u8, u8, usize, bool),
}

/// A lookup whose build fails, then another of the same snapshot.
pub(crate) struct FailedBuildTestCase {
    pub(crate) description: &'static str,
    pub(crate) failure: &'static str,
    pub(crate) retried_value: u8,
    /// The failed lookup's error and the retried lookup's value.
    pub(crate) expected_outcome: (Option<&'static str>, Option<u8>),
}

/// A command checked before and after a publication adds the node it names.
pub(crate) struct NewerPublicationTestCase {
    pub(crate) description: &'static str,
    pub(crate) command: &'static [&'static str],
    pub(crate) added_node: &'static str,
    pub(crate) expected_before_error: &'static str,
    pub(crate) expected_after_selection: &'static [&'static str],
}

/// A selection's count, page of keys, next offset and refusal message.
pub(crate) type ExpectedSelection = (
    u64,
    &'static [&'static str],
    Option<u32>,
    Option<&'static str>,
);

/// One person's selection request: the count, page, next offset and refusal message it returns, or the HTTP status refusing it.
pub(crate) struct SelectionRequestTestCase {
    pub(crate) description: &'static str,
    pub(crate) person: AdhocPerson,
    pub(crate) select: &'static str,
    pub(crate) exclude: &'static str,
    pub(crate) limit: Option<u32>,
    pub(crate) expected_outcome: Result<ExpectedSelection, u16>,
}

/// A queued run: its ID, priority and each pool it takes as `(pool, amount, limit)`.
pub(crate) type PooledRun = (u128, i64, &'static [(&'static str, u32, u32)]);

pub(crate) struct PoolAmountTestCase {
    pub(crate) description: &'static str,
    pub(crate) runs: &'static [PooledRun],
    pub(crate) caps: &'static [(&'static str, u32)],
    pub(crate) expected_claimed: &'static [u128],
    pub(crate) expected_slots: &'static [(&'static str, i64)],
}

pub(crate) struct GroupMemberSettingsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each member job's `ignore_audit_failures` setting and the audit failures its run ignores.
    pub(crate) members: [(&'static str, bool, u32); 2],
    pub(crate) expected_group_status: RunStatus,
    pub(crate) expected_group_ignored: u32,
}

pub(crate) struct NodeEventResourcesTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each node and check event's type, resource kind and resource name, in the order they happened.
    pub(crate) expected_named: [(&'static str, &'static str, &'static str); 4],
}

pub(crate) struct NodeOfTestCase {
    pub(crate) description: &'static str,
    /// The node identity columns a run's events read selects from an event's payload.
    pub(crate) fields: &'static [(&'static str, &'static str)],
    pub(crate) expected_node: Option<(&'static str, &'static str)>,
}

/// A store upgraded from before 0.245, whose stored run requests still carry `concurrency_groups`.
pub(crate) struct StoredRequestTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The source's claim, its retry's origin, its person rerun's status and the rerun's cancellation.
    pub(crate) expected_outcomes: (bool, RunOrigin, RunStatus, RunStatus),
}

/// One run node as the run's node list reads it back.
pub(crate) type NodeResultRow = (
    &'static str,
    &'static str,
    Option<&'static str>,
    (Option<i64>, Option<i64>, Option<i64>),
    Option<&'static str>,
);

pub(crate) struct NodeResultsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each node's key, status, materialization, (rows affected, table rows, rows so far) and why it changed nothing.
    pub(crate) expected_nodes: &'static [NodeResultRow],
    pub(crate) expected_not_run: i64,
}

pub(crate) struct LaunchEventTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) container_id: Option<&'static str>,
    pub(crate) expected_message: &'static str,
}
