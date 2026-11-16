//! What `relation_keys` reads for a source: its native view itself, or its `source_entry`.

use crate::resource_views::main::source_views::read_source_view::read_source_view;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// What `relation_keys` reads for a source: its native view itself, or its `source_entry`.
pub fn source_key_input<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if read_source_view(source, |_, _| Ok(())).is_some() {
        return Ok(source.clone());
    }
    native_field(source, "source_entry")
}
