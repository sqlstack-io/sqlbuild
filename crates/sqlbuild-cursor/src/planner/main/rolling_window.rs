//! Invocation-anchored rolling microbatch window.

use crate::algebra::errors::CursorError;
use crate::algebra::main::floor_to_grain::floor_to_grain;
use crate::algebra::main::next_boundary::next_boundary;
use crate::algebra::models::{DurationSpan, Grain, Scalar, ScalarValue};
use crate::planner::_helpers::shifted_clock::shifted_clock;

/// The `[clock floored to grain - span, end of the clock's period)` window of a UTC clock, so the current partial period is included.
pub fn rolling_window(
    clock: &Scalar,
    span: &DurationSpan,
    grain: Grain,
) -> Result<(Scalar, Scalar), CursorError> {
    let anchor: Scalar = floor_to_grain(clock, Some(grain));
    let start = shifted_clock(&anchor, span, -1)?;
    let end: Scalar = next_boundary(&anchor, Some(grain))?;
    Ok((
        Scalar {
            value: ScalarValue::Timestamp(start),
            handle: None,
        },
        end,
    ))
}
