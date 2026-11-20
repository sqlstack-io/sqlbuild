//! This process's effective environment.

use std::ffi::OsString;

use crate::process_environment::_helpers::overlay::current;

/// `std::env::vars_os()` with this process's overlay applied, inherited variables first.
#[must_use]
pub fn environment_vars_os() -> Vec<(OsString, OsString)> {
    let overlay = current();
    let mut variables: Vec<(OsString, OsString)> = Vec::new();
    for (name, value) in std::env::vars_os() {
        let replaced: bool = overlay.set.iter().any(|(set, _)| *set == name);
        if !replaced && !overlay.remove.contains(&name) {
            variables.push((name, value));
        }
    }
    for (name, value) in overlay.set {
        variables.retain(|(kept, _)| *kept != name);
        variables.push((name, value));
    }
    variables
}
