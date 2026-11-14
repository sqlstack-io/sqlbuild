//! Authoring parameters reserved by the code-location host.

/// The calendar's evaluation window is supplied by the host, not a provider.
pub(crate) const CALENDAR_WINDOW_PARAMETER: &str = "window";

pub(crate) const SOURCE_NEW_DATA_CONDITION: &str = "source_new_data";
pub(crate) const SOURCE_STALE_CONDITION: &str = "source_stale";

/// The sensor context is supplied by the host, not a provider.
pub(crate) const SENSOR_CONTEXT_PARAMETER: &str = "ctx";

pub(crate) const AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";
/// The removed records `sqb migrate orchestration` rebuilds while it imports a project.
pub(crate) const LEGACY_AUTHORING_MODULE: &str =
    "sqlbuild.orchestration_declarations.models.legacy_authoring";
/// Frames of these modules construct declarations on the author's behalf.
pub(crate) const CONSTRUCTING_MODULES: [&str; 3] =
    [AUTHORING_MODULE, LEGACY_AUTHORING_MODULE, "dataclasses"];

/// The authoring classes publication imports calendar and sensor modules for.
pub(crate) const CALENDAR_CLASS: &str = "Calendar";
pub(crate) const SENSOR_CLASS: &str = "Sensor";

/// Pages one bounded state-history SELECT streams before the scan continues from its cursor.
pub(crate) const HISTORY_STREAM_PAGES: usize = 50;
