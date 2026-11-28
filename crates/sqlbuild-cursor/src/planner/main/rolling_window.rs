//! Invocation-anchored rolling microbatch window.

use crate::algebra::errors::CursorError;
use crate::algebra::main::floor_to_grain::floor_to_grain;
use crate::algebra::main::next_boundary::next_boundary;
use crate::algebra::main::shift_by_duration::shift_by_duration;
use crate::algebra::models::{DurationSpan, Grain, Moment, Scalar, ScalarValue};

/// The `[clock floored to grain - span, end of the clock's period)` window of a UTC clock, so the current partial period is included.
pub fn rolling_window(
    clock: &Scalar,
    span: &DurationSpan,
    grain: Grain,
) -> Result<(Scalar, Scalar), CursorError> {
    let anchor: Scalar = floor_to_grain(clock, Some(grain));
    let ScalarValue::Timestamp(anchored) = &anchor.value else {
        return Err(CursorError::Type(
            "invocation clock must be a timestamp".into(),
        ));
    };
    let Moment::Timestamp { local, fold } = shift_by_duration(
        Moment::Timestamp {
            local: anchored.local,
            fold: anchored.fold,
        },
        span,
        -1,
    )?
    else {
        return Err(CursorError::Type(
            "invocation clock must be a timestamp".into(),
        ));
    };
    let mut start = anchored.clone();
    start.local = local;
    start.fold = fold;
    start.source_text = None;
    let end: Scalar = next_boundary(&anchor, Some(grain))?;
    Ok((
        Scalar {
            value: ScalarValue::Timestamp(start),
            handle: None,
        },
        end,
    ))
}
