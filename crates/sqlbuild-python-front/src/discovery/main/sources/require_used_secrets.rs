//! Fail before anything runs when a provider in `uses` lacks required secrets, listing each with what needs it.

use crate::discovery::_helpers::sources::provider_secrets;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Fail before anything runs when a provider in `uses` lacks required secrets, listing each with what needs it.
#[inline]
pub fn require_used_secrets(
    discovered: &Bound<'_, PyAny>,
    uses: &[(String, String)],
) -> PyResult<()> {
    provider_secrets::require_used_secrets(discovered, uses)
}
