//! Shapes the env file loader passes between its steps.

/// The chosen file's `KEY=value` pairs, `None` when nothing was read, and its permissions warning.
pub(crate) type ChosenPairs = (Option<Vec<(String, String)>>, Option<String>);
