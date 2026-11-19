//! Resume the dynamic models whose holds expired, in each state location a sync read.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::project::main::load_dynamic_config::load_dynamic_config;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// One state location's database and schema, and the pause table token its last quiet pass left.
pub type ResumeLocation = ((Option<String>, String), Option<String>);

/// Each resumed hold's run ID and dotted relation, after `[dynamic] heartbeat_grace` of silence,
/// and each location's new quiet token, in `locations` order.
pub fn resume_expired_dynamic_holds<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    project_dir: &Bound<'py, PyAny>,
    locations: &[ResumeLocation],
    recorded_by: &str,
) -> PyResult<(Vec<(String, String)>, Vec<Option<String>>)> {
    let py = adapter.py();
    let grace_seconds: u64 =
        configured(project_dir, load_dynamic_config)?.heartbeat_grace_seconds;
    let resume: Bound<'py, PyAny> = host_entry(py, ExecutorHost::ResumeExpiredDynamicHolds)?;
    let mut resumed: Vec<(String, String)> = Vec::new();
    let mut quiet_tokens: Vec<Option<String>> = Vec::new();
    for (location, quiet_token) in locations {
        let outcome: Bound<'py, PyAny> = call_with(
            &resume,
            &[
                ("adapter", adapter.clone()),
                ("connection", connection.clone()),
                ("location", location.clone().into_bound_py_any(py)?),
                (
                    "grace_seconds",
                    (grace_seconds as f64).into_bound_py_any(py)?,
                ),
                ("recorded_by", recorded_by.into_bound_py_any(py)?),
                ("quiet_token", quiet_token.clone().into_bound_py_any(py)?),
            ],
        )?;
        for hold in outcome.getattr("resumed")?.try_iter()? {
            let hold: Bound<'py, PyAny> = hold?;
            let relation: Bound<'py, PyAny> = hold.getattr("relation")?;
            let mut parts: Vec<String> = Vec::new();
            for part in ["database", "schema", "name"] {
                parts.extend(relation.getattr(part)?.extract::<Option<String>>()?);
            }
            resumed.push((hold.getattr("run_id")?.extract()?, parts.join(".")));
        }
        quiet_tokens.push(outcome.getattr("quiet_token")?.extract()?);
    }
    Ok((resumed, quiet_tokens))
}
