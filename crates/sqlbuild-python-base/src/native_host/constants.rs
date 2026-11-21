//! Public constants of native host.

/// The calendar's evaluation window is supplied by the host, not a provider.
pub const CALENDAR_WINDOW_PARAMETER: &str = "window";

pub const SOURCE_NEW_DATA_CONDITION: &str = "source_new_data";
/// The sensor condition on checks, audits and SQL tests that fail or warn inside runs.
pub const CHECK_EVENT_CONDITION: &str = "check_event";

pub const SOURCE_STALE_CONDITION: &str = "source_stale";

/// The sensor context is supplied by the host, not a provider.
pub const SENSOR_CONTEXT_PARAMETER: &str = "ctx";

pub const AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";

/// Frames of these modules construct declarations on the author's behalf.
pub const CONSTRUCTING_MODULES: [&str; 2] = [AUTHORING_MODULE, "dataclasses"];

/// The authoring classes publication imports calendar and sensor modules for.
pub const CALENDAR_CLASS: &str = "Calendar";

pub const SENSOR_CLASS: &str = "Sensor";

/// Pages one bounded state-history SELECT streams before the scan continues from its cursor.
pub const HISTORY_STREAM_PAGES: usize = 50;

/// `env_prefix_target` values under which pydantic-settings prefixes alias names too.
pub const ALIAS_PREFIX_TARGET: &str = "alias";

pub const ALL_PREFIX_TARGET: &str = "all";
