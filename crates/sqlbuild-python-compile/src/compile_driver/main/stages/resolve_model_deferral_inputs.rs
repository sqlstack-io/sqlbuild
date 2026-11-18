//! Resolve native target locations and warehouse relations for model deferral.

use crate::compile_driver::_helpers::stages::deferral::DeferredNaming;
use crate::compile_driver::_helpers::stages::deferral::deferred_locations;
use crate::compile_driver::_helpers::stages::deferral::deferred_relations;
use crate::compile_driver::_helpers::stages::deferral::deferred_target_config;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Resolve native target locations and warehouse relations for model deferral.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn resolve_model_deferral_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_model_deferral_inputs",
        options,
        (
            &[
                "project",
                "discovered_inputs",
                "adapter",
                "connection",
                "defer_to",
            ],
            &[],
        ),
    )?;
    let defer_to: Bound<'py, PyAny> = arguments.value("defer_to")?;
    if defer_to.is_none() {
        return new_record(py, RecordId::DeferralInputs, (), None);
    }
    let project: Bound<'py, PyAny> = arguments.value("project")?;
    let adapter: Bound<'py, PyAny> = arguments.value("adapter")?;
    let target_config: Bound<'py, PyAny> = deferred_target_config(
        &arguments.value("discovered_inputs")?,
        &defer_to,
        &native_field(&project, "effective_target_name")?,
    )?;
    let locations: Bound<'py, PyDict> = deferred_locations(
        &project,
        &DeferredNaming {
            target_config,
            effective_vars: native_field(&project, "effective_vars")?,
            default_schema: adapter.call_method0("default_schema")?,
            default_database: adapter.call_method0("default_database")?,
            render_qualified_name: native_field(&adapter, "render_qualified_name")?,
        },
    )?;
    let relations: Bound<'py, PyDict> = deferred_relations(
        &adapter,
        &arguments.value("connection")?,
        locations.as_any(),
    )?;
    new_record_with(
        py,
        RecordId::DeferralInputs,
        &[
            ("deferred_locations", locations.into_any()),
            ("deferred_relations", relations.into_any()),
            ("defer_to", defer_to),
        ],
    )
}
