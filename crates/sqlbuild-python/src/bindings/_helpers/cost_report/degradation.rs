//! Cost telemetry that never changes a build result: failures become a logged warning.

use std::path::PathBuf;

use pyo3::exceptions::PyException;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTypeMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cost_report::main::format_cost_breakdown::format_cost_breakdown;
use sqlbuild_core::cost_report::models::BreakdownOptions;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::cost_report::attribution::report_cost_status;
use crate::bindings::_helpers::cost_report::publication::publish_resource_costs;
use crate::bindings::_helpers::cost_report::records::cost_run;
use crate::bindings::_helpers::cost_report::store;
use crate::bindings::_helpers::cost_telemetry::values::cost_aware;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const LOGGER_NAME: &str = "sqlbuild.cli.commands._helpers.cost.collection";
const COLLECTION_LOGGER_NAME: &str = "sqlbuild.cost";
const DEFAULT_RATE_SOURCE: &str = "default";
const CONFIGURED_RATE_SOURCE: &str = "configured";
const STATEMENT_LEDGER_PATH: [&str; 4] = ["target", "executions", "", "statements.jsonl"];
const BUILD_BREAKDOWN_LIMIT: usize = 5;
const FALLBACK_TERMINAL_COLUMNS: usize = 100;

/// Log the failure with its traceback, as `_LOGGER.exception` inside the handler did.
fn log_failure(py: Python<'_>, message: &str, error: &PyErr) -> PyResult<()> {
    log_to(py, LOGGER_NAME, message, error)
}

fn log_to(py: Python<'_>, logger_name: &str, message: &str, error: &PyErr) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = py
        .import("logging")?
        .call_method1("getLogger", (logger_name,))?;
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("exc_info", error.value(py))?;
    logger.call_method("exception", (message,), Some(&keywords))?;
    Ok(())
}

fn error_type(py: Python<'_>, error: &PyErr) -> PyResult<String> {
    Ok(error.get_type(py).name()?.to_string())
}

/// `_write_warning`: a warning line on the build's stream; its own failure is only logged.
fn warn(output_stream: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    warn_logging_to(output_stream, message, LOGGER_NAME)
}

fn warn_logging_to(
    output_stream: &Bound<'_, PyAny>,
    message: &str,
    logger_name: &str,
) -> PyResult<()> {
    let written: PyResult<()> = output_stream
        .call_method1("write", (format!("\nWARNING: {message}\n"),))
        .and_then(|_| output_stream.call_method0("flush"))
        .map(|_| ());
    match written {
        Ok(()) => Ok(()),
        Err(error) => log_to(
            output_stream.py(),
            logger_name,
            "automatic run-cost warning output failed",
            &error,
        ),
    }
}

/// Persist a run's cost artifact; any failure is logged and reported as a warning.
#[pyfunction]
#[pyo3(signature = (*, project_dir, record, output_stream, stage))]
fn persist_cost_run(
    project_dir: PathBuf,
    record: &Bound<'_, PyAny>,
    output_stream: &Bound<'_, PyAny>,
    stage: &str,
) -> PyResult<()> {
    let py: Python<'_> = record.py();
    let Err(error) = cost_run(record).and_then(|run| store::write(py, &project_dir, &run)) else {
        return Ok(());
    };
    log_failure(py, &format!("automatic run-cost {stage} failed"), &error)?;
    warn(
        output_stream,
        &format!(
            "Cost telemetry failed during {stage} ({}); build result unchanged.",
            error_type(py, &error)?
        ),
    )
}

fn breakdown_text(py: Python<'_>, record: &Bound<'_, PyAny>, use_color: bool) -> PyResult<String> {
    let terminal_width: usize = py
        .import("shutil")?
        .call_method1("get_terminal_size", ((FALLBACK_TERMINAL_COLUMNS, 24),))?
        .getattr("columns")?
        .extract()?;
    Ok(format_cost_breakdown(
        &cost_run(record)?,
        BreakdownOptions {
            use_color,
            limit: Some(BUILD_BREAKDOWN_LIMIT),
            terminal_width,
            show_run_id: false,
        },
    ))
}

/// Write the build's cost breakdown; an `Exception` while rendering becomes a warning.
#[pyfunction]
#[pyo3(signature = (*, record, output_stream, use_color))]
fn write_build_cost_breakdown(
    record: &Bound<'_, PyAny>,
    output_stream: &Bound<'_, PyAny>,
    use_color: bool,
) -> PyResult<()> {
    let py: Python<'_> = record.py();
    let written: PyResult<()> = breakdown_text(py, record, use_color)
        .and_then(|text| output_stream.call_method1("write", (format!("\n{text}\n"),)))
        .and_then(|_| output_stream.call_method0("flush"))
        .map(|_| ());
    let Err(error) = written else {
        return Ok(());
    };
    if !error.is_instance_of::<PyException>(py) {
        return Err(error);
    }
    log_failure(py, "automatic run-cost output failed", &error)?;
    warn(
        output_stream,
        &format!(
            "Cost telemetry failed during output rendering ({}); build result unchanged.",
            error_type(py, &error)?
        ),
    )
}

/// One build's cost record with `cost` as its summary.
fn cost_record<'py>(
    finalization: &Bound<'py, PyAny>,
    summary: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = finalization.py();
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for name in [
        "run_id",
        "adapter_name",
        "target_name",
        "build_status",
        "started_at",
        "completed_at",
    ] {
        fields.push((name, native_field(finalization, name)?));
    }
    fields.push(("cost", summary));
    fields.push((
        "had_executable_work",
        native_field(finalization, "had_executable_work")?,
    ));
    new_record_with(py, RecordId::CostRunRecord, &fields)
}

fn summary<'py>(
    py: Python<'py>,
    status: Bound<'py, PyAny>,
    (config, rate_source): (&Bound<'py, PyAny>, &str),
    message: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(py, ExecutorHost::RunCostSummary)?,
        &[
            ("status", status),
            ("usd_per_credit", native_field(config, "usd_per_credit")?),
            ("rate_source", PyString::new(py, rate_source).into_any()),
            ("message", message),
        ],
    )
}

fn persist<'py>(
    finalization: &Bound<'py, PyAny>,
    record: &Bound<'py, PyAny>,
    stage: &str,
) -> PyResult<()> {
    persist_cost_run(
        native_field(finalization, "project_dir")?.extract()?,
        record,
        &native_field(finalization, "output_stream")?,
        stage,
    )
}

/// Record the run's cost in the warehouse state schema; any failure is logged and warned.
fn publish<'py>(
    finalization: &Bound<'py, PyAny>,
    record: &Bound<'py, PyAny>,
    (config, rate_source): (&Bound<'py, PyAny>, &str),
) -> PyResult<()> {
    let py: Python<'py> = finalization.py();
    let rate_source: Bound<'py, PyAny> = PyString::new(py, rate_source).into_any();
    let published: PyResult<()> = cost_run(record).and_then(|run| {
        publish_resource_costs(
            finalization,
            &run,
            (&native_field(config, "usd_per_credit")?, &rate_source),
        )
    });
    let Err(error) = published else {
        return Ok(());
    };
    log_failure(
        py,
        "automatic run-cost warehouse publication failed",
        &error,
    )?;
    warn(
        &native_field(finalization, "output_stream")?,
        &format!(
            "Cost telemetry failed during warehouse publication ({}); build result unchanged.",
            error_type(py, &error)?
        ),
    )
}

/// `_persist_collection_failure`: log, warn and persist a failed-collection record.
fn collection_failure<'py>(
    finalization: &Bound<'py, PyAny>,
    (config, rate_source): (&Bound<'py, PyAny>, &str),
    stage: &str,
    error: &PyErr,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = finalization.py();
    log_to(
        py,
        COLLECTION_LOGGER_NAME,
        &format!("automatic run-cost {stage} failed"),
        error,
    )?;
    let error_type: String = error_type(py, error)?;
    warn_logging_to(
        &native_field(finalization, "output_stream")?,
        &format!("Cost telemetry failed during {stage} ({error_type}); build result unchanged."),
        COLLECTION_LOGGER_NAME,
    )?;
    let status: Bound<'py, PyAny> =
        host::entry(py, ExecutorHost::CostStatus)?.getattr("COLLECTION_FAILED")?;
    report_cost_status(&status)?;
    let message: Bound<'py, PyAny> = PyString::new(
        py,
        &format!("Cost telemetry failed during {stage} ({error_type})."),
    )
    .into_any();
    let record: Bound<'py, PyAny> = cost_record(
        finalization,
        summary(py, status, (config, rate_source), message)?,
    )?;
    persist(finalization, &record, "failure artifact persistence")?;
    publish(finalization, &record, (config, rate_source))?;
    Ok(record)
}

fn collected_summary<'py>(
    finalization: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    (config, rate_source): (&Bound<'py, PyAny>, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = finalization.py();
    let run_id: Bound<'py, PyAny> = native_field(finalization, "run_id")?;
    let mut ledger: Bound<'py, PyAny> = native_field(finalization, "project_dir")?;
    for part in STATEMENT_LEDGER_PATH {
        ledger = if part.is_empty() {
            ledger.div(&run_id)?
        } else {
            ledger.div(part)?
        };
    }
    call_with(
        &native_field(adapter, "collect_run_cost")?,
        &[
            (
                "connection_config",
                native_field(finalization, "connection_config")?,
            ),
            (
                "target_database",
                native_field(finalization, "target_database")?,
            ),
            ("run_id", run_id),
            ("started_at", native_field(finalization, "started_at")?),
            ("completed_at", native_field(finalization, "completed_at")?),
            ("statement_ledger_path", ledger),
            ("usd_per_credit", native_field(config, "usd_per_credit")?),
            ("rate_source", PyString::new(py, rate_source).into_any()),
        ],
    )
}

/// Collect, persist, and render without allowing cost failures to affect a build.
#[pyfunction]
pub(crate) fn finalize_build_cost<'py>(
    finalization: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = finalization.py();
    let adapter: Bound<'py, PyAny> = native_field(finalization, "adapter")?;
    if !cost_aware(&adapter)? {
        return Ok(py.None().into_bound(py));
    }
    let config: Bound<'py, PyAny> = native_field(finalization, "config")?;
    let rate_source: String = if native_field(&config, "usd_per_credit_is_default")?.is_truthy()? {
        DEFAULT_RATE_SOURCE.to_owned()
    } else {
        CONFIGURED_RATE_SOURCE.to_owned()
    };
    let rate: (&Bound<'py, PyAny>, &str) = (&config, &rate_source);
    let capability: Bound<'py, PyAny> = match adapter.call_method0("cost_capability") {
        Ok(capability) => capability,
        Err(error) => {
            return collection_failure(finalization, rate, "capability detection", &error);
        }
    };
    if capability.eq(host::entry(py, ExecutorHost::CostCapability)?.getattr("NONE")?)? {
        return Ok(py.None().into_bound(py));
    }
    let cost: Bound<'py, PyAny> = if native_field(finalization, "collect")?.is_truthy()? {
        match collected_summary(finalization, &adapter, rate) {
            Ok(summary) => summary,
            Err(error) => return collection_failure(finalization, rate, "collection", &error),
        }
    } else {
        summary(
            py,
            native_field(finalization, "cost_status")?,
            rate,
            native_field(finalization, "cost_message")?,
        )?
    };
    report_cost_status(&native_field(&cost, "status")?)?;
    let record: Bound<'py, PyAny> = cost_record(finalization, cost)?;
    persist(finalization, &record, "artifact persistence")?;
    if native_field(finalization, "collect")?.is_truthy()? {
        publish(finalization, &record, rate)?;
    }
    if !native_field(finalization, "render")?.is_truthy()? {
        return Ok(record);
    }
    write_build_cost_breakdown(
        &record,
        &native_field(finalization, "output_stream")?,
        native_field(finalization, "use_color")?.is_truthy()?,
    )?;
    Ok(record)
}

/// Render a previously finalized record without affecting native build output.
pub(crate) fn render_build_cost(
    record: &Bound<'_, PyAny>,
    output_stream: &Bound<'_, PyAny>,
    use_color: bool,
) -> PyResult<()> {
    if record.is_none() {
        return Ok(());
    }
    write_build_cost_breakdown(record, output_stream, use_color)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(finalize_build_cost, module)?)?;
    module.add_function(wrap_pyfunction!(persist_cost_run, module)?)?;
    module.add_function(wrap_pyfunction!(write_build_cost_breakdown, module)?)
}
