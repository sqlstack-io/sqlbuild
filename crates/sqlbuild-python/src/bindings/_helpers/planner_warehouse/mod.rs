//! Native warehouse inspection for planning over Python adapter callbacks.

pub(crate) mod constants;
pub(crate) mod cursors;
pub(crate) mod fingerprint_cache;
pub(crate) mod models;
pub(crate) mod reads;
pub(crate) mod registration;
pub(crate) mod semantic_sources;
pub(crate) mod snapshot;
pub(crate) mod source_columns;
pub(crate) mod source_deferral;
pub(crate) mod source_freshness;
pub(crate) mod source_tables;
pub(crate) mod synced_reads;
pub(crate) mod watermarks;
