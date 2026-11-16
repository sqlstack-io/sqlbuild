//! Whether a rules request needs SQBRPERFORMANCE102's grain facts.

use crate::engine::constants::JSON_GRAIN_RULE_CODE;
use crate::models::EvaluateRequest;
use crate::query_facts::constants::SNOWFLAKE_DIALECT;
use crate::rules::main::request_selects::request_selects;

/// Whether `request` selects SQBRPERFORMANCE102 for a dialect it runs on.
pub fn needs_json_grains(request: &EvaluateRequest) -> Result<bool, String> {
    Ok(request.dialect.eq_ignore_ascii_case(SNOWFLAKE_DIALECT)
        && request_selects(request, |rule| rule.code == JSON_GRAIN_RULE_CODE)?)
}
