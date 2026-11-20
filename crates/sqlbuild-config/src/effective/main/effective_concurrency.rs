//! `effective_concurrency`: the worker limit of one command.

use crate::effective::models::{ConcurrencyLayers, EffectiveConcurrency};

/// `--concurrency`, else the target's `concurrency`, else the local-then-project setting.
#[must_use]
pub fn effective_concurrency(layers: &ConcurrencyLayers) -> EffectiveConcurrency {
    EffectiveConcurrency {
        limit: layers
            .requested
            .or(layers.target)
            .unwrap_or(layers.settings),
    }
}
