use crate::models::{CustomRule, RuleMetadata};

pub(crate) fn assemble_catalogue(custom: &[CustomRule]) -> Result<Vec<RuleMetadata>, String> {
    crate::rules::_helpers::dispatch::catalogue::with_custom(custom)
}
