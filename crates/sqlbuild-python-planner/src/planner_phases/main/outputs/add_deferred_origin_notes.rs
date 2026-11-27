//! Note unselected upstreams deferred to pre-migration names, and each lookup in `failures`.

use crate::planner_phases::_helpers::outputs::output_assembly::concat;
use crate::planner_phases::main::outputs::unselected_upstream_models::unselected_upstream_models;
use crate::planner_phases::models::PlanOutputFields;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_phases::main::native::native;
use sqlbuild_python_project::planner_phases::main::text::text;
use sqlbuild_python_project::planner_phases::main::tuple::tuple;

/// Note unselected upstreams deferred to pre-migration names, and each lookup in `failures`.
pub fn add_deferred_origin_notes<'py>(
    mut output: PlanOutputFields<'py>,
    (project, scopes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (deferral, failures): (&Bound<'py, PyAny>, &[(String, String)]),
) -> PyResult<PlanOutputFields<'py>> {
    let py = project.py();
    let locations = native_field(deferral, "deferred_locations")?;
    if locations.is_none() {
        return Ok(output);
    }
    let names: Vec<String> = unselected_upstream_models(scopes)?;
    let warning = |name: &str, message: &str| -> PyResult<Bound<'py, PyAny>> {
        native(
            py,
            RecordId::PlanWarning,
            &[
                ("model_name", text(py, name)),
                (
                    "severity",
                    enum_member(py, RecordId::WarningSeverity, "WARNING")?,
                ),
                ("message", text(py, message)),
            ],
        )
    };
    let mut notes: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        if names.binary_search(&name).is_err() {
            continue;
        }
        let Some(location) = locations.cast::<PyDict>()?.get_item(&name)? else {
            continue;
        };
        if native_field(&location, "name")?
            .eq(native_field(&native_field(&model, "destination")?, "name")?)?
        {
            continue;
        }
        notes.push(warning(
            &name,
            &format!(
                "deferred to pre-migration name {}: the deferred target has not built model '{name}' under its current name yet",
                native_field(&location, "qualified_name")?
            ),
        )?);
    }
    for (name, message) in failures {
        notes.push(warning(name, message)?);
    }
    if !notes.is_empty() {
        let warnings = concat(py, &[output.field("warnings")?, tuple(py, notes)?])?;
        output.set("warnings", warnings);
    }
    Ok(output)
}
