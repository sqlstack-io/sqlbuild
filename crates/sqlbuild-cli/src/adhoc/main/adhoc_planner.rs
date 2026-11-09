//! The command planner `sqb server` resolves ad-hoc runs with.

use std::sync::Arc;

use sqlbuild_orchestration::runs::types::CommandPlanner;

use crate::adhoc::_helpers::planner::CliPlanner;

/// The CLI's grammar, flag allowlist and selector resolver behind the control plane's planner contract.
#[must_use]
pub fn adhoc_planner() -> Arc<dyn CommandPlanner> {
    Arc::new(CliPlanner)
}
