//! Public header records and user-supplied constructors at the Python boundary.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

/// Factory results, source locations and the cached public audit tuple are Python-owned edges.
pub(crate) struct AuditProjection {
    pub(crate) custom: Option<Vec<Py<PyAny>>>,
    pub(crate) generated: Vec<Py<PyAny>>,
    pub(crate) location: Py<PyAny>,
    pub(crate) records: PyOnceLock<Py<PyTuple>>,
}

/// A supplied constructor and its public family records; native callers retain the family specs.
pub(crate) struct FamilyProjection {
    class: Py<PyAny>,
    pub(crate) custom: bool,
    records: PyOnceLock<Py<PyTuple>>,
}

impl FamilyProjection {
    pub(crate) fn new(class: &Bound<'_, PyAny>, custom: bool) -> Self {
        Self {
            class: class.clone().unbind(),
            custom,
            records: PyOnceLock::new(),
        }
    }

    pub(crate) fn project<'py>(
        &self,
        py: Python<'py>,
        specs: &[DynamicFamily],
    ) -> PyResult<Bound<'py, PyTuple>> {
        self.records
            .get_or_try_init(py, || {
                let mut records: Vec<Bound<'_, PyAny>> = Vec::with_capacity(specs.len());
                for family in specs {
                    let fields = PyDict::new(py);
                    fields.set_item("name", &family.name)?;
                    fields.set_item("pivot_column", &family.pivot_column)?;
                    fields.set_item("value_column", &family.value_column)?;
                    fields.set_item("aggregate", &family.aggregate)?;
                    fields.set_item("type", &family.data_type)?;
                    fields.set_item("name_pattern", &family.name_pattern)?;
                    records.push(self.class.bind(py).call((), Some(&fields))?);
                }
                PyTuple::new(py, records).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }
}
