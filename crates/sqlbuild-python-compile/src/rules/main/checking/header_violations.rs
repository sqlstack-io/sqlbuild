//! The header rule violations of `headers` in `contents`.

use crate::rules::models::NativeHeaderParser;
use sqlbuild_rules::dsl_headers::main::lint_headers::lint_headers;
use sqlbuild_rules::dsl_headers::models::{HeaderRulesConfig, HeaderSpan, HeaderViolation};

/// The header rule violations of `headers` in `contents`.
pub fn header_violations(
    contents: &[char],
    headers: &[HeaderSpan],
    config: HeaderRulesConfig,
) -> Result<Vec<HeaderViolation>, String> {
    lint_headers(contents, headers, config, &NativeHeaderParser)
}
