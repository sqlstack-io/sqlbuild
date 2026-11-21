//! Names a plan from synced state uses to find and attach the target's mirrored catalog, and run records' plan names.

/// The `plan_from` source that plans from the target's synced copy of warehouse state.
pub(crate) const SYNCED_STATE: &str = "synced_state";
/// The variable naming a control store other than the project's own.
pub(crate) const CONTROL_STORE_VARIABLE: &str = "SQLBUILD_CONTROL_DATABASE_URL";
/// The project's own control store, beside its other `sqb dev` state.
pub(crate) const CONTROL_STORE_FILE: &str = ".sqlbuild/control.sqlite3";
/// The adapter method that checks a synced copy against the warehouse; only Snowflake has it.
pub(crate) const RELATION_CHANGES_METHOD: &str = "read_relation_changes";
/// The adapter attribute inspection reads the synced catalog from.
pub(crate) const SYNCED_CATALOG_ATTRIBUTE: &str = "synced_catalog";
/// The plan action of a node the build leaves as it is.
pub(crate) const SKIP_ACTION: &str = "skip";
/// The most connections one phase of a run record opens; each writes its share of schemas in turn.
pub(crate) const MAX_RUN_RECORD_CONNECTIONS: usize = 4;
/// How often a run record's caller checks for Ctrl+C while its writer threads run.
pub(crate) const RUN_RECORD_SIGNAL_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(5);

/// The snapshot kind of a Python check node.
pub(crate) const PYTHON_CHECK_KIND: &str = "check";
