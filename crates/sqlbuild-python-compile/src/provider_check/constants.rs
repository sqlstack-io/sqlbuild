//! Discovered collections, key kinds and variables the provider check reads.

/// Discovered Python nodes that run as their own node, with the key kind each publishes.
pub(crate) const OWN_NODES: [(&str, &str); 3] = [
    ("task_functions", "task"),
    ("asset_functions", "asset"),
    ("check_functions", "check"),
];

/// Model settings whose hooks may call a provider's hook.
pub(crate) const HOOK_SETTINGS: [&str; 2] = ["pre_hooks", "post_hooks"];

/// Set in a worker's environment; its run reports to the location's server.
pub(crate) const RUN_TOKEN_VARIABLE: &str = "SQLBUILD_RUN_TOKEN";
