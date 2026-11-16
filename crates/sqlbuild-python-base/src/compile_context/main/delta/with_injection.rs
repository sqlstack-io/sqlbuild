//! Run `body` with `injection` visible to the rerun stages; returns it unless still held.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::DeltaInjection;

/// Run `body` with `injection` visible to the rerun stages; returns it unless still held.
#[inline]
pub fn with_injection<T>(
    injection: DeltaInjection,
    body: impl FnOnce() -> T,
) -> (T, Option<DeltaInjection>) {
    delta::with_injection(injection, body)
}
