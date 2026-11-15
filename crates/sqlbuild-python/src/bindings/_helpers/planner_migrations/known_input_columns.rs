//! Physical columns of the relations a model reads, as inspected by the planner.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyFrozenSet;
use pyo3::{pyclass, pymethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

const SOURCE_INPUT_FUNCTION: &str = "__source";
const REF_INPUT_FUNCTION: &str = "__ref";

/// Look up the lower-cased physical columns a query binds against, or None when unknown.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct KnownInputColumns {
    model_columns: Py<PyAny>,
    source_columns: Py<PyAny>,
}

impl KnownInputColumns {
    /// Inspected model and source columns by relation name.
    pub(crate) fn new(model_columns: &Bound<'_, PyAny>, source_columns: &Bound<'_, PyAny>) -> Self {
        Self {
            model_columns: model_columns.clone().unbind(),
            source_columns: source_columns.clone().unbind(),
        }
    }
}

#[pymethods]
impl KnownInputColumns {
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        kind: &str,
        name: &str,
    ) -> PyResult<Option<Bound<'py, PyFrozenSet>>> {
        let inspected = match kind {
            REF_INPUT_FUNCTION => self.model_columns.bind(py),
            SOURCE_INPUT_FUNCTION => self.source_columns.bind(py),
            _ => return Ok(None),
        };
        let columns = mapping_get(inspected, name)?;
        if columns.is_none() || !columns.is_truthy()? {
            return Ok(None);
        }
        let mut names: Vec<String> = Vec::new();
        for column in columns.try_iter()? {
            names.push(
                native_field(&column?, "name")?
                    .extract::<String>()?
                    .to_lowercase(),
            );
        }
        PyFrozenSet::new(py, names).map(Some)
    }
}
