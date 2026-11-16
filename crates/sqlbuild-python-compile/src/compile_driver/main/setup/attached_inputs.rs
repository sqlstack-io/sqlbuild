//! The first compile input snapshot, with diagnostics collected while attaching appended.

use crate::compile_driver::models::InputsRequest;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_base::compile_context::main::diagnostic_collector::collect_compile_diagnostics::collect_compile_diagnostics;
use sqlbuild_python_project::compiled_views::main::project::inputs_with::inputs_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use crate::compile_driver::_helpers::setup::inputs::compile_inputs;

/// The first compile input snapshot, with diagnostics collected while attaching appended.
pub fn attached_inputs<'py>(
    py: Python<'py>,
    request: &InputsRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let collector: Bound<'py, PyAny> = collect_compile_diagnostics(py)?.into_any();
    let (built, violations): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        within(&collector, |violations| {
            Ok((compile_inputs(py, request)?, violations.clone()))
        })?;
    let collected: Bound<'py, PyAny> = native_field(&violations, "diagnostics")?;
    if !collected.is_truthy()? {
        return Ok(built);
    }
    let diagnostics: Bound<'py, PyTuple> = PyTuple::new(
        py,
        native_field(&built, "diagnostics")?
            .try_iter()?
            .chain(collected.try_iter()?)
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    inputs_with(
        &built,
        Some(keywords(py, &[("diagnostics", diagnostics.into_any())])?),
    )
}
