//! `target_settings` of a compiled project's target.

use crate::compile_driver::_helpers::setup::targets::target_settings;
use crate::compile_driver::models::TargetSettingsValues;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `target_settings` of a compiled project's target.
pub fn project_target_settings<'py>(
    discovered: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    (connection, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<TargetSettingsValues> {
    target_settings(
        (discovered, &native_field(project, "effective_target_name")?),
        (
            native_field(project, "effective_target_database")?.extract()?,
            native_field(project, "effective_target_schema")?.extract()?,
        ),
        (
            connection,
            &native_field(project, "effective_vars")?,
            adapter,
        ),
    )
}
