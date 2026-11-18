//! Authoring parameters reserved by the code-location host.

/// The calendar's evaluation window is supplied by the host, not a provider.
pub(crate) const CALENDAR_WINDOW_PARAMETER: &str = "window";

pub(crate) const SOURCE_NEW_DATA_CONDITION: &str = "source_new_data";
pub(crate) const SOURCE_STALE_CONDITION: &str = "source_stale";

/// The sensor context is supplied by the host, not a provider.
pub(crate) const SENSOR_CONTEXT_PARAMETER: &str = "ctx";

pub(crate) const AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";
/// Frames of these modules construct declarations on the author's behalf.
pub(crate) const CONSTRUCTING_MODULES: [&str; 2] = [AUTHORING_MODULE, "dataclasses"];

/// The authoring classes publication imports calendar and sensor modules for.
pub(crate) const CALENDAR_CLASS: &str = "Calendar";
pub(crate) const SENSOR_CLASS: &str = "Sensor";

/// Pages one bounded state-history SELECT streams before the scan continues from its cursor.
pub(crate) const HISTORY_STREAM_PAGES: usize = 50;

/// `env_prefix_target` values under which pydantic-settings prefixes alias names too.
pub(crate) const ALIAS_PREFIX_TARGET: &str = "alias";
pub(crate) const ALL_PREFIX_TARGET: &str = "all";
/// The `plan_from` source that plans from the target's synced copy of warehouse state.
pub(crate) const SYNCED_STATE: &str = "synced_state";
/// The variable naming a control store other than the project's own.
pub(crate) const CONTROL_STORE_VARIABLE: &str = "SQLBUILD_CONTROL_DATABASE_URL";
/// The project's own control store, beside its other `sqb dev` state.
pub(crate) const CONTROL_STORE_FILE: &str = ".sqlbuild/control.sqlite3";
/// The adapter attribute inspection reads the synced catalog from.
pub(crate) const SYNCED_CATALOG_ATTRIBUTE: &str = "synced_catalog";
