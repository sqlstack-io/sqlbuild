//! Format direct clone command execution results as JSON.

use crate::result_output::_helpers::results::assets::clone_asset;
use crate::result_output::_helpers::results::documents::Document;
use crate::result_output::_helpers::results::documents::Extras;
use crate::result_output::_helpers::results::documents::FAILED;
use crate::result_output::_helpers::results::documents::emitted;
use crate::result_output::_helpers::results::documents::outcome_status;
use crate::result_output::_helpers::results::documents::summary;
use crate::result_output::_helpers::results::projection::Projection;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_planner::plan_output::main::items::items;
use sqlbuild_python_planner::plan_output::main::text::text;

/// Format direct clone command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, result, resource_types_by_name, phase_timings = None))]
pub fn format_clone_execution_json<'py>(
    py: Python<'py>,
    result: Bound<'py, PyAny>,
    resource_types_by_name: Bound<'py, PyAny>,
    phase_timings: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    let (mut failure_count, mut warning_count): (usize, usize) = (0, 0);
    for item in items(&native_field(&result, "item_results")?)? {
        let name: String = text(&native_field(&item, "name")?)?;
        let resource_type: String = text(&resource_types_by_name.get_item(&name)?)?;
        if !projection.resource_terminal_recorded(&name, &format!("{resource_type}:{name}"))? {
            continue;
        }
        let status: Bound<'py, PyAny> = native_field(&native_field(&item, "status")?, "value")?;
        failure_count += usize::from(status.eq(FAILED)?);
        warning_count += usize::from(status.eq("warning")?);
        assets.push(clone_asset(&projection, &item, &resource_type)?);
    }
    let total: usize = assets.len();
    emitted(
        &projection,
        Document {
            command: "clone".to_owned(),
            status: outcome_status(failure_count),
            summary: summary(
                py,
                &[
                    ("success_count", total - failure_count - warning_count),
                    ("failure_count", failure_count),
                    ("warning_count", warning_count),
                    ("total_count", total),
                ],
            )?,
            assets,
            checks: Vec::new(),
        },
        Extras {
            trailing: phase_timings
                .into_iter()
                .map(|timings| ("phase_timings", timings.into_any()))
                .collect(),
            ..Extras::default()
        },
    )
}
