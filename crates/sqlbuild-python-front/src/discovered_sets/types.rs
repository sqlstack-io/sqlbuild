//! The registered function that calls one discover function.

use pyo3::prelude::{Bound, PyAny, Python};

/// Calls a discover function: `(project dir, target)`, `(record, for_each functions)`; JSON or why not.
pub type DiscoverRunner = for<'py> fn(
    Python<'py>,
    (&Bound<'py, PyAny>, Option<&str>),
    (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> Result<String, String>;
