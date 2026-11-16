//! Public traits and type aliases of native host.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Extraction at the permanent Python executor edge, including nested result records.
pub trait ReadResult<'py>: Sized {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self>;
}
