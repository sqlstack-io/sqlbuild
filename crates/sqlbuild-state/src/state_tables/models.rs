//! Columns of append-only state tables as their DDL declares them.

/// One declared column: its name, the adapter's rendered type and whether it is `NOT NULL`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableColumn {
    pub name: String,
    pub rendered_type: String,
    pub required: bool,
}

/// A bounded legacy-compatible history projection, with its optional source event ID.
#[derive(Clone, Copy, Debug)]
pub struct HistoryRead<'a> {
    pub columns: &'a [&'a str],
    pub timestamp_column: &'a str,
    pub observed_since: Option<&'a str>,
    pub adapter: &'a str,
    pub has_event_id: bool,
    pub limit: usize,
}
