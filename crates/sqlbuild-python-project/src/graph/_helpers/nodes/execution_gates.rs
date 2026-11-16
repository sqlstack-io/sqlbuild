//! Ordering edges attached audits and model hooks add, and the keys hooks and SQL refs name.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

/// The model config keys whose entries are hooks, in lifecycle order.
pub(crate) const MODEL_HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];

pub(crate) fn keyword_call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(name, value)?;
    }
    callable.call((), Some(&keywords))
}
