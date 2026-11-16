use crate::models::{RuleMetadata, RulesConfig};

pub(crate) fn fingerprint(
    rules: &[&RuleMetadata],
    config: &RulesConfig,
    dialect: &str,
) -> Result<String, String> {
    crate::rules::_helpers::dispatch::catalogue::fingerprint(rules, config, dialect)
}
