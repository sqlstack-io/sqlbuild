//! The compiled object key of the model `name`.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// The compiled object key of the model `name`.
pub fn model_key<'py>(py: Python<'py>, name: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::CompiledObjectKey,
        &[
            (
                "resource_type",
                enum_member(py, RecordId::CompiledResourceType, "MODEL")?,
            ),
            ("name", name.clone()),
        ],
    )
}
