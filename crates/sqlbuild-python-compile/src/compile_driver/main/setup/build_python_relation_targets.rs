//! Return adapter-qualified runtime relations required by selected Python nodes.

use crate::compile_driver::_helpers::setup::relation_targets::RefKinds;
use crate::compile_driver::_helpers::setup::relation_targets::planned_relation_refs;
use crate::compile_driver::_helpers::setup::relation_targets::resolved_relation;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Return adapter-qualified runtime relations required by selected Python nodes.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_python_relation_targets<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_python_relation_targets",
        options,
        (&["adapter", "project", "plan_output"], &["required_refs"]),
    )?;
    let kinds: RefKinds<'py> = RefKinds::load(py)?;
    let adapter: Bound<'py, PyAny> = arguments.value("adapter")?;
    let project: Bound<'py, PyAny> = arguments.value("project")?;
    let plan_output: Bound<'py, PyAny> = arguments.value("plan_output")?;
    let required: Bound<'py, PyAny> = arguments.value("required_refs")?;
    let refs: Bound<'py, PyAny> = if required.is_none() {
        planned_relation_refs(&plan_output, &kinds)?
    } else {
        required
    };
    let targets: Bound<'py, PyDict> = PyDict::new(py);
    for reference in refs.try_iter()? {
        let reference: Bound<'py, PyAny> = reference?;
        let relation: Bound<'py, PyAny> =
            resolved_relation((&adapter, &project, &plan_output), &reference, &kinds)?;
        targets.set_item(reference, relation)?;
    }
    Ok(targets)
}
