//! `default_rules_notice`: how to turn off configurable Rules, shown when unselected defaults report findings.

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

use crate::constants::SQL_RULE_PREFIX;

/// The notice when `select` is omitted and a finding comes from a default SQL Rule, otherwise `None`.
#[must_use]
pub fn default_rules_notice<'a>(
    select_omitted: bool,
    mut codes: impl Iterator<Item = &'a str>,
) -> Option<String> {
    (select_omitted && codes.any(|code| code.starts_with(SQL_RULE_PREFIX))).then(|| {
        format!(
            "Using default rules. To disable configurable checks, add to {PROJECT_CONFIG_FILENAME}:\n\n    [rules]\n    select = []\n\nAlready have [rules]? Add select = [] there. Compiler validation stays enabled."
        )
    })
}
