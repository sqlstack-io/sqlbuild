//! `sorted(items)` as a tuple of strings.

use crate::planner_phases::main::text::text;
use crate::planner_phases::main::tuple::tuple;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// `sorted(items)` as a tuple of strings.
pub fn sorted_names<'py>(py: Python<'py>, mut names: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    names.sort();
    tuple(py, names.iter().map(|name| text(py, name)).collect())
}
