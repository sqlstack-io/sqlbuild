//! The one stop switch of the process.

use std::sync::OnceLock;

use crate::dev_stop::models::StopSwitch;

/// Set on first use by `stop_switch`.
pub(crate) static SWITCH: OnceLock<StopSwitch> = OnceLock::new();
