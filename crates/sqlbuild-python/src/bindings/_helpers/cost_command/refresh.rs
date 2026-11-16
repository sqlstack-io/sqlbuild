//! Best-effort delayed cost refresh of one pending or partial persisted run.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_core::cost_command::constants::{
    HISTORY_SELECTOR, LATEST_SELECTOR, REFRESHABLE_STATUSES,
};
use sqlbuild_core::cost_command::main::accepts_cost_refresh::accepts_cost_refresh;
use sqlbuild_core::cost_command::models::CostCoverage;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_target_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::cost_report::functions::read_cost_run;
use crate::bindings::_helpers::cost_report::functions::resolve_cost_run;
use crate::bindings::_helpers::cost_report::functions::write_cost_run;
use crate::bindings::_helpers::cost_telemetry::values::cost_aware;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;

/// Collection budget of an explicit `sqb cost` refresh, which a user waits for on purpose.
const REFRESH_BUDGET_SECONDS: f64 = 300.0;

fn coverage(summary: &Bound<'_, PyAny>) -> PyResult<CostCoverage> {
    Ok(CostCoverage {
        status: native_field(summary, "status")?
            .getattr("value")?
            .extract()?,
        expected_statement_count: native_field(summary, "expected_statement_count")?.extract()?,
        observed_statement_count: native_field(summary, "observed_statement_count")?.extract()?,
        query_count: native_field(summary, "query_count")?.extract()?,
        limitations: native_field(summary, "limitations")?
            .try_iter()?
            .map(|limitation| limitation?.extract())
            .collect::<PyResult<_>>()?,
    })
}

/// The persisted record the selector names: latest, an exact run id, or a unique prefix.
fn refresh_record<'py>(
    project_dir: &Bound<'py, PyAny>,
    selector: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let resolve = || {
        resolve_cost_run(
            py,
            project_dir.clone().extract::<std::path::PathBuf>()?,
            &selector.clone().extract::<String>()?,
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
    };
    if selector.extract::<String>()? == LATEST_SELECTOR {
        return resolve();
    }
    let exact: Bound<'py, PyAny> = read_cost_run(
        py,
        project_dir.clone().extract::<std::path::PathBuf>()?,
        &selector.clone().extract::<String>()?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    if !exact.is_none() {
        return Ok(exact);
    }
    resolve()
}

/// The target database the collection queries: the target's, else the connection's.
fn target_database<'py>(
    target_config: &Bound<'py, PyAny>,
    connection_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_config.py();
    if !target_config.is_none() {
        let database: Bound<'py, PyAny> = native_field(target_config, "database")?;
        if !database.is_none() {
            return Ok(database);
        }
    }
    let database: Bound<'py, PyAny> = mapping_get(connection_config, "database")?;
    if database.is_none() {
        return Ok(py.None().into_bound(py));
    }
    Ok(database.str()?.into_any())
}

fn refresh(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<()> {
    let selector: Bound<'_, PyAny> = native_field(request, "selector")?;
    if selector.extract::<String>()? == HISTORY_SELECTOR {
        return Ok(());
    }
    let requested: Bound<'_, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'_, PyAny> = if requested.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        requested
    };
    let record: Bound<'_, PyAny> = refresh_record(&project_dir, &selector)?;
    if record.is_none() {
        return Ok(());
    }
    let current: Bound<'_, PyAny> = native_field(&record, "cost")?;
    let status: String = native_field(&current, "status")?
        .getattr("value")?
        .extract()?;
    if !REFRESHABLE_STATUSES.contains(&status.as_str()) {
        return Ok(());
    }
    let target_name: Bound<'_, PyAny> = native_field(&record, "target_name")?;
    let discovered: Bound<'_, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    let target_config: Bound<'_, PyAny> = build_effective_target_config(
        &discovered.clone(),
        Some(target_name.clone()).filter(|value| !value.is_none()),
    )?;
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'_, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir)?;
    if !cost_aware(&adapter)? {
        return Ok(());
    }
    let none_capability: Bound<'_, PyAny> =
        host_entry(py, ExecutorHost::CostCapability)?.getattr("NONE")?;
    if adapter
        .call_method0("cost_capability")?
        .eq(none_capability)?
    {
        return Ok(());
    }
    let connection_config: Bound<'_, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered),
                ("project_dir", project_dir.clone()),
                ("selected_target", target_name),
            ],
        )?),
    )?;
    let run_id: Bound<'_, PyAny> = native_field(&record, "run_id")?;
    let ledger: Bound<'_, PyAny> = project_dir
        .call_method1("__truediv__", ("target",))?
        .call_method1("__truediv__", ("executions",))?
        .call_method1("__truediv__", (&run_id,))?
        .call_method1("__truediv__", ("statements.jsonl",))?;
    let summary: Bound<'_, PyAny> = call_with(
        &native_field(&adapter, "collect_run_cost")?,
        &[
            ("connection_config", connection_config.clone()),
            (
                "target_database",
                target_database(&target_config, &connection_config)?,
            ),
            ("run_id", run_id),
            ("started_at", native_field(&record, "started_at")?),
            ("completed_at", native_field(&record, "completed_at")?),
            ("statement_ledger_path", ledger),
            ("usd_per_credit", native_field(&current, "usd_per_credit")?),
            ("rate_source", native_field(&current, "rate_source")?),
            (
                "budget_seconds",
                REFRESH_BUDGET_SECONDS.into_pyobject(py)?.into_any(),
            ),
        ],
    )?;
    if accepts_cost_refresh(&coverage(&current)?, &coverage(&summary)?) != Some(true) {
        return Ok(());
    }
    let changes: Bound<'_, PyDict> = PyDict::new(py);
    changes.set_item("cost", summary)?;
    let refreshed: Bound<'_, PyAny> = replaced_keywords(&record, &changes)?;
    write_cost_run(py, project_dir.extract::<std::path::PathBuf>()?, &refreshed)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    Ok(())
}

/// Refresh one delayed record without making lookup depend on warehouse availability.
#[pyfunction]
pub(crate) fn refresh_pending_cost_run(py: Python<'_>, request: &Bound<'_, PyAny>) {
    let _ = refresh(py, request);
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(refresh_pending_cost_run, module)?)?;
    Ok(())
}
