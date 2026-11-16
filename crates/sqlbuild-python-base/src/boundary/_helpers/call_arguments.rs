//! Reading the arguments of a native method call.

use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};

/// The argument `name`, passed by keyword or at `position`.
pub(crate) fn call_argument<'py>(
    (args, kwargs): (&Bound<'py, PyTuple>, Option<&Bound<'py, PyDict>>),
    name: &str,
    position: usize,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(kwargs) = kwargs
        && let Some(value) = kwargs.get_item(name)?
    {
        return Ok(value);
    }
    args.get_item(position)
}
