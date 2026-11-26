//! Write an execution header for command progress output.

use crate::build_progress::_helpers::progress_text::HEADER_OPTIONS;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::execution_header_text::execution_header_text;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Write an execution header for command progress output.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn write_execution_header(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "write_execution_header", options, HEADER_OPTIONS)?;
    let command: String = arguments.value("command")?.extract()?;
    let target: Option<String> = arguments.value("target")?.extract()?;
    let concurrency: i64 = arguments.value("concurrency")?.extract()?;
    let style: CliStyle = CliStyle {
        use_color: arguments.value("use_color")?.extract()?,
    };
    let engines: Option<String> = match arguments.passed("engines")? {
        true => arguments.value("engines")?.extract()?,
        false => None,
    };
    let text: String = execution_header_text(
        style,
        (&command, target.as_deref(), concurrency),
        engines.as_deref(),
    );
    let stream: Bound<'_, PyAny> = arguments.value("stream")?;
    stream.call_method1("write", (text,))?;
    stream.call_method0("flush")?;
    Ok(())
}
