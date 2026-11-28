//! Reversed edges: every source key first, then each new target in first-seen order.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// Reversed edges: every source key first, then each new target in first-seen order.
pub fn inverted<'py>(edges: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = edges.py();
    let inverted: Bound<'py, PyDict> = PyDict::new(py);
    for key in edges.try_iter()? {
        inverted.set_item(key?, PyList::empty(py))?;
    }
    for item in mapping_items(edges)?.try_iter()? {
        let (source, targets): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        for target in targets.try_iter()? {
            let target: Bound<'py, PyAny> = target?;
            let sources: Bound<'py, PyList> = match inverted.get_item(&target)? {
                Some(sources) => sources.cast_into()?,
                None => {
                    let sources: Bound<'py, PyList> = PyList::empty(py);
                    inverted.set_item(&target, &sources)?;
                    sources
                }
            };
            sources.append(&source)?;
        }
    }
    Ok(inverted)
}
