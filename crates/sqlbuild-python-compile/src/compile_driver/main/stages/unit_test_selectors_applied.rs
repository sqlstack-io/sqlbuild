//! Options whose selectors name only resources, with unit tests selected apart.

use crate::compile_driver::_helpers::stages::selection_phases::DATACLASSES_MODULE;
use crate::compile_driver::_helpers::stages::selection_phases::unselective;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_selection::main::split_project_unit_test_selectors::split_project_unit_test_selectors;

/// Options whose selectors name only resources, with unit tests selected apart.
pub fn unit_test_selectors_applied<'py>(
    options: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let resolved: Bound<'py, PyAny> = if options.is_none() {
        new_record(py, RecordId::CompilePipelineOptions, (), None)?
    } else {
        options.clone()
    };
    if unselective(&resolved)? {
        return Ok(resolved);
    }
    let split: Bound<'py, PyAny> = split_project_unit_test_selectors(
        &native_field(&resolved, "select")?,
        &native_field(&resolved, "exclude")?,
        discovered,
        native_field(&resolved, "accepts_unit_test_selectors")?.extract()?,
    )?;
    imported(py, DATACLASSES_MODULE, "replace")?.call(
        (resolved,),
        Some(&keywords(
            py,
            &[
                ("select", native_field(&split, "select")?),
                ("exclude", native_field(&split, "exclude")?),
                (
                    "sql_test_selection",
                    native_field(&split, "sql_test_selection")?,
                ),
            ],
        )?),
    )
}
