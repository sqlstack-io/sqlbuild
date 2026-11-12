//! Authoring parameters reserved by the code-location host.

/// The calendar's evaluation window is supplied by the host, not a provider.
pub(crate) const CALENDAR_WINDOW_PARAMETER: &str = "window";

pub(crate) const RUN_EVENT_CONDITION: &str = "run_event";
pub(crate) const NODE_EVENT_CONDITION: &str = "node_event";
pub(crate) const CHECK_KIND: &str = "check";
pub(crate) const SOURCE_NEW_DATA_CONDITION: &str = "source_new_data";
pub(crate) const SOURCE_STALE_CONDITION: &str = "source_stale";

/// The sensor context is supplied by the host, not a provider.
pub(crate) const SENSOR_CONTEXT_PARAMETER: &str = "ctx";

pub(crate) const AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";
/// Frames of these modules construct declarations on the author's behalf.
pub(crate) const CONSTRUCTING_MODULES: [&str; 2] = [AUTHORING_MODULE, "dataclasses"];

/// Kinds named in the global unique-name check.
pub(crate) const JOB_LABEL: &str = "job";
pub(crate) const GROUP_LABEL: &str = "job group";
pub(crate) const SENSOR_LABEL: &str = "sensor";
pub(crate) const CALENDAR_LABEL: &str = "calendar schedule";

/// Pages one bounded state-history SELECT streams before the scan continues from its cursor.
pub(crate) const HISTORY_STREAM_PAGES: usize = 50;
