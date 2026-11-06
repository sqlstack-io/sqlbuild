//! Shared CLI text helpers as `_native` functions; styles are read from a `CliStyle`'s `use_color`.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyList, PyListMethods};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::cli_text::main::format_aligned_name_value::format_aligned_name_value as aligned;
use sqlbuild_core::cli_text::main::format_completion_line::format_completion_line as completion;
use sqlbuild_core::cli_text::main::format_count_noun::format_count_noun as count_noun;
use sqlbuild_core::cli_text::main::format_overflow_line::format_overflow_line;
use sqlbuild_core::cli_text::main::format_status_cell::format_status_cell as status_cell;
use sqlbuild_core::cli_text::main::format_summary_footer::format_summary_footer as footer;
use sqlbuild_core::cli_text::main::resolve_name_column_width::resolve_name_column_width as column_width;
use sqlbuild_core::cli_text::main::split_count_header::split_count_header;
use sqlbuild_core::cli_text::models::{AlignedRow, CliStyle, CompletionState, SummaryCount};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::records::fields::field as native_field;

const ALIGNED_OPTIONS: (&[&str], &[&str]) = (
    &["plain_name", "styled_name", "value", "name_column_width"],
    &["prefix"],
);
const OVERFLOW_OPTIONS: (&[&str], &[&str]) = (
    &["lines", "total_count", "visible_count", "indent", "options"],
    &[],
);
const DEFAULT_PREFIX: &str = "  ";
const FAIL_STATE: &str = "fail";
const WARN_STATE: &str = "warn";

/// The native style of a Python `CliStyle`.
pub(crate) fn style_of(style: &Bound<'_, PyAny>) -> PyResult<CliStyle> {
    Ok(CliStyle {
        use_color: native_field(style, "use_color")?.is_truthy()?,
    })
}

/// Format summary counts with semantic colors.
#[pyfunction]
#[pyo3(signature = (*, counts, use_color, elapsed = None))]
fn format_summary_footer(
    counts: Vec<(String, i64)>,
    use_color: bool,
    elapsed: Option<&str>,
) -> String {
    let counts: Vec<SummaryCount> = counts
        .into_iter()
        .map(|(label, value)| SummaryCount { label, value })
        .collect();
    footer(CliStyle { use_color }, &counts, elapsed)
}

/// Render a count with the singular noun for one and the plural noun otherwise.
#[pyfunction]
#[pyo3(signature = (*, count, singular, plural = None))]
fn format_count_noun(count: i64, singular: &str, plural: Option<&str>) -> String {
    count_noun(count, singular, plural)
}

/// Render a single-line completion summary: state glyph, label, trailing summary.
#[pyfunction]
#[pyo3(signature = (*, style, state, label, summary = None))]
fn format_completion_line(
    style: &Bound<'_, PyAny>,
    state: &str,
    label: &str,
    summary: Option<&str>,
) -> PyResult<String> {
    let state: CompletionState = match state {
        FAIL_STATE => CompletionState::Fail,
        WARN_STATE => CompletionState::Warn,
        _ => CompletionState::Ok,
    };
    Ok(completion(style_of(style)?, state, label, summary))
}

/// Render a fixed-width status cell padded on the plain text, not the ANSI text.
#[pyfunction]
#[pyo3(signature = (*, style, status, width = 6))]
fn format_status_cell(style: &Bound<'_, PyAny>, status: &str, width: usize) -> PyResult<String> {
    Ok(status_cell(style_of(style)?, status, width))
}

/// Format one aligned name/value row using a plain-text width basis.
#[pyfunction]
#[pyo3(signature = (**options))]
fn format_aligned_name_value(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "format_aligned_name_value", options, ALIGNED_OPTIONS)?;
    let prefix: String = if arguments.passed("prefix")? {
        arguments.value("prefix")?.extract()?
    } else {
        DEFAULT_PREFIX.to_owned()
    };
    let width: i64 = arguments.value("name_column_width")?.extract()?;
    Ok(aligned(&AlignedRow {
        plain_name: arguments.value("plain_name")?.extract()?,
        styled_name: arguments.value("styled_name")?.extract()?,
        value: arguments.value("value")?.extract()?,
        name_column_width: usize::try_from(width).unwrap_or(0),
        prefix,
    }))
}

/// Resolve a left-column width from the longest visible name.
#[pyfunction]
#[pyo3(signature = (*, names, min_width = 20))]
fn resolve_name_column_width(names: Vec<String>, min_width: usize) -> usize {
    column_width(&names, min_width)
}

/// Append a standard overflow hint when a section is truncated, and return the lines.
#[pyfunction]
#[pyo3(signature = (**options))]
fn append_overflow_line<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "append_overflow_line", options, OVERFLOW_OPTIONS)?;
    let lines: Bound<'py, PyList> = arguments.value("lines")?.downcast_into()?;
    let total: i64 = arguments.value("total_count")?.extract()?;
    let visible: i64 = arguments.value("visible_count")?.extract()?;
    let indent: String = arguments.value("indent")?.extract()?;
    let flag: String = arguments
        .value("options")?
        .getattr("overflow_flag")?
        .extract()?;
    if let Some(line) = format_overflow_line(total - visible, &indent, &flag) {
        lines.append(line)?;
    }
    Ok(lines)
}

/// A header style that renders `Title (count)` as a styled title and a dim count.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CountHeaderStyle {
    style: CliStyle,
    title_style: Option<Py<PyAny>>,
}

#[pymethods]
impl CountHeaderStyle {
    fn __call__(&self, py: Python<'_>, text: &str) -> PyResult<String> {
        let title = |title: &str| -> PyResult<String> {
            match &self.title_style {
                Some(styled) => styled.bind(py).call1((title,))?.extract(),
                None => Ok(self.style.section(title)),
            }
        };
        let Some((heading, count)) = split_count_header(text) else {
            return title(text);
        };
        Ok(format!(
            "{} {}",
            title(heading)?,
            self.style.muted(&format!("({count}"))
        ))
    }
}

/// Return a header style that renders `Title (count)` as a bold title and dim count.
#[pyfunction]
#[pyo3(signature = (*, style, title_style = None))]
fn count_header_style(
    style: &Bound<'_, PyAny>,
    title_style: Option<Py<PyAny>>,
) -> PyResult<CountHeaderStyle> {
    Ok(CountHeaderStyle {
        style: style_of(style)?,
        title_style,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CountHeaderStyle>()?;
    module.add_function(wrap_pyfunction!(format_summary_footer, module)?)?;
    module.add_function(wrap_pyfunction!(format_count_noun, module)?)?;
    module.add_function(wrap_pyfunction!(format_completion_line, module)?)?;
    module.add_function(wrap_pyfunction!(format_status_cell, module)?)?;
    module.add_function(wrap_pyfunction!(format_aligned_name_value, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_name_column_width, module)?)?;
    module.add_function(wrap_pyfunction!(append_overflow_line, module)?)?;
    module.add_function(wrap_pyfunction!(count_header_style, module)?)
}
