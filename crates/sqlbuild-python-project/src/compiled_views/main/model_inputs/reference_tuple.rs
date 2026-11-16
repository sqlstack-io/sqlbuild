//! `CompileSqlReference` records of `rows`, built once into `cache`.

use crate::compiled_views::models::InputViewClasses;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_project::models::ReferenceRow;

/// `CompileSqlReference` records of `rows`, built once into `cache`.
pub fn reference_tuple(
    classes: &InputViewClasses<'_>,
    rows: &[ReferenceRow],
    cache: &PyOnceLock<Py<PyTuple>>,
) -> PyResult<Py<PyTuple>> {
    let py: Python<'_> = classes.sql_reference.py();
    cache
        .get_or_try_init(py, || {
            let mut references: Vec<Bound<'_, PyAny>> = Vec::with_capacity(rows.len());
            for ReferenceRow {
                kind,
                name,
                package,
                call_argument_count: count,
            } in rows
            {
                let fields: Bound<'_, PyDict> = PyDict::new(py);
                fields.set_item("ref_kind", classes.reference_kind.call1((kind,))?)?;
                fields.set_item("ref_name", name)?;
                fields.set_item("ref_package", package)?;
                fields.set_item("call_argument_count", count)?;
                references.push(classes.sql_reference.call((), Some(&fields))?);
            }
            Ok::<_, pyo3::PyErr>(PyTuple::new(py, references)?.unbind())
        })
        .map(|references| references.clone_ref(py))
}
