use crate::models::RuleMetadata;

pub fn catalogue() -> Vec<RuleMetadata> {
    crate::rules::_helpers::dispatch::catalogue::catalogue()
}
