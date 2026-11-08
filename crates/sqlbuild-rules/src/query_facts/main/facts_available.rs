//! `facts_available`: whether SQLBuild parses a dialect's SQL for built-in rules.

/// Whether SQLBuild parses `dialect`'s SQL for the built-in rules that read query facts.
pub(crate) fn facts_available(dialect: &str) -> bool {
    crate::query_facts::_helpers::facts::facts_dialect(dialect).is_some()
}
