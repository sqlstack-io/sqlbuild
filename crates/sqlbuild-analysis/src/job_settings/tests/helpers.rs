use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};

use crate::assembly::project::types::ObjectKey;
use crate::graph::main::attach_jobs::attach_jobs;
use crate::graph::main::build_project_graph::build_project_graph;
use crate::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use crate::graph::models::{GraphResource, JobSelection, ProjectGraph};
use crate::job_settings::_helpers::node_job_settings::node_job_settings;
use crate::job_settings::_helpers::node_values::node_values;
use crate::job_settings::errors::NodeJobSettingsError;
use crate::job_settings::main::resolve_job_settings::resolve_job_settings;
use crate::job_settings::models::{DeclaredRun, JobRetry, NodeJobSettings};

fn key(kind: &str, name: &str) -> ObjectKey {
    (kind.to_owned(), name.to_owned())
}

fn texts(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Staged orders feed orders and refunds; customers stands alone.
fn graph() -> ProjectGraph {
    let model = |name: &str, deps: &[&str]| GraphResource {
        key: key("model", name),
        deps: deps.iter().map(|dep| key("model", dep)).collect(),
        tags: Vec::new(),
        folder: None,
    };
    build_project_graph(&[
        model("stg_orders", &[]),
        model("orders", &["stg_orders"]),
        model("refunds", &["stg_orders"]),
        model("customers", &[]),
    ])
}

/// Jobs over the graph with their settings, and a group of two of them.
pub(super) fn declared() -> BTreeMap<String, DeclaredRun> {
    let job = |settings: Json, retry: Option<JobRetry>| DeclaredRun::Job {
        settings: settings
            .as_object()
            .map(|object| object.clone().into_iter().collect())
            .unwrap_or_default(),
        retry,
    };
    BTreeMap::from([
        (
            "orders_job".to_owned(),
            job(
                json!({"concurrency": 2, "run_tests": false, "full_refresh": true}),
                None,
            ),
        ),
        (
            "refunds_job".to_owned(),
            job(json!({"concurrency": 4}), None),
        ),
        (
            "staging_job".to_owned(),
            job(json!({"run_tests": true}), None),
        ),
        (
            "quiet_job".to_owned(),
            job(json!({"run_tests": false}), None),
        ),
        (
            "crm_job".to_owned(),
            job(json!({"vars": {"region": "eu"}}), None),
        ),
        (
            "crm_us_job".to_owned(),
            job(json!({"vars": {"region": "us"}}), None),
        ),
        (
            "retried_job".to_owned(),
            job(
                json!({}),
                Some(JobRetry {
                    attempts: 3,
                    delay_seconds: 60,
                }),
            ),
        ),
        (
            "retried_fast_job".to_owned(),
            job(
                json!({}),
                Some(JobRetry {
                    attempts: 2,
                    delay_seconds: 0,
                }),
            ),
        ),
        (
            "broken_job".to_owned(),
            job(json!({"full_refresh": true}), None),
        ),
        (
            "orders_wh_job".to_owned(),
            job(
                json!({"warehouse": "ORDERS_WH", "run_tests": false}),
                Some(JobRetry {
                    attempts: 2,
                    delay_seconds: 0,
                }),
            ),
        ),
        (
            "refunds_wh_job".to_owned(),
            job(json!({"warehouse": "REFUNDS_WH"}), None),
        ),
        (
            "daily_group".to_owned(),
            DeclaredRun::Group {
                members: texts(&["orders_job", "refunds_job"]),
            },
        ),
    ])
}

fn selections() -> BTreeMap<String, JobSelection> {
    let job = |select: &[&str]| JobSelection::Job {
        select: texts(select),
        exclude: Vec::new(),
    };
    BTreeMap::from([
        ("orders_job".to_owned(), job(&["+orders"])),
        ("refunds_job".to_owned(), job(&["+refunds"])),
        ("staging_job".to_owned(), job(&["stg_orders"])),
        ("quiet_job".to_owned(), job(&["stg_orders"])),
        ("crm_job".to_owned(), job(&["customers"])),
        ("crm_us_job".to_owned(), job(&["customers"])),
        ("retried_job".to_owned(), job(&["stg_orders"])),
        ("retried_fast_job".to_owned(), job(&["stg_orders"])),
        ("broken_job".to_owned(), job(&["missing_model"])),
        ("orders_wh_job".to_owned(), job(&["+orders"])),
        ("refunds_wh_job".to_owned(), job(&["+refunds"])),
        (
            "daily_group".to_owned(),
            JobSelection::Group {
                members: texts(&["orders_job", "refunds_job"]),
            },
        ),
    ])
}

/// The graph with the jobs attached, as a command that names a job resolves it.
pub(super) fn graph_with_jobs() -> ProjectGraph {
    attach_jobs(graph(), selections())
}

pub(super) fn strings(values: &[&str]) -> Vec<String> {
    texts(values)
}

/// What selecting `select` minus `exclude` gives each node of the graph's jobs.
pub(super) fn resolved_settings(
    select: &[&str],
    exclude: &[&str],
) -> Result<NodeJobSettings, NodeJobSettingsError> {
    let graph: ProjectGraph = graph_with_jobs();
    let token_nodes = |token: &str, exclude: &[String]| {
        let excluded: BTreeSet<ObjectKey> = resolve_selector_tokens(&graph, exclude)?
            .into_iter()
            .collect();
        let selected = resolve_selector_tokens(&graph, &[token.to_owned()])?;
        Ok(selected
            .into_iter()
            .filter(|key| !excluded.contains(key))
            .collect())
    };
    node_job_settings(
        &graph,
        (&texts(select), &texts(exclude)),
        &declared(),
        &token_nodes,
    )
}

/// Jobs `a_job`, `b_job` and `c_job` setting `name` to each of `values`, unset where `None`.
pub(super) fn jobs_setting(name: &str, values: [Option<Json>; 3]) -> BTreeMap<String, DeclaredRun> {
    ["a_job", "b_job", "c_job"]
        .into_iter()
        .zip(values)
        .map(|(job, value)| {
            let settings: BTreeMap<String, Json> = value
                .map(|value| (name.to_owned(), value))
                .into_iter()
                .collect();
            (
                job.to_owned(),
                DeclaredRun::Job {
                    settings,
                    retry: None,
                },
            )
        })
        .collect()
}

/// The `job:` selectors of `jobs`.
pub(super) fn job_selectors(jobs: &[&str]) -> Vec<String> {
    jobs.iter().map(|job| format!("job:{job}")).collect()
}

/// Whether naming `named`, `given` on the command line, conflicts on `name`, and the build takes it.
pub(super) fn resolves_build_wide(name: &str, named: &[&str], given: &[&str]) -> (bool, bool) {
    let declared = jobs_setting(name, [Some(json!("first")), Some(json!("second")), None]);
    let given: BTreeSet<String> = given.iter().map(|setting| (*setting).to_owned()).collect();
    resolve_job_settings(declared, (&job_selectors(named), &given), None)
        .map_or((true, false), |resolved| {
            (false, resolved.build.contains_key(name))
        })
}

/// Whether `a_job` and `b_job` giving one model `first` and `second` for `name` conflict.
pub(super) fn conflicts_on_a_node(name: &str) -> bool {
    let declared = jobs_setting(name, [Some(json!("first")), Some(json!("second")), None]);
    let model: ObjectKey = key("model", "orders");
    node_values(&model, &["a_job".to_owned(), "b_job".to_owned()], &declared).is_err()
}

/// The whole-run value of `name` when `a_job` and `b_job` set it to `values`.
pub(super) fn whole_run_value(name: &str, values: [Option<Json>; 3]) -> Option<Json> {
    let declared = jobs_setting(name, values);
    resolve_job_settings(
        declared,
        (&job_selectors(&["a_job", "b_job"]), &BTreeSet::new()),
        None,
    )
    .ok()
    .and_then(|resolved| resolved.whole_run.get(name).cloned())
}
