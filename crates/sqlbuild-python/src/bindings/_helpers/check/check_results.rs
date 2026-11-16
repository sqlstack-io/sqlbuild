//! Python check results as grouped rows and as the `sqb check --json` payload.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::check_command::main::check_group_header::check_group_header;
use sqlbuild_core::check_command::main::check_group_label::check_group_label;
use sqlbuild_core::check_command::main::check_json::check_json;
use sqlbuild_core::check_command::main::render_check_row::render_check_row;
use sqlbuild_core::check_command::models::{CheckGrouping, CheckResultText, CheckRow, CheckStatus};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::json::models::JsonValue;

use crate::bindings::_helpers::result_output::terminal_index::current_terminal_event_index;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::main::progress::current_native_progress_projector::current_native_progress_projector;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::result_output::main::json_value::json_value;

const WARN_SEVERITY: &str = "warn";

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

/// The metadata as JSON, through `json.dumps` when it holds values only that serializer accepts.
fn metadata_json(metadata: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if let Some(value) = json_value(metadata)? {
        return Ok(value);
    }
    let py: Python<'_> = metadata.py();
    let json: Bound<'_, PyAny> = py.import("json")?.into_any();
    let round_trip: Bound<'_, PyAny> =
        json.call_method1("loads", (json.call_method1("dumps", (metadata,))?,))?;
    json_value(&round_trip)?
        .ok_or_else(|| PyValueError::new_err("check metadata is not JSON serializable"))
}

/// One executor `PythonCheckExecutionResult` as the JSON output renders it, metadata included.
fn check_result_json_text(result: &Bound<'_, PyAny>) -> PyResult<CheckResultText> {
    Ok(CheckResultText {
        metadata: metadata_json(&native_field(result, "metadata")?)?,
        ..check_result_text(result)?
    })
}

/// One executor `PythonCheckExecutionResult` as the rows and summary render it, without metadata.
pub(crate) fn check_result_text(result: &Bound<'_, PyAny>) -> PyResult<CheckResultText> {
    let severity: String = native_field(result, "severity")?
        .getattr("value")?
        .extract()?;
    let status: CheckStatus = if native_field(result, "passed")?.is_truthy()? {
        CheckStatus::Pass
    } else if severity == WARN_SEVERITY {
        CheckStatus::Warn
    } else {
        CheckStatus::Fail
    };
    Ok(CheckResultText {
        name: native_field(result, "node_name")?.extract()?,
        status,
        severity,
        message: optional_text(native_field(result, "message")?)?,
        error_message: optional_text(native_field(result, "error_message")?)?,
        metadata: JsonValue::Null,
        warning_messages: native_field(result, "warning_messages")?.extract()?,
    })
}

fn names(values: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// The output group of `name`, from the Python graph and the check's declared group.
fn grouping(
    name: &str,
    checks: &HashMap<String, Bound<'_, PyAny>>,
    python_graph: &Bound<'_, PyAny>,
) -> PyResult<CheckGrouping> {
    let group: Option<String> = match checks.get(name) {
        Some(check) => optional_text(native_field(check, "group")?)?,
        None => None,
    };
    if python_graph.is_none() {
        return Ok(CheckGrouping {
            dependencies: None,
            group,
        });
    }
    let py: Python<'_> = python_graph.py();
    let empty: Bound<'_, PyAny> = pyo3::types::PyTuple::empty(py).into_any();
    let mut dependencies: Vec<String> =
        names(native_field(python_graph, "upstream_deps")?.call_method1("get", (name, empty))?)?;
    let node: Bound<'_, PyAny> =
        native_field(python_graph, "nodes_by_name")?.call_method1("get", (name,))?;
    if !node.is_none() {
        for reference in native_field(&node, "sql_deps")?.try_iter()? {
            dependencies.push(native_field(&reference?, "name")?.extract()?);
        }
    }
    Ok(CheckGrouping {
        dependencies: Some(dependencies),
        group,
    })
}

/// Write grouped check rows, claiming each row's projected duration once its row is written.
pub(crate) fn write_results(
    stream: &Bound<'_, PyAny>,
    results: &[Bound<'_, PyAny>],
    (check_functions, python_graph): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    style: CliStyle,
) -> PyResult<()> {
    let py: Python<'_> = stream.py();
    let mut checks: HashMap<String, Bound<'_, PyAny>> = HashMap::new();
    for check in check_functions.try_iter()? {
        let check: Bound<'_, PyAny> = check?;
        checks.insert(native_field(&check, "name")?.extract()?, check);
    }
    let projector: Bound<'_, PyAny> = current_native_progress_projector(py)?;
    let mut groups: Vec<(String, Vec<CheckResultText>)> = Vec::new();
    for result in results {
        let text: CheckResultText = check_result_text(result)?;
        let label: String = check_group_label(&grouping(&text.name, &checks, python_graph)?);
        match groups.iter_mut().find(|(group, _)| *group == label) {
            Some((_, members)) => members.push(text),
            None => groups.push((label, vec![text])),
        }
    }
    stream.call_method1("write", ("\n",))?;
    for (group, members) in groups {
        stream.call_method1("write", (check_group_header(&group),))?;
        for result in members {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("resource_name", &result.name)?;
            options.set_item("resource_id", format!("check:{}", result.name))?;
            let duration_ms: Option<f64> = if projector.is_none() {
                None
            } else {
                projector
                    .call_method("resource_terminal_duration", (), Some(&options))?
                    .extract()?
            };
            let row: CheckRow = CheckRow {
                result,
                duration_ms,
            };
            stream.call_method1("write", (render_check_row(&row, style),))?;
            if !projector.is_none() {
                projector.call_method("consume_resource_terminal", (), Some(&options))?;
            }
        }
        stream.call_method1("write", ("\n",))?;
    }
    Ok(())
}

/// The JSON payload of the results the terminal event index saw finish, or of all of them.
pub(crate) fn results_json(results: &[Bound<'_, PyAny>]) -> PyResult<String> {
    let Some(first) = results.first() else {
        return check_json(&[]).map_err(|error| PyValueError::new_err(format!("{error:?}")));
    };
    let py: Python<'_> = first.py();
    let index: Bound<'_, PyAny> = current_terminal_event_index(py)?;
    let mut texts: Vec<CheckResultText> = Vec::new();
    for result in results {
        let name: Bound<'_, PyAny> = native_field(result, "node_name")?;
        if !index.is_none() {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("resource_name", &name)?;
            options.set_item(
                "resource_id",
                PyString::new(py, &format!("check:{}", name.str()?)),
            )?;
            if index
                .call_method("resource_terminal", (), Some(&options))?
                .is_none()
            {
                continue;
            }
        }
        texts.push(check_result_json_text(result)?);
    }
    check_json(&texts).map_err(|error| PyValueError::new_err(format!("{error:?}")))
}

/// Write human-readable check result rows.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_check_results(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "write_check_results",
        options,
        (
            &["stream", "results", "use_color"],
            &["check_functions", "python_graph"],
        ),
    )?;
    let results: Vec<Bound<'_, PyAny>> = arguments
        .value("results")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let check_functions: Bound<'_, PyAny> = match arguments.value("check_functions")? {
        functions if functions.is_none() => pyo3::types::PyTuple::empty(py).into_any(),
        functions => functions,
    };
    write_results(
        &arguments.value("stream")?,
        &results,
        (&check_functions, &arguments.value("python_graph")?),
        CliStyle {
            use_color: arguments.flag("use_color")?,
        },
    )
}

/// Format check execution JSON.
#[pyfunction]
#[pyo3(signature = (*, results))]
fn format_check_json(results: &Bound<'_, PyAny>) -> PyResult<String> {
    let results: Vec<Bound<'_, PyAny>> = results.try_iter()?.collect::<PyResult<_>>()?;
    results_json(&results)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_check_results, module)?)?;
    module.add_function(wrap_pyfunction!(format_check_json, module)?)?;
    Ok(())
}
