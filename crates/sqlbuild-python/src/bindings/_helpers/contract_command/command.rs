//! `sqb contract`: compare selected contracts with one target, optionally adopt them.

use std::time::Instant;

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyString, PyTuple, PyTupleMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::contract_command::main::contract_exit_code::contract_exit_code;
use sqlbuild_core::contract_command::main::contract_progress_message::contract_progress_message;
use sqlbuild_core::contract_command::main::render_contract_json::render_contract_json;
use sqlbuild_core::contract_command::main::render_contract_text::render_contract_text;
use sqlbuild_core::contract_command::models::{
    ContractFindingText, ContractProgress, ContractReport, ContractResourceText,
};

use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::command_runtime::invocation::resolve_adapter_connection_context;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::pipeline_entries::build_project_graph_from_compiled_project;
use crate::bindings::_helpers::compile_driver::project::build_compiled_project;
use crate::bindings::_helpers::contract_command::write::{ContractWrite, write_contracts};
use crate::bindings::_helpers::planner_selection::selectors::select_project_graph;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::refactor::contract_compare::compare_contracts;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::native_host::main::connection_failure::connection_failure;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const GENERATE_ACTION: &str = "generate";

/// A `CliUserError` with its code.
fn user_error(py: Python<'_>, message: &str, code: &str) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", code)?;
        error_class(py, ErrorId::CliUser)?.call((message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.to_string()))
}

/// The selected graph, compiled against the source target, and its selected models and sources.
struct Selection<'py> {
    graph: Bound<'py, PyAny>,
    models: Bound<'py, PyTuple>,
    sources: Bound<'py, PyTuple>,
}

fn selected_of<'py>(
    resources: Bound<'py, PyAny>,
    selected: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    for resource in resources.try_iter()? {
        let resource: Bound<'py, PyAny> = resource?;
        if selected.contains(native_field(&resource, "key")?)? {
            kept.push(resource);
        }
    }
    PyTuple::new(resources.py(), kept)
}

fn select_resources<'py>(
    request: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<Selection<'py>> {
    let py: Python<'py> = request.py();
    let analysis: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompileAnalysisSelection,
        &[("no_cache", PyBool::new(py, true).to_owned().into_any())],
    )?;
    let project: Bound<'py, PyAny> = build_compiled_project(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", native_field(context, "adapter")?),
                ("selected_target", native_field(request, "from_target")?),
                ("cli_vars", native_field(request, "cli_vars")?),
                (
                    "resolved_connection",
                    native_field(context, "connection_config")?,
                ),
                ("analysis_selection", analysis),
            ],
        )?),
    )?;
    let graph: Bound<'py, PyAny> = build_project_graph_from_compiled_project(
        py,
        Some(keywords(py, &[("project", project)])?),
    )?;
    let selected: Bound<'py, PyAny> = select_project_graph(
        &graph.clone(),
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let compiled: Bound<'py, PyAny> = native_field(&graph, "project")?;
    let models: Bound<'py, PyTuple> = selected_of(native_field(&compiled, "models")?, &selected)?;
    let sources: Bound<'py, PyTuple> = selected_of(native_field(&compiled, "sources")?, &selected)?;
    if models.is_empty() && sources.is_empty() {
        return Err(user_error(
            py,
            "contract selection contains no models or sources",
            "C473",
        ));
    }
    Ok(Selection {
        graph,
        models,
        sources,
    })
}

/// Where progress goes and whether it is coloured.
struct Progress<'py> {
    stream: Bound<'py, PyAny>,
    use_color: bool,
}

impl<'py> Progress<'py> {
    fn status(&self) -> PyResult<Bound<'py, PyAny>> {
        status_reporter(&self.stream, self.use_color)
    }
}

fn exception(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyException>(py)
}

fn connection_timing(reporter: &Bound<'_, PyAny>, method: &str, started: Instant) -> PyResult<()> {
    let py: Python<'_> = reporter.py();
    call_with(
        &native_field(reporter, method)?,
        &[
            ("connection_count", PyInt::new(py, 1).into_any()),
            (
                "elapsed_seconds",
                PyFloat::new(py, started.elapsed().as_secs_f64()).into_any(),
            ),
        ],
    )?;
    Ok(())
}

/// Compare the selected contracts over one connection that is always closed.
fn compare<'py>(
    request: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
    selection: &Selection<'py>,
    progress: &Progress<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let adapter: Bound<'py, PyAny> = native_field(context, "adapter")?;
    let reporter: Bound<'py, PyAny> = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("adapter_name", native_field(context, "adapter_name")?),
                ("stream", progress.stream.clone()),
                (
                    "use_color",
                    PyBool::new(py, progress.use_color).to_owned().into_any(),
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let connection_started: Instant = Instant::now();
    reporter.call_method1("on_connection_start", (1,))?;
    let connection: Bound<'py, PyAny> =
        match adapter.call_method1("connect", (native_field(context, "connection_config")?,)) {
            Ok(connection) => connection,
            Err(error) => {
                if exception(py, &error) {
                    connection_timing(&reporter, "on_connection_error", connection_started)?;
                }
                return Err(connection_failure(&adapter, &error));
            }
        };
    let compared: PyResult<Bound<'py, PyAny>> = (|| {
        connection_timing(&reporter, "on_connection_complete", connection_started)?;
        let count: usize = selection.models.len() + selection.sources.len();
        let target: String = native_field(request, "from_target")?.str()?.to_string();
        let status: Bound<'py, PyAny> = progress.status()?;
        let started: Instant = Instant::now();
        status.call_method1(
            "start",
            (contract_progress_message(
                &ContractProgress::InspectionStart {
                    count,
                    target: target.clone(),
                },
            ),),
        )?;
        let evidence: Bound<'py, PyAny> = match compare_contracts(Some(&keywords(
            py,
            &[
                ("adapter", adapter.clone()),
                ("connection", connection.clone()),
                ("models", selection.models.clone().into_any()),
                ("sources", selection.sources.clone().into_any()),
            ],
        )?))
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
        {
            Ok(evidence) => evidence,
            Err(error) => {
                if exception(py, &error) {
                    status.call_method1(
                        "error",
                        (contract_progress_message(
                            &ContractProgress::InspectionFailed {
                                seconds: started.elapsed().as_secs_f64(),
                            },
                        ),),
                    )?;
                }
                return Err(error);
            }
        };
        let options: Bound<'py, PyDict> = PyDict::new(py);
        options.set_item(
            "message",
            contract_progress_message(&ContractProgress::InspectionComplete {
                count,
                target,
                seconds: started.elapsed().as_secs_f64(),
            }),
        )?;
        status.call_method("complete", (), Some(&options))?;
        Ok(evidence)
    })();
    let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (connection,));
    let evidence: Bound<'py, PyAny> = compared?;
    closed?;
    Ok(evidence)
}

/// Write the adopted declarations into the repository and report the files written.
fn write_updates<'py>(
    request: &Bound<'py, PyAny>,
    (project_dir, context): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (graph, result): (&Bound<'py, PyAny>, Bound<'py, PyAny>),
    progress: &Progress<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let status: Bound<'py, PyAny> = progress.status()?;
    let started: Instant = Instant::now();
    status.call_method1("start", ("Updating repository contracts...",))?;
    let write: ContractWrite<'py> = ContractWrite {
        project_dir: project_dir.clone(),
        overwrite: native_field(request, "overwrite")?,
        adapter: native_field(context, "adapter")?,
        resolved_connection: native_field(context, "connection_config")?,
        cli_vars: native_field(request, "cli_vars")?,
    };
    let updated: Bound<'py, PyAny> = match write_contracts(&write, graph, &result) {
        Ok(updated) => updated,
        Err(error) => {
            if exception(py, &error) {
                status.call_method1(
                    "error",
                    (contract_progress_message(&ContractProgress::WriteFailed {
                        seconds: started.elapsed().as_secs_f64(),
                    }),),
                )?;
            }
            return Err(error);
        }
    };
    let written: usize = native_field(&updated, "written_paths")?.len()?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item(
        "message",
        contract_progress_message(&ContractProgress::WriteComplete {
            count: written,
            seconds: started.elapsed().as_secs_f64(),
        }),
    )?;
    status.call_method("complete", (), Some(&options))?;
    Ok(updated)
}

fn finding_text(finding: Bound<'_, PyAny>) -> PyResult<ContractFindingText> {
    Ok(ContractFindingText {
        kind: native_field(&finding, "kind")?.str()?.to_string(),
        column: optional_text(native_field(&finding, "column_name")?)?,
        declared_type: optional_text(native_field(&finding, "declared_type")?)?,
        physical_type: optional_text(native_field(&finding, "physical_type")?)?,
        message: native_field(&finding, "message")?.str()?.to_string(),
    })
}

fn resource_text(item: Bound<'_, PyAny>) -> PyResult<ContractResourceText> {
    let mut parts: Vec<String> = Vec::new();
    for field in ["database", "schema", "relation"] {
        let value: Bound<'_, PyAny> = native_field(&item, field)?;
        if value.is_truthy()? {
            parts.push(value.str()?.to_string());
        }
    }
    Ok(ContractResourceText {
        resource_type: native_field(&item, "resource_type")?.str()?.to_string(),
        resource_name: native_field(&item, "resource_name")?.str()?.to_string(),
        relation: parts.join("."),
        findings: native_field(&item, "findings")?
            .try_iter()?
            .map(|finding| finding_text(finding?))
            .collect::<PyResult<_>>()?,
    })
}

fn report(result: &Bound<'_, PyAny>) -> PyResult<ContractReport> {
    Ok(ContractReport {
        from_target: native_field(result, "from_target")?.str()?.to_string(),
        resources: native_field(result, "evidence")?
            .try_iter()?
            .map(|item| resource_text(item?))
            .collect::<PyResult<_>>()?,
        written_paths: native_field(result, "written_paths")?
            .try_iter()?
            .map(|path| Ok(path?.str()?.to_string()))
            .collect::<PyResult<_>>()?,
    })
}

fn progress_for<'py>(request: &Bound<'py, PyAny>) -> PyResult<Progress<'py>> {
    let py: Python<'py> = request.py();
    let json_output: bool = native_field(request, "json_output")?.is_truthy()?;
    let stream: Bound<'py, PyAny> =
        py.import("sys")?
            .getattr(if json_output { "stderr" } else { "stdout" })?;
    let use_color: bool = !native_field(request, "no_color")?.is_truthy()?
        && !json_output
        && stdout_supports_color(py)?;
    Ok(Progress { stream, use_color })
}

/// Compare or adopt selected contracts from one read-only target.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_contract_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let action: String = native_field(request, "action")?.str()?.to_string();
    let generate_write: bool =
        action == GENERATE_ACTION && native_field(request, "write")?.is_truthy()?;
    if native_field(request, "overwrite")?.is_truthy()? && !generate_write {
        return Err(user_error(
            py,
            "contract --overwrite requires generate --write",
            "C471",
        ));
    }
    let requested_dir: Bound<'_, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'_, PyAny> = if requested_dir.is_truthy()? {
        requested_dir
    } else {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    };
    let discovered: Bound<'_, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    reject_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        &discovered.clone(),
    )?;
    let from_target: Bound<'_, PyAny> = native_field(request, "from_target")?;
    if !native_field(&discovered, "project_config")?
        .getattr("targets")?
        .contains(&from_target)?
    {
        return Err(user_error(
            py,
            &format!("unknown contract source target '{}'", from_target.str()?),
            "C472",
        ));
    }
    let context: Bound<'_, PyAny> = resolve_adapter_connection_context(
        &discovered,
        &project_dir,
        (py.None().into_bound(py), native_field(request, "cli_vars")?),
        true,
    )?;
    let selection: Selection<'_> = select_resources(request, &discovered, &context)?;
    let progress: Progress<'_> = progress_for(request)?;
    let evidence: Bound<'_, PyAny> = compare(request, &context, &selection, &progress)?;
    let mut result: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::ContractAdoptionResult,
        &[("from_target", from_target), ("evidence", evidence)],
    )?;
    if generate_write {
        result = write_updates(
            request,
            (&project_dir, &context),
            (&selection.graph, result),
            &progress,
        )?;
    }
    let report: ContractReport = report(&result)?;
    let rendered: String = if native_field(request, "json_output")?.is_truthy()? {
        render_contract_json(&report)
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?
    } else {
        render_contract_text(&report)
    };
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (PyString::new(py, &rendered),))?;
    Ok(contract_exit_code(report.difference_count()))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_contract_command, module)?)?;
    Ok(())
}
