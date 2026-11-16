//! Expand `sql` with the expansion's inputs, through the bridge's memo when one is active.

use crate::render::_helpers::expansion::macro_expansion;
use crate::render::models::ExpansionOutput;
use crate::render::models::NativeMacroExpansion;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Expand `sql` with the expansion's inputs, through the bridge's memo when one is active.
#[inline]
pub fn expanded<'py>(
    expansion: &Bound<'py, NativeMacroExpansion>,
    sql: &str,
    bridge: &Bound<'py, PyAny>,
) -> PyResult<ExpansionOutput> {
    macro_expansion::expanded(expansion, sql, bridge)
}
