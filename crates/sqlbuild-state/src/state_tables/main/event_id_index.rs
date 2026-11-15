//! Where a history read's row carries its event ID.

use crate::state_tables::constants::EVENT_ID_COLUMN;

/// A selected `event_id` column, else the one the history read appends after the columns.
#[must_use]
pub fn event_id_index(columns: &[&str]) -> usize {
    columns
        .iter()
        .position(|column| *column == EVENT_ID_COLUMN)
        .unwrap_or(columns.len())
}
