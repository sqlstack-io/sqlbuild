//! Public traits and type aliases of sqltext.

use pyo3::prelude::Py;
use pyo3::prelude::PyAny;

pub type ParsedModelHeader = (
    Option<Py<PyAny>>,
    Option<Vec<(String, usize, usize)>>,
    Option<String>,
);
