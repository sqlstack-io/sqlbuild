//! Invocation-anchored automatic cursor horizon shared by both safety policies.

use crate::algebra::errors::CursorError;
use crate::algebra::main::floor_to_grain::floor_to_grain;
use crate::algebra::models::{DurationSpan, Grain, Scalar, ScalarValue};
use crate::planner::_helpers::shifted_clock::shifted_clock;

/// Shift a UTC invocation clock, retaining the discovered cursor's date representation.
pub fn safety_horizon(
    clock: &Scalar,
    duration: &DurationSpan,
    grain: Grain,
    date_only: bool,
) -> Result<Scalar, CursorError> {
    let maximum = shifted_clock(clock, duration, 1)?;
    let value = if date_only {
        ScalarValue::Date(maximum.local.date)
    } else {
        ScalarValue::Timestamp(maximum)
    };
    Ok(floor_to_grain(
        &Scalar {
            value,
            handle: None,
        },
        Some(grain),
    ))
}
