//! The `--target` the running invocation passed.

use crate::discovery::_helpers::sources::provider_secrets::INVOCATION_TARGET;

/// The running invocation's `--target`, `None` when it passed none.
pub fn invocation_target() -> Option<String> {
    INVOCATION_TARGET.with_borrow(Clone::clone)
}
