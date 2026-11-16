//! Comment, quoted-text and parenthesis boundaries at Python string offsets of one SQL text.

use crate::sqltext::models::SqlCodeScanner;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlCodeScanner>()?;
    Ok(())
}
