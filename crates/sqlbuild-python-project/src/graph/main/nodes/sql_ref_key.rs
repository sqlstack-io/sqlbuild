//! The compiled graph key a typed model, source or seed reference names.

use crate::graph::_helpers::nodes::execution_gates::keyword_call;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The compiled graph key a typed model, source or seed reference names.
#[pyfunction]
pub fn sql_ref_key<'py>(
    py: Python<'py>,
    reference: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let resource_type: Bound<'py, PyAny> = enum_from_value(
        py,
        RecordId::CompiledResourceType,
        native_field(reference, "kind")?.getattr("value")?,
    )?;
    keyword_call(
        &record_class(py, RecordId::CompiledObjectKey)?,
        &[
            ("resource_type", resource_type),
            ("name", native_field(reference, "name")?),
        ],
    )
}
