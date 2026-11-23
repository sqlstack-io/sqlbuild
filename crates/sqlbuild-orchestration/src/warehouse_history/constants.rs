//! Limits of the warehouse history reads.

/// The most groups one batch of daily cost series may ask for.
pub const COST_SERIES_GROUPS: usize = 200;
/// The price source of dollars from the project's `[cost] usd_per_credit`.
pub(crate) const CONFIGURED_PRICE: &str = "configured";
