//! A UTC invocation clock shifted by a duration.

use crate::algebra::errors::CursorError;
use crate::algebra::main::shift_by_duration::shift_by_duration;
use crate::algebra::models::{DurationSpan, Moment, Scalar, ScalarValue, Timestamp};

/// The clock's timestamp moved by `direction` times `span`, keeping its other fields.
pub(crate) fn shifted_clock(
    clock: &Scalar,
    span: &DurationSpan,
    direction: i64,
) -> Result<Timestamp, CursorError> {
    let ScalarValue::Timestamp(timestamp) = &clock.value else {
        return Err(CursorError::Type(
            "invocation clock must be a timestamp".into(),
        ));
    };
    let Moment::Timestamp { local, fold } = shift_by_duration(
        Moment::Timestamp {
            local: timestamp.local,
            fold: timestamp.fold,
        },
        span,
        direction,
    )?
    else {
        return Err(CursorError::Type(
            "invocation clock must be a timestamp".into(),
        ));
    };
    let mut shifted = timestamp.clone();
    shifted.local = local;
    shifted.fold = fold;
    shifted.source_text = None;
    Ok(shifted)
}
