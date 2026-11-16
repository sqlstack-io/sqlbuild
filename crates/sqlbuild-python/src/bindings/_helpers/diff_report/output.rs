//! `sqb diff` output: the terminal summary, JSON documents, output publication and exit codes.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_cli::diff_report::main::render_diff_error_json::render_diff_error_json as error_document;
use sqlbuild_cli::diff_report::main::render_diff_json::render_diff_json;
use sqlbuild_cli::diff_report::main::render_diff_text::render_diff_text;
use sqlbuild_cli::diff_report::models::{
    DiffDocumentOptions, DiffErrorFacts, DiffExampleOptions, DiffModelFacts, DiffTextOptions,
};

use crate::bindings::_helpers::diff_report::facts::{diff_facts, json_of};
use sqlbuild_cli::diff_report::types::PhaseSeconds;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DEFAULT_MAX_VALUE_LENGTH: usize = 160;
const TEXT_OPTIONS: (&[&str], &[&str]) = (
    &[
        "result",
        "from_label",
        "to_label",
        "mode_label",
        "use_color",
        "verbose",
        "max_column_examples",
        "max_row_only_examples",
    ],
    &["max_value_length", "suppress_example_values", "outcome"],
);
const ERROR_OPTIONS: (&[&str], &[&str]) =
    (&["status", "code", "message"], &["help_text", "size_guard"]);

fn optional_string(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

fn phase_seconds(value: Bound<'_, PyAny>) -> PyResult<Option<Vec<(String, f64)>>> {
    if value.is_none() {
        return Ok(None);
    }
    let mapping: Bound<'_, PyDict> = value.downcast_into()?;
    let mut phases: Vec<(String, f64)> = Vec::new();
    for (phase, seconds) in mapping.iter() {
        phases.push((phase.extract()?, seconds.extract()?));
    }
    Ok(Some(phases))
}

/// The example options of explicit `max_value_length`/`suppress_example_values` arguments.
fn example_options(arguments: &KeywordArguments<'_>) -> PyResult<DiffExampleOptions> {
    Ok(DiffExampleOptions {
        max_value_length: if arguments.passed("max_value_length")? {
            {
                let limit: Bound<'_, PyAny> = arguments.value("max_value_length")?;
                if limit.is_none() {
                    None
                } else {
                    Some(limit.extract()?)
                }
            }
        } else {
            Some(DEFAULT_MAX_VALUE_LENGTH)
        },
        suppress_values: arguments.flag("suppress_example_values")?,
    })
}

fn document_options(
    arguments: &KeywordArguments<'_>,
    examples: DiffExampleOptions,
) -> PyResult<DiffDocumentOptions> {
    Ok(DiffDocumentOptions {
        from_label: arguments.value("from_label")?.extract()?,
        to_label: arguments.value("to_label")?.extract()?,
        examples,
        outcome: optional_string(arguments.value("outcome")?)?,
        phase_seconds: phase_seconds(arguments.value("phase_seconds")?)?,
    })
}

fn json_text(models: &[DiffModelFacts], options: &DiffDocumentOptions) -> PyResult<String> {
    render_diff_json(models, options).map_err(emit_error)
}

/// Render the concise terminal summary of a diff result.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_diff_output(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "render_diff_output", options, TEXT_OPTIONS)?;
    let models: Vec<DiffModelFacts> = diff_facts(&arguments.value("result")?)?;
    let document: DiffDocumentOptions = document_options(&arguments, example_options(&arguments)?)?;
    Ok(render_diff_text(
        &models,
        &document,
        &DiffTextOptions {
            mode_label: arguments.value("mode_label")?.extract()?,
            use_color: arguments.flag("use_color")?,
            verbose: arguments.flag("verbose")?,
            max_column_examples: arguments.value("max_column_examples")?.extract()?,
            max_row_only_examples: arguments.value("max_row_only_examples")?.extract()?,
        },
    ))
}

/// Render a machine-readable incomplete or execution-failure outcome.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_diff_error_json(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "render_diff_error_json", options, ERROR_OPTIONS)?;
    let size_guard: Bound<'_, PyAny> = arguments.value("size_guard")?;
    error_document(&DiffErrorFacts {
        status: arguments.value("status")?.extract()?,
        code: arguments.value("code")?.extract()?,
        message: arguments.value("message")?.extract()?,
        help: optional_string(arguments.value("help_text")?)?,
        size_guard: if size_guard.is_none() {
            None
        } else {
            Some(json_of(&size_guard)?)
        },
    })
    .map_err(emit_error)
}

fn mode_label(request: &Bound<'_, PyAny>) -> PyResult<String> {
    if native_field(request, "schema_only")?.is_truthy()? {
        return Ok("schema-only".to_owned());
    }
    let bounded: Bound<'_, PyAny> = native_field(request, "bounded")?;
    if bounded.is_truthy()? {
        return Ok(format!("bounded {}", bounded.str()?));
    }
    Ok("full".to_owned())
}

fn write_stdout(py: Python<'_>, text: &str) -> PyResult<()> {
    imported(py, "sys", "stdout")?.call_method1("write", (format!("{text}\n"),))?;
    Ok(())
}

/// Write the requested JSON file, then print the JSON document or the terminal summary.
pub(crate) fn publish_diff_output(
    request: &Bound<'_, PyAny>,
    (from_label, to_label): (&str, &str),
    (max_column_examples, max_row_only_examples): (i64, i64),
    (models, phase_seconds, outcome): (&[DiffModelFacts], Option<PhaseSeconds>, Option<String>),
) -> PyResult<()> {
    let py: Python<'_> = request.py();
    let max_value_length: Bound<'_, PyAny> = native_field(request, "max_value_length")?;
    let examples: DiffExampleOptions = DiffExampleOptions {
        max_value_length: if native_field(request, "full_example_values")?.is_truthy()? {
            None
        } else if max_value_length.is_truthy()? {
            Some(max_value_length.extract()?)
        } else {
            Some(DEFAULT_MAX_VALUE_LENGTH)
        },
        suppress_values: native_field(request, "suppress_example_values")?.is_truthy()?,
    };
    let document: DiffDocumentOptions = DiffDocumentOptions {
        from_label: from_label.to_owned(),
        to_label: to_label.to_owned(),
        examples,
        outcome,
        phase_seconds,
    };
    let path: Bound<'_, PyAny> = native_field(request, "json_output_path")?;
    if !path.is_none() {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("parents", true)?;
        keywords.set_item("exist_ok", true)?;
        native_field(&path, "parent")?.call_method("mkdir", (), Some(&keywords))?;
        let encoding: Bound<'_, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        path.call_method(
            "write_text",
            (format!("{}\n", json_text(models, &document)?),),
            Some(&encoding),
        )?;
    }
    if native_field(request, "json_output")?.is_truthy()? {
        return write_stdout(py, &json_text(models, &document)?);
    }
    let use_color: bool =
        !native_field(request, "no_color")?.is_truthy()? && stdout_supports_color(py)?;
    write_stdout(
        py,
        &render_diff_text(
            models,
            &document,
            &DiffTextOptions {
                mode_label: mode_label(request)?,
                use_color,
                verbose: native_field(request, "verbose")?.is_truthy()?,
                max_column_examples: usize::try_from(max_column_examples).unwrap_or_default(),
                max_row_only_examples: usize::try_from(max_row_only_examples).unwrap_or_default(),
            },
        ),
    )
}
