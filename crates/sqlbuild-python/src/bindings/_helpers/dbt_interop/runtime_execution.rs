//! Execute dbt through the subprocess boundary and finalize results in artifact order.

use crate::bindings::_helpers::dbt_interop::cli::parse_dbt_ls_json_lines;
use crate::bindings::_helpers::dbt_interop::event_stream::execute_dbt_json_event_stream;
use crate::bindings::_helpers::dbt_interop::execution::dbt_execution_ls_argv;
use crate::bindings::_helpers::dbt_interop::runtime_results::{append_missing, result_ids};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyInt, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::models::TransientStatusReporter;
use sqlbuild_python_base::presentation_text::models::PythonCliStyle;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_output::dbt_events::main::render_dbt_node_result::render_dbt_node_result;
use std::collections::HashSet;
use std::time::Instant;

const OPTIONS: (&[&str], &[&str]) = (
    &[
        "runner",
        "options",
        "merged_argv",
        "progress_stream",
        "stdout_stream",
        "use_color",
    ],
    &[
        "skip_message",
        "expected_nodes",
        "on_node_result",
        "on_progress",
    ],
);

fn write(stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    stream.call_method1("write", (text,))?;
    stream.call_method0("flush")?;
    Ok(())
}

fn expected_total<'py>(
    arguments: &KeywordArguments<'py>,
    argv: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = argv.py();
    let ls = pyo3::IntoPyObjectExt::into_bound_py_any(
        dbt_execution_ls_argv(argv.clone().extract::<Vec<String>>()?),
        py,
    )?;
    if ls.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let status = TransientStatusReporter::new(
        py,
        &arguments.value("progress_stream")?,
        arguments.value("use_color")?.extract::<bool>()?,
        true,
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    status.call_method1("start", ("Resolving dbt execution selection...",))?;
    let start = Instant::now();
    let result = call_with(
        &arguments.value("runner")?.getattr("invoke")?,
        &[
            (
                "argv",
                py.import("builtins")?.getattr("tuple")?.call1((ls,))?,
            ),
            ("cwd", arguments.value("options")?.getattr("project_dir")?),
        ],
    );
    call_with(
        &native_field(&status, "complete")?,
        &[(
            "message",
            PyString::new(
                py,
                &format!(
                    "Resolved dbt execution selection. ({:.2}s)",
                    start.elapsed().as_secs_f64()
                ),
            )
            .into_any(),
        )],
    )?;
    let result = result?;
    if native_field(&result, "returncode")?.extract::<i64>()? != 0 {
        return Ok(py.None().into_bound(py));
    }
    let nodes = parse_dbt_ls_json_lines(py, &native_field(&result, "stdout")?.extract::<String>()?)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    Ok(PyInt::new(py, nodes.len()?).into_any())
}

fn progress(arguments: &KeywordArguments<'_>, message: &str) -> PyResult<()> {
    let callback = arguments.value("on_progress")?;
    if !callback.is_none() {
        callback.call1((message,))?;
    }
    Ok(())
}

/// Execute selected dbt work and backfill results absent from its JSON event stream.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn execute_dbt_commands<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments = KeywordArguments::read(py, "execute_dbt_commands", options, OPTIONS)?;
    let argv = arguments.value("merged_argv")?;
    let output = arguments.value("progress_stream")?;
    let style = Bound::new(py, PythonCliStyle::new(&arguments.value("use_color")?)?)
        .map(Bound::into_any)?;
    if argv.is_none() {
        let message = arguments.value("skip_message")?;
        let message = if message.is_none() {
            PyString::new(py, "Skipping dbt: no dbt work selected.").into_any()
        } else {
            message
        };
        write(
            &output,
            &format!(
                "{}\n",
                style
                    .call_method1("muted", (message,))?
                    .extract::<String>()?
            ),
        )?;
        return new_record_with(
            py,
            RecordId::DbtCommandExecutionResult,
            &[("returncode", PyInt::new(py, 0).into_any())],
        );
    }
    let name: String = py
        .import("pathlib")?
        .getattr("Path")?
        .call1((argv.get_item(0)?,))?
        .getattr("name")?
        .extract()?;
    let detail = if argv.len()? >= super::constants::COMMAND_AND_SUBCOMMAND_ARGUMENT_COUNT {
        format!("{name} {}", argv.get_item(1)?.extract::<String>()?)
    } else {
        name
    };
    write(
        &output,
        &format!(
            "{}  {}\n\n",
            style
                .call_method1("dbt_execution_label", ("dbt execution",))?
                .extract::<String>()?,
            style
                .call_method1("muted", (detail,))?
                .extract::<String>()?
        ),
    )?;
    let expected_nodes = arguments.value("expected_nodes")?;
    let expected: Option<HashSet<String>> = if expected_nodes.is_none() {
        None
    } else {
        Some(result_ids(&expected_nodes)?)
    };
    let total = if let Some(ids) = &expected {
        PyInt::new(py, ids.len()).into_any()
    } else {
        expected_total(&arguments, &argv)?
    };
    let dbt_options = arguments.value("options")?;
    let mut target = native_field(&dbt_options, "target_path")?;
    let project_dir = native_field(&dbt_options, "project_dir")?;
    if target.is_none() && !project_dir.is_none() {
        target = project_dir.call_method1("__truediv__", ("target",))?;
    }
    let (returncode, streamed): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        execute_dbt_json_event_stream(
            py,
            Some(keywords(
                py,
                &[
                    ("argv", argv),
                    ("cwd", project_dir),
                    ("stream", arguments.value("stdout_stream")?),
                    ("use_color", arguments.value("use_color")?),
                    ("target_path", target.clone()),
                    ("display_total", total.clone()),
                    ("on_node_result", arguments.value("on_node_result")?),
                ],
            )?),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
        .extract()?;
    progress(&arguments, "Finalizing dbt run...")?;
    let streamed_ids = result_ids(&streamed)?;
    let mut filtered: Vec<Bound<'py, PyAny>> = Vec::new();
    for result in streamed.try_iter()? {
        let result = result?;
        let id: String = native_field(&result, "unique_id")?.extract()?;
        if expected.as_ref().is_none_or(|ids| ids.contains(&id)) {
            filtered.push(result);
        }
    }
    let results = append_missing(
        &PyTuple::new(py, filtered)?.into_any(),
        &target,
        expected.as_ref(),
    )?;
    let mut appended: Vec<Bound<'py, PyAny>> = Vec::new();
    for result in results.try_iter()? {
        let result = result?;
        if !streamed_ids.contains(&native_field(&result, "unique_id")?.extract::<String>()?) {
            appended.push(result);
        }
    }
    let count = results.len()? - appended.len();
    for (offset, result) in appended.into_iter().enumerate() {
        let rendered = render_dbt_node_result(
            py,
            Some(keywords(
                py,
                &[
                    ("result", result.clone()),
                    ("use_color", native_field(&style, "use_color")?),
                    (
                        "display_index",
                        PyInt::new(py, count + offset + 1).into_any(),
                    ),
                    (
                        "display_total",
                        if total.is_none() {
                            PyInt::new(py, results.len()?).into_any()
                        } else {
                            total.clone()
                        },
                    ),
                ],
            )?),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        write(
            &arguments.value("stdout_stream")?,
            &rendered.extract::<String>()?,
        )?;
        let callback = arguments.value("on_node_result")?;
        if !callback.is_none() {
            callback.call1((result,))?;
        }
    }
    progress(&arguments, "Finalized dbt run.")?;
    new_record_with(
        py,
        RecordId::DbtCommandExecutionResult,
        &[("returncode", returncode), ("node_results", results)],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execute_dbt_commands, module)?)
}
