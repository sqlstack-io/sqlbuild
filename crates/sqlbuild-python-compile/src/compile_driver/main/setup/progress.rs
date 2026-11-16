//! Report `message` through `on_progress` when a callback is set.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// Report `message` through `on_progress` when a callback is set.
pub fn progress(on_progress: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    if !on_progress.is_none() {
        on_progress.call1((message,))?;
    }
    Ok(())
}
