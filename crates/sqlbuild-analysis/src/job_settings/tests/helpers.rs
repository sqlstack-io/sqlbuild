use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::assembly::project::types::ObjectKey;
use crate::graph::main::attach_jobs::attach_jobs;
use crate::graph::main::build_project_graph::build_project_graph;
use crate::graph::models::{GraphResource, JobSelection, ProjectGraph};
use crate::job_settings::errors::NodeJobSettingsError;
use crate::job_settings::main::node_job_settings::node_job_settings;
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
    node_job_settings(
        &graph_with_jobs(),
        (&texts(select), &texts(exclude)),
        &declared(),
    )
}
