//! Authoring parameters reserved by the code-location host.

/// The calendar's evaluation window is supplied by the host, not a provider.
pub(crate) const CALENDAR_WINDOW_PARAMETER: &str = "window";

pub(crate) const RUN_EVENT_CONDITION: &str = "run_event";
pub(crate) const SOURCE_NEW_DATA_CONDITION: &str = "source_new_data";

/// The sensor context is supplied by the host, not a provider.
pub(crate) const SENSOR_CONTEXT_PARAMETER: &str = "ctx";
