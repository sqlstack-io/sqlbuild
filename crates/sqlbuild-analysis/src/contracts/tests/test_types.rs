use crate::contracts::models::{ContractModel, PromotionRequest};

pub(crate) struct ContractTestCase {
    pub(crate) description: &'static str,
    pub(crate) implicit: bool,
    pub(crate) model: ContractModel,
    /// One `code severity location message` line per diagnostic, or `deferred:<kind>`.
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct SharedTypesTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: Vec<ContractModel>,
    /// Every model's lines in order, as each model's own single-model request yields them.
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct PromotionTestCase {
    pub(crate) description: &'static str,
    pub(crate) request: PromotionRequest,
    pub(crate) expected_models: &'static [usize],
    pub(crate) expected_help: Option<&'static str>,
}

pub(crate) struct UnknownDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    /// Whether the declared and inferred types are compared, so their mismatch is reported.
    pub(crate) expected_compared: bool,
}

pub(crate) struct EnumLiteralTestCase {
    pub(crate) description: &'static str,
    /// The authored model file; its query starts after the first `);\n`.
    pub(crate) authored: &'static str,
    /// The expanded query, when it differs from the authored one.
    pub(crate) expanded: Option<&'static str>,
    /// One `column value line:column` line per violation, `-` for an unlocated literal.
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct EnumLiteralMessageTestCase {
    pub(crate) description: &'static str,
    pub(crate) query: &'static str,
    /// `(code, message, help)` per violation.
    pub(crate) expected_violations: &'static [(&'static str, &'static str, &'static str)],
}

pub(crate) struct EnumLiteralDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) expected_count: usize,
}

pub(crate) struct CastingBuildTestCase {
    pub(crate) description: &'static str,
    pub(crate) declared_type: &'static str,
    pub(crate) inferred_type: &'static str,
    pub(crate) materialized: &'static str,
    pub(crate) incremental_mode: Option<&'static str>,
    pub(crate) staged_promotion: bool,
    /// Whether the column's type is reported as a K002 mismatch.
    pub(crate) expected_mismatch: bool,
}
