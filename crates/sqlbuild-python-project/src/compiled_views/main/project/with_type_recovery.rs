//! The model with its recovered bindings and its poisoned outputs' types forgotten.

use crate::compiled_views::main::project::replace::replace;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyFrozenSet, PyFrozenSetMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The model with its recovered bindings and its poisoned outputs' types forgotten.
#[pyfunction]
#[pyo3(signature = (*, model, diagnostics, poisoned))]
pub fn with_type_recovery<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
    poisoned: &Bound<'py, PyFrozenSet>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let name: String = native_field(model, "name")?.extract()?;
    let mut unchecked: Vec<String> = Vec::new();
    for pair in poisoned.iter() {
        let (model_name, column): (String, String) = pair.extract()?;
        if model_name == name {
            unchecked.push(column);
        }
    }
    if !model.is_instance(&record_class(py, RecordId::CompiledModel)?)? {
        return model.call_method1("with_type_recovery", (diagnostics, unchecked));
    }
    let inferred: Bound<'py, PyAny> = native_field(model, "inferred_columns")?;
    let columns: Bound<'py, PyAny> = if inferred.is_none() {
        inferred
    } else {
        let mut columns: Vec<Bound<'py, PyAny>> = Vec::new();
        for column in inferred.try_iter()? {
            let column: Bound<'py, PyAny> = column?;
            let key = (name.as_str(), native_field(&column, "name")?);
            columns.push(if poisoned.contains(key)? {
                replace(&column, &[("type", py.None().into_bound(py))])?
            } else {
                column
            });
        }
        PyTuple::new(py, columns)?.into_any()
    };
    replace(
        model,
        &[
            ("binding_diagnostics", diagnostics.clone()),
            ("inferred_columns", columns),
            (
                "unchecked_output_columns",
                PyFrozenSet::new(py, &unchecked)?.into_any(),
            ),
        ],
    )
}
