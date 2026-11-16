//! Fail module init when the typed names and the registration tables disagree.

use crate::records::_helpers::native_classes::registry;
use pyo3::prelude::PyResult;

/// Fail module init when the typed names and the registration tables disagree.
#[inline]
pub fn verify_tables() -> PyResult<()> {
    registry::verify_tables()
}
