//! The bounds a location's reported rule findings must stay within.

use crate::platform::constants::{
    MAX_PUBLISH_ERROR_CHARS, MAX_REPORTED_RULE_FINDINGS, MAX_RULE_FINDING_CHARS,
};
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;

/// Refuse a compile failure whose error or rule findings exceed the reported bounds.
pub(crate) fn check_rule_findings(report: &LocationHealthReport) -> Result<(), ControlError> {
    let LocationHealth::CompileFailed { rules, error, .. } = &report.health else {
        return Ok(());
    };
    if error
        .as_deref()
        .is_some_and(|error| error.chars().count() > MAX_PUBLISH_ERROR_CHARS)
    {
        return Err(ControlError::Invalid(
            "location load error exceeds the reported bounds".into(),
        ));
    }
    let Some(rules) = rules else {
        return Ok(());
    };
    let oversized = |text: &str| text.chars().count() > MAX_RULE_FINDING_CHARS;
    if rules.findings.len() > MAX_REPORTED_RULE_FINDINGS
        || rules.finding_count < rules.findings.len() as u64
        || rules.findings.iter().any(|finding| {
            oversized(&finding.code)
                || oversized(&finding.message)
                || finding.path.as_deref().is_some_and(oversized)
        })
    {
        return Err(ControlError::Invalid(
            "location rule findings exceed the reported bounds".into(),
        ));
    }
    Ok(())
}
