//! One dbt node row and its messages, rendered from a Python `DbtNodeExecutionResult`.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::dbt_events::main::render_dbt_node_row::render_dbt_node_row;
use sqlbuild_core::dbt_events::models::DbtRowPosition;
use sqlbuild_python_base::boundary::models::KeywordArguments;

use crate::dbt_events::_helpers::results::native_result;
use crate::dbt_events::constants::RENDER_OPTIONS;

/// One dbt node row and its messages.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn render_dbt_node_result(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "render_dbt_node_result", options, RENDER_OPTIONS)?;
    let position: DbtRowPosition = DbtRowPosition {
        display_index: arguments.value("display_index")?.extract()?,
        display_total: arguments.value("display_total")?.extract()?,
    };
    let detail: String = arguments
        .value("detail")?
        .extract::<Option<String>>()?
        .unwrap_or_default();
    Ok(render_dbt_node_row(
        &native_result(&arguments.value("result")?)?,
        position,
        &detail,
        CliStyle {
            use_color: arguments.flag("use_color")?,
        },
    ))
}
