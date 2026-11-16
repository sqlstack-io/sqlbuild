//! Refresh pacing and the size above which a state change is announced without its states.

use std::time::Duration;

/// Coalesce the change signals of a burst, such as a run's events, into one store read.
pub const NODE_STATES_DEBOUNCE: Duration = Duration::from_millis(500);
/// A change touching more nodes than this sends `full: true`, and clients refetch the states.
pub const MAX_PUSHED_NODE_STATES: usize = 2_000;
/// Pending state updates a slow change stream may fall behind before it is told to refetch.
pub(crate) const NODE_STATE_UPDATES: usize = 256;
/// The longest reason a failing changed build reports, in characters.
pub const MAX_FAILING_REASON_CHARS: usize = 2_000;
