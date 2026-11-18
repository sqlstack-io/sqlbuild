//! Names a plan from synced state uses to find and attach the target's mirrored catalog.

/// The `plan_from` source that plans from the target's synced copy of warehouse state.
pub(crate) const SYNCED_STATE: &str = "synced_state";
/// The variable naming a control store other than the project's own.
pub(crate) const CONTROL_STORE_VARIABLE: &str = "SQLBUILD_CONTROL_DATABASE_URL";
/// The project's own control store, beside its other `sqb dev` state.
pub(crate) const CONTROL_STORE_FILE: &str = ".sqlbuild/control.sqlite3";
/// The adapter attribute inspection reads the synced catalog from.
pub(crate) const SYNCED_CATALOG_ATTRIBUTE: &str = "synced_catalog";
