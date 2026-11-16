//! `read` applied to each item of the iterable attribute `name` of `owner`.

use crate::discovery::_helpers::project::project_facts;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `read` applied to each item of the iterable attribute `name` of `owner`.
#[inline]
pub fn each<T>(
    owner: &Bound<'_, PyAny>,
    name: &str,
    read: impl Fn(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    project_facts::each(owner, name, read)
}
