//! Run lifecycle inputs and the regions selected Python nodes run in.

use crate::assembly::project::types::ObjectKey;

/// One Python node as lifecycle classification reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleNode {
    /// `task`, `asset`, `loader` or `check`.
    pub kind: String,
    /// The SQL resources the node reads, in authored order.
    pub sql_deps: Vec<ObjectKey>,
}

/// A selected loader whose selected Python upstream reads SQL, so it runs after that SQL is built.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoundTripLoader {
    pub name: String,
    /// The SQL its selected upstream tasks and assets read, which its load waits for; sorted.
    pub waits_for: Vec<ObjectKey>,
    /// Its selected upstream tasks and assets, which must succeed for it to load; sorted.
    pub gates: Vec<String>,
}

/// Selected Python nodes split into ingress and read-side regions, each sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunLifecycle {
    pub ingress_names: Vec<String>,
    pub ingress_loader_names: Vec<String>,
    pub read_side_names: Vec<String>,
    /// Loaders that load in the SQL build after the SQL their upstream reads, sorted by name.
    pub round_trip_loaders: Vec<RoundTripLoader>,
}
