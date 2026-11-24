//! Limits of the warehouse history reads.

/// The most groups one batch of daily cost series may ask for.
pub const COST_SERIES_GROUPS: usize = 200;
/// The price source of dollars from the project's `[cost] usd_per_credit`.
pub(crate) const CONFIG_PRICE_SOURCE: &str = "config";
/// The price source of the list price for the account's region: an estimate.
pub(crate) const LIST_PRICE_SOURCE: &str = "list_price";
/// The source of a total whose rows were priced from different sources, none an estimate.
pub(crate) const MIXED_PRICE_SOURCE: &str = "mixed";
