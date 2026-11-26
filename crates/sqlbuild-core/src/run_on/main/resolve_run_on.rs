//! The effective `RunOn` of a run: the flag, then the target, then `[settings]`, then `auto`.

use crate::run_on::models::{RunOn, RunOnLayers};

/// The first layer that sets a valid value; `auto` when none does.
#[must_use]
pub fn resolve_run_on(layers: &RunOnLayers) -> RunOn {
    [&layers.flag, &layers.target, &layers.settings]
        .into_iter()
        .flatten()
        .find_map(|value| RunOn::parse(value))
        .unwrap_or_default()
}
