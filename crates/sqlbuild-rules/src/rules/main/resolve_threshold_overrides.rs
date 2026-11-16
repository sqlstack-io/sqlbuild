use crate::models::RulesConfig;
use crate::rules::models::ResolvedThresholdOverride;

pub(crate) fn resolve_threshold_overrides(
    config: &RulesConfig,
) -> Result<Vec<ResolvedThresholdOverride>, String> {
    crate::rules::_helpers::dispatch::evaluation::resolve_threshold_overrides(config)
}
