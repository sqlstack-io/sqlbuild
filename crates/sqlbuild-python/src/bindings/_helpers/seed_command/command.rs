//! `sqb seed`: compile the selected seeds, load them, and report.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::seed_command::main::render_seed_completion::render_seed_completion;
use sqlbuild_core::seed_command::main::render_seed_ready::render_seed_ready;
use sqlbuild_core::seed_command::main::render_seed_row::render_seed_row;
use sqlbuild_core::seed_command::main::seed_exit_code::seed_exit_code;
use sqlbuild_core::seed_command::models::SeedRow;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::build_progress::progress::current_native_progress_projector;
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_text::write_execution_header;
use crate::bindings::_helpers::command_runtime::concurrency::resolve_concurrency;
use crate::bindings::_helpers::command_runtime::invocation::resolve_command_invocation;
use crate::bindings::_helpers::compile_driver::pipeline_entries::build_static_pipeline_result;
use crate::bindings::_helpers::compile_driver::plan_result::connection_with_hooks;
use crate::bindings::_helpers::compile_driver::static_context::compile_static_command_context;
use crate::bindings::_helpers::compiled_project::snapshot_execution::{VerifiedRun, WorkerBuild};
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_entries::command_plans::build_seed_command_plan;
use crate::bindings::_helpers::planner_entries::command_plans::read_selected_seed_fingerprints;
use crate::bindings::_helpers::records::construct::{new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::result_output::documents::format_seed_execution_json;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;

const SUCCESS_STATUS: &str = "success";
const FAILED_STATUS: &str = "failed";
const EXECUTION_COMMAND: &str = "sqb seed";

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn status_value(result: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(&native_field(result, "status")?, "value")?.extract()
}

/// Forward positional connection timing to a reporter that takes keywords.
#[pyclass(module = "sqlbuild._native", frozen)]
struct SeedConnectionRelay {
    reporter: Py<PyAny>,
}

#[pymethods]
impl SeedConnectionRelay {
    fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.forward(
            py,
            "on_connection_complete",
            (connection_count, elapsed_seconds),
        )
    }

    fn on_connection_error(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.forward(
            py,
            "on_connection_error",
            (connection_count, elapsed_seconds),
        )
    }
}

impl SeedConnectionRelay {
    fn forward(
        &self,
        py: Python<'_>,
        method: &str,
        (connection_count, elapsed_seconds): (i64, f64),
    ) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("connection_count", connection_count)?;
        options.set_item("elapsed_seconds", elapsed_seconds)?;
        self.reporter
            .bind(py)
            .call_method(method, (), Some(&options))?;
        Ok(())
    }
}

fn connection_relay<'py>(
    reporter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, SeedConnectionRelay>> {
    Bound::new(
        reporter.py(),
        SeedConnectionRelay {
            reporter: reporter.clone().unbind(),
        },
    )
}

/// Write each finished seed's row and its execution event.
#[pyclass(module = "sqlbuild._native", frozen)]
struct SeedCompletionRelay {
    worker: Option<Arc<WorkerBuild>>,
    stream: Py<PyAny>,
    use_color: bool,
    ordinals: HashMap<String, usize>,
    total: usize,
    event_writer: Py<PyAny>,
    plan: Py<PyAny>,
    projector: Option<Py<PyAny>>,
}

#[pymethods]
impl SeedCompletionRelay {
    fn on_seed_complete(&self, py: Python<'_>, result: Bound<'_, PyAny>) -> PyResult<()> {
        let name: String = native_field(&result, "seed_name")?.extract()?;
        if let Some(worker) = &self.worker {
            worker.completed(&result, &format!("seed:{name}"))?;
        }
        let mut result: Bound<'_, PyAny> = result;
        if let Some(projector) = &self.projector {
            let duration: Bound<'_, PyAny> = call_with(
                &native_field(projector.bind(py), "consume_resource_terminal")?,
                &[
                    ("resource_name", text(py, &name)),
                    ("resource_id", text(py, &format!("seed:{name}"))),
                ],
            )?;
            if !duration.is_none() {
                let milliseconds: Bound<'_, PyAny> =
                    py.import("builtins")?.getattr("int")?.call1((duration,))?;
                let changes: Bound<'_, PyDict> = PyDict::new(py);
                changes.set_item("duration_ms", milliseconds)?;
                result = replaced_keywords(&result, &changes)?;
            }
        }
        let ordinal: usize = *self
            .ordinals
            .get(&name)
            .ok_or_else(|| PyKeyError::new_err(name.clone()))?;
        let style: CliStyle = CliStyle {
            use_color: self.use_color,
        };
        let row: SeedRow = SeedRow {
            name,
            ordinal,
            total: self.total,
            succeeded: status_value(&result)? == SUCCESS_STATUS,
            duration_ms: native_field(&result, "duration_ms")?.extract()?,
            error_message: native_field(&result, "error_message")?.extract()?,
            error_code: native_field(&result, "error_code")?.extract()?,
            error_help: native_field(&result, "error_help")?.extract()?,
        };
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        stream.call_method1("write", (render_seed_row(&row, style),))?;
        stream.call_method0("flush")?;
        call_with(
            &self.event_writer.bind(py).getattr("write_build_result")?,
            &[
                ("result", result),
                ("plan", self.plan.bind(py).clone()),
                ("command", text(py, "seed")),
            ],
        )?;
        Ok(())
    }
}

/// Compile the selected seeds, reading their stored fingerprints when any are selected.
fn compile_seeds<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let stream: Bound<'py, PyAny> = native_field(invocation, "progress_stream")?;
    let adapter: Bound<'py, PyAny> = native_field(invocation, "adapter")?;
    let connection_config: Bound<'py, PyAny> = native_field(invocation, "connection_config")?;
    let discovered: Bound<'py, PyAny> = native_field(invocation, "discovered_inputs")?;
    let connection_progress: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", native_field(invocation, "adapter_name")?),
                    ("stream", stream.clone()),
                    ("use_color", native_field(invocation, "use_color")?),
                ],
            )?),
        )?,
    )?
    .into_any();
    stream.call_method1("write", ("\n",))?;
    stream.call_method0("flush")?;
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        native_field(invocation, "effective_project_dir")?,
        discovered.clone(),
    )?;
    let options: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[
            ("selected_target", native_field(request, "selected_target")?),
            ("select", native_field(request, "select")?),
            ("exclude", native_field(request, "exclude")?),
            ("connection_config", connection_config.clone()),
            ("cli_vars", native_field(request, "cli_vars")?),
            ("external_sql_reference_resolver", resolver),
        ],
    )?;
    let context: Bound<'py, PyAny> = compile_static_command_context(Some(&keywords(
        py,
        &[
            ("discovered_inputs", discovered),
            ("adapter", adapter.clone()),
            ("options", options),
        ],
    )?))?;
    let project: Bound<'py, PyAny> = native_field(&context, "project")?;
    let scope: Bound<'py, PyAny> = native_field(&context, "scope")?;
    let selected_keys: Bound<'py, PyAny> = native_field(&scope, "selected_keys")?;
    let mut selected: Vec<Bound<'py, PyAny>> = Vec::new();
    for seed in native_field(&project, "seeds")?.try_iter()? {
        let seed: Bound<'py, PyAny> = seed?;
        if selected_keys.contains(native_field(&seed, "key")?)? {
            selected.push(seed);
        }
    }
    let fingerprints: Bound<'py, PyAny> = if selected.is_empty() {
        PyDict::new(py).into_any()
    } else {
        let relay: Bound<'py, SeedConnectionRelay> = connection_relay(&connection_progress)?;
        let hooks: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::ConnectionHooks)?,
            &[
                (
                    "on_connection_start",
                    native_field(&connection_progress, "on_connection_start")?,
                ),
                (
                    "on_connection_complete",
                    native_field(&relay, "on_connection_complete")?,
                ),
                (
                    "on_connection_error",
                    native_field(&relay, "on_connection_error")?,
                ),
            ],
        )?;
        let connection: Bound<'py, PyAny> =
            connection_with_hooks(&adapter, &connection_config, &hooks)?;
        let read: PyResult<Bound<'py, PyAny>> = read_selected_seed_fingerprints(
            &adapter,
            &connection,
            &PyTuple::new(py, &selected)?.into_any(),
        )
        .map(Bound::into_any);
        let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (connection,));
        let read: Bound<'py, PyAny> = read?;
        closed?;
        read
    };
    let plan_output: Bound<'py, PyAny> = build_seed_command_plan(
        &project,
        &scope,
        &native_field(&context, "relations")?,
        Some(fingerprints),
    )?;
    build_static_pipeline_result(
        py,
        Some(keywords(
            py,
            &[("context", context), ("plan_output", plan_output)],
        )?),
    )
}

fn seed_names(pipeline: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    native_field(&native_field(pipeline, "plan_output")?, "seed_entries")?
        .try_iter()?
        .map(|entry| native_field(&entry?, "name")?.extract())
        .collect()
}

/// Write the ready header, the selected seeds and the execution header.
fn write_header(
    invocation: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
    concurrency: i64,
) -> PyResult<()> {
    let py: Python<'_> = invocation.py();
    let stream: Bound<'_, PyAny> = native_field(invocation, "progress_stream")?;
    let use_color: Bound<'_, PyAny> = native_field(invocation, "use_color")?;
    let style: CliStyle = CliStyle {
        use_color: use_color.is_truthy()?,
    };
    stream.call_method1("write", (render_seed_ready(&seed_names(pipeline)?, style),))?;
    let header: Bound<'_, PyDict> = PyDict::new(py);
    header.set_item("stream", &stream)?;
    header.set_item("command", EXECUTION_COMMAND)?;
    header.set_item("target", py.None())?;
    header.set_item("concurrency", concurrency)?;
    header.set_item("use_color", use_color)?;
    write_execution_header(py, Some(header))?;
    Ok(())
}

/// Load the planned seeds, writing one row per finished seed.
fn load_seeds<'py>(
    invocation: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
    (concurrency, worker): (i64, Option<Arc<WorkerBuild>>),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = invocation.py();
    let stream: Bound<'py, PyAny> = native_field(invocation, "progress_stream")?;
    let use_color: Bound<'py, PyAny> = native_field(invocation, "use_color")?;
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let names: Vec<String> = seed_names(pipeline)?;
    let ordinals: HashMap<String, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index + 1))
        .collect();
    let event_writer: Bound<'py, PyAny> =
        Bound::new(py, ExecutionEventWriter::new(py, None)?)?.into_any();
    let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
    let projector: Option<Py<PyAny>> = if projector.is_none() {
        None
    } else {
        let order: Bound<'py, PyDict> = PyDict::new(py);
        for (index, name) in names.iter().enumerate() {
            order.set_item(name, index + 1)?;
        }
        call_with(
            &native_field(&projector, "configure_resources")?,
            &[
                ("ordinals", order.into_any()),
                ("total", PyInt::new(py, names.len()).into_any()),
            ],
        )?;
        Some(projector.unbind())
    };
    let completion: Bound<'py, SeedCompletionRelay> = Bound::new(
        py,
        SeedCompletionRelay {
            worker,
            stream: stream.clone().unbind(),
            use_color: use_color.is_truthy()?,
            ordinals,
            total: names.len(),
            event_writer: event_writer.clone().unbind(),
            plan: plan.clone().unbind(),
            projector,
        },
    )?;
    let connection_progress: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", native_field(invocation, "adapter_name")?),
                    ("stream", stream),
                    (
                        "blank_line_after_complete",
                        PyBool::new(py, true).to_owned().into_any(),
                    ),
                    ("use_color", use_color),
                ],
            )?),
        )?,
    )?
    .into_any();
    let relay: Bound<'py, SeedConnectionRelay> = connection_relay(&connection_progress)?;
    let loaded: PyResult<Vec<Bound<'py, PyAny>>> = (|| {
        call_with(
            &host::entry(py, ExecutorHost::RunSeedPipeline)?,
            &[
                ("plan", plan),
                (
                    "connection_config",
                    native_field(invocation, "connection_config")?,
                ),
                ("adapter", native_field(invocation, "adapter")?),
                ("max_concurrency", PyInt::new(py, concurrency).into_any()),
                ("run_id", native_field(&project, "run_id")?),
                (
                    "query_change_tracking",
                    native_field(
                        &native_field(&project, "settings")?,
                        "query_change_tracking",
                    )?,
                ),
                (
                    "on_seed_complete",
                    native_field(&completion, "on_seed_complete")?,
                ),
                (
                    "on_connection_start",
                    native_field(&connection_progress, "on_connection_start")?,
                ),
                (
                    "on_connection_complete",
                    native_field(&relay, "on_connection_complete")?,
                ),
                (
                    "on_connection_error",
                    native_field(&relay, "on_connection_error")?,
                ),
            ],
        )?
        .try_iter()?
        .collect()
    })();
    let closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let results: Vec<Bound<'py, PyAny>> = loaded?;
    closed?;
    Ok(results)
}

/// Run one `sqb seed` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_seed_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    run_seed(py, request, None)
}

/// Run one `sqb seed` request; a claimed run's seeds come from its verified inputs and stay in its write set.
pub(crate) fn run_seed<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
    verified: Option<VerifiedRun<'_, 'py>>,
) -> PyResult<i32> {
    let (worker, discovered) = verified.unzip();
    let invocation: Bound<'_, PyAny> = resolve_command_invocation(
        request,
        &record_class(py, RecordId::SeedInvocation)?,
        discovered,
    )?;
    let pipeline: Bound<'_, PyAny> = compile_seeds(request, &invocation)?;
    if let Some(worker) = &worker {
        let seeds: Vec<String> = seed_names(&pipeline)?
            .into_iter()
            .map(|name| format!("seed:{name}"))
            .collect();
        worker.enforce_writers(py, &seeds)?;
        for seed in &seeds {
            worker.start_key(py, seed)?;
        }
    }
    let concurrency: i64 = resolve_concurrency(
        &native_field(request, "concurrency")?,
        &native_field(&invocation, "discovered_inputs")?,
        &native_field(request, "selected_target")?,
    )?
    .limit
    .max(1);
    write_header(&invocation, &pipeline, concurrency)?;
    let started: Instant = Instant::now();
    let results: Vec<Bound<'_, PyAny>> = load_seeds(&invocation, &pipeline, (concurrency, worker))?;
    let elapsed: f64 = started.elapsed().as_secs_f64();
    let mut passed: usize = 0;
    let mut failed: usize = 0;
    for result in &results {
        match status_value(result)?.as_str() {
            SUCCESS_STATUS => passed += 1,
            FAILED_STATUS => failed += 1,
            _ => {}
        }
    }
    let stream: Bound<'_, PyAny> = native_field(&invocation, "progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: native_field(&invocation, "use_color")?.is_truthy()?,
    };
    stream.call_method1(
        "write",
        (render_seed_completion(
            (passed, failed, results.len()),
            elapsed,
            style,
        ),),
    )?;
    stream.call_method0("flush")?;
    let payload: Bound<'_, PyAny> = PyString::new(
        py,
        &format_seed_execution_json(
            py,
            PyTuple::new(py, &results)?.into_any(),
            native_field(&pipeline, "plan_output")?,
        )?,
    )
    .into_any();
    write_execution_json_output(
        py,
        payload,
        native_field(request, "json_output")?.extract()?,
        native_field(request, "json_output_path")?,
    )?;
    Ok(seed_exit_code(failed))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_seed_command, module)?)?;
    Ok(())
}
