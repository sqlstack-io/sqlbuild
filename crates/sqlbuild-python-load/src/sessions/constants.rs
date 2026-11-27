//! The `LoadSession` methods a caller session calls, and the exception SQLLoad reads as "doesn't exist".

pub(crate) const EXECUTE: &str = "execute";
pub(crate) const QUERY_TEXT: &str = "query_text";
pub(crate) const COPY_IN: &str = "copy_in";
pub(crate) const LOAD_PARQUET: &str = "load_parquet";
pub(crate) const CLOSE: &str = "close";
/// The keyword arguments those methods take.
pub(crate) const STATEMENT: &str = "statement";
pub(crate) const QUERY: &str = "query";
pub(crate) const DATA: &str = "data";
pub(crate) const TABLE: &str = "table";
pub(crate) const FILE: &str = "file";
/// The class name of a driver's "doesn't exist" error, such as BigQuery's `google.api_core.exceptions.NotFound`.
pub(crate) const NOT_FOUND_ERROR: &str = "NotFound";
