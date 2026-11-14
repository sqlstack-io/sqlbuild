//! The Python rules as catalogue entries.

use crate::constants::NATIVE_BUILD_IDENTITY;
use crate::models::{RuleMetadata, RuleScope};
use crate::python_rules::constants::PYTHON_RULES;
use crate::python_rules::models::PythonRuleSpec;

/// Every `SQBRPYTHON` rule, reported against the project rather than a model.
#[must_use]
pub fn python_catalogue() -> Vec<RuleMetadata> {
    PYTHON_RULES.iter().map(metadata).collect()
}

fn metadata(rule: &PythonRuleSpec) -> RuleMetadata {
    RuleMetadata {
        code: rule.code.into(),
        family: "python".into(),
        slug: rule.slug.into(),
        message: rule.message.into(),
        remediation: rule.remediation.into(),
        guidance: None,
        implementation_fingerprint: NATIVE_BUILD_IDENTITY.into(),
        enabled_by_default: rule.enabled_by_default,
        project_wide: true,
        custom: false,
        scope: RuleScope::Python,
    }
}
