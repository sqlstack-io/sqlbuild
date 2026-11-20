//! This process's effective environment.

use std::ffi::OsString;

use crate::process_environment::_helpers::overlay::current;

/// `std::env::vars_os()` with this process's overlay applied, inherited variables first.
#[must_use]
pub fn environment_vars_os() -> Vec<(OsString, OsString)> {
    let overlay = current();
    let mut variables: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(name, _)| {
            !overlay.remove.contains(name) && !overlay.set.iter().any(|(set, _)| set == name)
        })
        .collect();
    for (name, value) in overlay.set {
        variables.retain(|(kept, _)| *kept != name);
        variables.push((name, value));
    }
    variables
}
