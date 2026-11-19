//! `sqlquack_parse_options`: the options every SQLBuild SQLQuack parse passes.

use sqlquack_syntax::{ComplexityGuardOptions, ParseOptions};

use crate::parse_guard::constants::MAX_FUNCTION_CALL_DEPTH;

/// SQLQuack's default complexity limits, allowing SQLBuild's function-call depth.
#[must_use]
pub fn sqlquack_parse_options() -> ParseOptions {
    ParseOptions {
        complexity_guard: Some(ComplexityGuardOptions {
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            ..ComplexityGuardOptions::default()
        }),
    }
}
