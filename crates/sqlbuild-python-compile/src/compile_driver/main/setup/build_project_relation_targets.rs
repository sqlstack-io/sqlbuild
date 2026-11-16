//! Return every planned project relation for hard-coded name warnings, or None when off.

use crate::compile_driver::_helpers::setup::relation_targets::RefKinds;
use crate::compile_driver::_helpers::setup::relation_targets::planned_relation;
use crate::compile_driver::_helpers::setup::relation_targets::planned_relation_refs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return every planned project relation for hard-coded name warnings, or None when off.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_project_relation_targets<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_project_relation_targets",
        options,
        (&["adapter", "plan_output"], &[]),
    )?;
    let plan_output: Bound<'py, PyAny> = arguments.value("plan_output")?;
    if !native_field(&plan_output, "enforce_explicit_references")?.is_truthy()? {
        return Ok(None);
    }
    let kinds: RefKinds<'py> = RefKinds::load(py)?;
    let adapter: Bound<'py, PyAny> = arguments.value("adapter")?;
    let targets: Bound<'py, PyDict> = PyDict::new(py);
    for reference in planned_relation_refs(&plan_output, &kinds)?.try_iter()? {
        let reference: Bound<'py, PyAny> = reference?;
        let relation: Bound<'py, PyAny> =
            planned_relation(&adapter, &plan_output, &reference, &kinds)?;
        if !relation.is_none() {
            targets.set_item(reference, relation)?;
        }
    }
    Ok(Some(targets))
}
