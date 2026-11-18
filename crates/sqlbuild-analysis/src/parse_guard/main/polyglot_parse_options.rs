//! `polyglot_parse_options`: the options every SQLBuild Polyglot parse passes.

use polyglot_sql::ParseOptions;

use crate::parse_guard::main::polyglot_complexity_guard::polyglot_complexity_guard;

/// Polyglot parse options under SQLBuild's complexity guard.
#[must_use]
pub fn polyglot_parse_options() -> ParseOptions {
    ParseOptions {
        complexity_guard: Some(polyglot_complexity_guard()),
    }
}
