//! The process's discovery source, the sets its last compile used, and the registered runner.

use std::sync::{Mutex, OnceLock};

use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;

use crate::discovered_sets::models::DiscoverySource;
use crate::discovered_sets::types::DiscoverRunner;

/// The source the next compiles expand from; `None` until a command sets one.
pub(crate) static SOURCE: Mutex<Option<DiscoverySource>> = Mutex::new(None);
/// The sets the newest expansion used.
pub(crate) static USED: Mutex<Option<DiscoveredSets>> = Mutex::new(None);
/// The function that calls a discover function with a target's providers.
pub(crate) static RUNNER: OnceLock<DiscoverRunner> = OnceLock::new();
