//! The Python modules sensor records live in.

/// The module the authoring records live in, such as `Sensor` and `RunRequest`.
pub const SENSOR_AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";
/// The module the event records a sensor's function receives live in.
pub const SENSOR_EVENTS_MODULE: &str = "sqlbuild.orchestration_declarations.models.sensor_events";
/// The module the event kind enums live in.
pub const SENSOR_KINDS_MODULE: &str = "sqlbuild.orchestration_declarations.types";
/// What replaces a provider secret value in a sensor's traceback or skip reason.
pub const SECRET_MASK: &str = "****";
