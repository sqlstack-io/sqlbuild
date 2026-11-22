//! The function that calls a discover function with a target's providers, registered once.

use crate::discovered_sets::_helpers::state::RUNNER;
use crate::discovered_sets::types::DiscoverRunner;

/// Use `runner` to call discover functions; the first registration wins.
pub fn register_discover_runner(runner: DiscoverRunner) {
    let _ = RUNNER.set(runner);
}
