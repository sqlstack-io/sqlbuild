//! Resume the dynamic models whose holds expired, in each state location a sync read.

use crate::discovery::main::project::configured_dynamic_grace::configured_dynamic_grace;
use crate::native_host::types::{ResumeConnection, ResumeLocation, ResumedHolds};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Resume holds silent past `[dynamic] heartbeat_grace` over a session closed before returning.
pub fn resume_expired_dynamic_holds<'py>(
    (adapter, connection_config): ResumeConnection<'_, 'py>,
    project_dir: &Bound<'py, PyAny>,
    locations: &[ResumeLocation],
    recorded_by: &str,
) -> PyResult<ResumedHolds> {
    let connection: Bound<'py, PyAny> =
        native_field(adapter, "connect")?.call1((connection_config,))?;
    let resumed: PyResult<ResumedHolds> =
        resume_each((adapter, &connection), project_dir, locations, recorded_by);
    let closed: PyResult<Bound<'py, PyAny>> = native_field(adapter, "close")?.call1((&connection,));
    let resumed: ResumedHolds = resumed?;
    closed?;
    Ok(resumed)
}

fn resume_each<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    project_dir: &Bound<'py, PyAny>,
    locations: &[ResumeLocation],
    recorded_by: &str,
) -> PyResult<ResumedHolds> {
    let py = adapter.py();
    let grace_seconds: u64 = configured_dynamic_grace(project_dir)?;
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
        for hold in native_field(&outcome, "resumed")?.try_iter()? {
            let hold: Bound<'py, PyAny> = hold?;
            let relation: Bound<'py, PyAny> = native_field(&hold, "relation")?;
            let mut parts: Vec<String> = Vec::new();
            for part in ["database", "schema", "name"] {
                parts.extend(native_field(&relation, part)?.extract::<Option<String>>()?);
            }
            resumed.push((native_field(&hold, "run_id")?.extract()?, parts.join(".")));
        }
        quiet_tokens.push(native_field(&outcome, "quiet_token")?.extract()?);
    }
    Ok((resumed, quiet_tokens))
}
