//! The process's discovery source, the sets its last compile used, and the registered runner.

use std::collections::{BTreeMap, BTreeSet};
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
/// Sets the newest compile found an item of naming a tag no node carries, with their errors.
pub(crate) static TAG_FAILURES: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
/// Sets already compiled once more with their last good items in this publication.
pub(crate) static FALLEN_BACK: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
