//! Compound scheduler return values.

use chrono::{DateTime, Utc};

/// Expanded wall-clock times and the first omitted time, if the batch was bounded.
pub(crate) type ScheduleExpansion = (Vec<DateTime<Utc>>, Option<DateTime<Utc>>);
