//! Where a compile's discovered sets come from.

use std::collections::BTreeSet;

use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use uuid::Uuid;

/// The sets a compile expands `for_each` assets from.
#[derive(Clone, Debug, Default)]
pub enum DiscoverySource {
    /// No sets: every `for_each` asset is its placeholder node, as outside a location.
    #[default]
    Placeholders,
    /// The sets a publication saved, as a worker compiles them.
    Saved(DiscoveredSets),
    /// Call discover functions now.
    Live(LiveDiscovery),
}

/// Discovery that calls the project's discover functions with a target's providers.
#[derive(Clone, Debug)]
pub struct LiveDiscovery {
    /// The target whose providers the functions get; `None` for the project's default target.
    pub target: Option<String>,
    /// The sets from before, reused where `rediscover` leaves a set out and kept when one fails.
    pub previous: DiscoveredSets,
    pub rediscover: Rediscover,
    /// Whether a failed set keeps its last good result, as a location does; otherwise the compile fails.
    pub keep_last_good: bool,
    /// The server whose current snapshot gives a failed set with no result its last good one.
    pub seed: Option<SeedSource>,
}

/// The location's server and target; its credential is read from the environment when used.
#[derive(Clone, Debug)]
pub struct SeedSource {
    pub server_url: String,
    pub location_id: Uuid,
    pub target: String,
}

/// Which discover functions a live discovery calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rediscover {
    All,
    Only(BTreeSet<String>),
}
