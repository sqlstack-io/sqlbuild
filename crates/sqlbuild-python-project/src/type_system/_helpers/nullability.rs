//! Adapter nullability rules over `InferredNullability` arguments, decided by the native rules.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyString;
use sqlbuild_analysis::nullability_rules::main::conditional_result_nullability::conditional_result_nullability;
use sqlbuild_analysis::nullability_rules::main::safe_cast_nullability::safe_cast_nullability;

use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

const UNKNOWN_MEMBER: &str = "UNKNOWN";

/// The `InferredNullability` member whose value is `value`.
fn member<'py>(args: &Bound<'py, PyAny>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    enum_from_value(args.py(), RecordId::InferredNullability, value)
}

/// The text of one argument; an argument that is not a string matches no state.
fn argument_text(value: &Bound<'_, PyAny>) -> String {
    match value.cast::<PyString>() {
        Ok(text) => text.to_string(),
        Err(_) => String::new(),
    }
}

/// The text of the first `count` arguments, read by index.
fn argument_values(args: &Bound<'_, PyAny>, count: usize) -> PyResult<Vec<String>> {
    (0..count)
        .map(|index| Ok(argument_text(&args.get_item(index)?)))
        .collect()
}

/// The first argument itself, or `InferredNullability.UNKNOWN` without arguments.
#[pyfunction]
pub(crate) fn infer_first_arg_nullability<'py>(
    args: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !args.is_truthy()? {
        return enum_member(args.py(), RecordId::InferredNullability, UNKNOWN_MEMBER);
    }
    args.get_item(0)
}

/// IF/IFF-style nullability from the true and false result arguments.
#[pyfunction]
pub(crate) fn infer_conditional_result_nullability<'py>(
    args: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let values: Vec<String> = argument_values(args, args.len()?)?;
    let values: Vec<&str> = values.iter().map(String::as_str).collect();
    member(args, conditional_result_nullability(&values))
}

/// SAFE_CAST/TRY_CAST nullability without assuming the conversion succeeds.
#[pyfunction]
pub(crate) fn infer_safe_cast_nullability<'py>(
    args: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let values: Vec<String> = if args.is_truthy()? {
        argument_values(args, 1)?
    } else {
        Vec::new()
    };
    let values: Vec<&str> = values.iter().map(String::as_str).collect();
    member(args, safe_cast_nullability(&values))
}
