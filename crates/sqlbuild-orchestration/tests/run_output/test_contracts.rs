//! Run output: worker batches into zstd chunks, the index, pages, search, download, the size cap and retention.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "test_cap.rs"]
mod test_cap;
#[path = "test_large.rs"]
mod test_large;
#[path = "test_logs.rs"]
mod test_logs;
#[path = "test_reads.rs"]
mod test_reads;
#[path = "test_retention.rs"]
mod test_retention;
#[path = "test_types.rs"]
pub(crate) mod test_types;
