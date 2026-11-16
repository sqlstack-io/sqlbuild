//! The running delta compile's injection, if any.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::DeltaInjection;
use std::sync::Arc;

/// The running delta compile's injection, if any.
#[inline]
pub fn injection() -> Option<Arc<DeltaInjection>> {
    delta::injection()
}
