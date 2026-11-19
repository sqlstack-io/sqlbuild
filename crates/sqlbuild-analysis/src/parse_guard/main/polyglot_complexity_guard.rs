//! `polyglot_complexity_guard`: Polyglot's default limits with SQLBuild's function-call depth.

use polyglot_sql::ComplexityGuardOptions;

use crate::parse_guard::constants::MAX_FUNCTION_CALL_DEPTH;

/// Polyglot's default complexity limits, allowing SQLBuild's function-call depth.
#[must_use]
pub fn polyglot_complexity_guard() -> ComplexityGuardOptions {
    ComplexityGuardOptions {
        max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
        ..ComplexityGuardOptions::default()
    }
}
