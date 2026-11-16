//! Run lifecycle inputs and the regions selected Python nodes run in.

/// One Python node as lifecycle classification reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleNode {
    /// `task`, `asset`, `loader` or `check`.
    pub kind: String,
}

/// Selected Python nodes split into ingress and read-side regions, each sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunLifecycle {
    pub ingress_names: Vec<String>,
    pub ingress_loader_names: Vec<String>,
    pub read_side_names: Vec<String>,
}
