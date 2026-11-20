//! One variable of this process's effective environment.

use std::ffi::{OsStr, OsString};

use crate::process_environment::_helpers::overlay::current;

/// `std::env::var_os(name)` with this process's overlay applied.
#[must_use]
pub fn environment_var_os(name: impl AsRef<OsStr>) -> Option<OsString> {
    let name: &OsStr = name.as_ref();
    let overlay = current();
    if let Some((_, value)) = overlay.set.iter().rev().find(|(set, _)| set == name) {
        return Some(value.clone());
    }
    if overlay.remove.iter().any(|removed| removed == name) {
        return None;
    }
    std::env::var_os(name)
}
