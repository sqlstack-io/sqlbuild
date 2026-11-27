//! Plain data of grouped command progress rows.

use crate::build_progress::models::ErrorText;

/// One nested child row drawn under a completed item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NestedChildRow {
    pub label: String,
    pub name: String,
    pub status_text: String,
    pub detail: String,
}

/// One completed item with its child rows and error.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NestedItem {
    /// The full name; a row under its group shows it without the group's `<group>__` prefix.
    pub item_name: String,
    /// Where the item is among all of them, counting from one.
    pub position: usize,
    pub status_text: String,
    /// `local` or `warehouse`; empty when the command doesn't say.
    pub run_on: String,
    pub seconds: Option<f64>,
    /// Extra detail, such as `1 real model`, shown on wide terminals.
    pub detail: String,
    pub error: ErrorText,
    pub child_rows: Vec<NestedChildRow>,
}

/// The row label, longest shown name, terminal width and item count of one command's rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NestedLayout {
    pub label: String,
    /// The longest name a row shows, without its group prefix.
    pub name_width: usize,
    /// The terminal's width; `None` when output isn't a terminal or its width is unknown, so nothing is cut.
    pub columns: Option<usize>,
    pub total: usize,
    /// Whether passing items show their child rows too.
    pub verbose: bool,
}

/// Aggregate worker state of concurrently running audits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuditAggregate {
    pub running: usize,
    pub completed: usize,
    pub total: usize,
    pub workers: i64,
}
