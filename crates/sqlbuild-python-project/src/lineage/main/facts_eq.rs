//! `facts == other`: item-wise equality with any sequence.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple, PyTupleMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `facts == other`: item-wise equality with any sequence.
pub fn facts_eq(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let other: Bound<'_, PyAny> = args.get_item(0)?;
    let sequence: Bound<'_, PyAny> = py.import("collections.abc")?.getattr("Sequence")?;
    if !other.is_instance(&sequence)? {
        return Ok(py.NotImplemented());
    }
    let mine: Bound<'_, PyAny> = native_field(instance, "facts")?;
    let theirs: Bound<'_, PyTuple> = PyTuple::new(
        py,
        other
            .try_iter()?
            .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()?,
    )?;
    mine.eq(theirs)?.into_py_any(py)
}
