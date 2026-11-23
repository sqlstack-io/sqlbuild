//! Public constants of native host.

use std::time::Duration;

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

/// SIGINT's number, as the wakeup descriptor carries it.
pub(crate) const INTERRUPT_BYTE: u8 = 2;
/// What stops the watcher; no signal has this number.
pub(crate) const STOP_BYTE: u8 = 0;
/// The most bytes one read of the wakeup descriptor takes.
pub(crate) const RECEIVE_BYTES: usize = 64;
/// 128 plus SIGINT, as shells report Ctrl+C.
pub(crate) const INTERRUPTED_CODE: i64 = 130;
/// 128 plus SIGTERM, which a command reports itself.
pub(crate) const TERMINATED_CODE: i64 = 143;
pub(crate) const INTERRUPTED_LINE: &str = "error: interrupted (Ctrl+C)";
/// How long an interrupted command may take to unwind before an exit that cuts nothing short.
pub(crate) const GRACE: Duration = Duration::from_millis(600);
pub(crate) const POLL: Duration = Duration::from_millis(50);
/// How long a forced exit waits for statements to cancel, and for output to flush.
pub(crate) const FORCED_EXIT_WAIT: Duration = Duration::from_secs(1);
pub(crate) const ADAPTER_BASE_MODULE: &str = "sqlbuild.adapter.contract.classes.base_adapter";
pub(crate) const ADAPTER_BASE_CLASS: &str = "BaseAdapter";
pub(crate) const CONNECT_METHOD: &str = "connect";
/// `ABANDONABLE_THREAD_PREFIX` of the discovered item pool, which names its threads with it.
pub(crate) const STEP_THREAD_PREFIX: &str = "sqlbuild-item";
