//! Map local cron times without using the cron dependency's different fold policy.

use chrono::{DateTime, LocalResult, NaiveDateTime, TimeZone, Utc};
use chrono_tz::{GapInfo, Tz};

use crate::store::errors::ControlError;

/// During the first fold, smaller wall times in the second fold may still be in the future.
pub(crate) fn search_start(timezone: Tz, after: DateTime<Utc>) -> NaiveDateTime {
    let local = after.with_timezone(&timezone).naive_local();
    match timezone.from_local_datetime(&local) {
        LocalResult::Ambiguous(earlier, later) if after < later.to_utc() => {
            local - (later.to_utc() - earlier.to_utc())
        }
        _ => local,
    }
}

pub(crate) fn resolve_wall_time(
    timezone: Tz,
    wall_time: NaiveDateTime,
) -> Result<DateTime<Utc>, ControlError> {
    match timezone.from_local_datetime(&wall_time) {
        LocalResult::Single(time) => Ok(time.to_utc()),
        LocalResult::Ambiguous(_, later) => Ok(later.to_utc()),
        LocalResult::None => GapInfo::new(&wall_time, &timezone)
            .and_then(|gap| gap.end)
            .map(|time| time.to_utc())
            .ok_or_else(|| ControlError::Invalid("cron time has no representable gap end".into())),
    }
}
