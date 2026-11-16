//! Table rows for the ad-hoc planner.

/// A command, and the exact keys and settings it plans to, or a fragment of its refusal.
pub(super) struct AdhocPlanTestCase {
    pub(super) description: &'static str,
    pub(super) command: &'static [&'static str],
    pub(super) expected_plan: Result<(&'static [&'static str], serde_json::Value), &'static str>,
}

/// A selection as typed, and the keys it selects or how `sqb` refuses it.
pub(super) struct SelectionEvaluationTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static str,
    pub(super) exclude: &'static str,
    pub(super) expected_evaluation: Result<&'static [&'static str], ExpectedProblem>,
}

/// A refusal's code and message, the `(field, token, start, end)` it comes from, and the suggested replacements.
pub(super) type ExpectedProblem = (
    Option<&'static str>,
    &'static str,
    Option<(
        sqlbuild_orchestration::runs::models::SelectionField,
        &'static str,
        u32,
        u32,
    )>,
    &'static [&'static str],
);

/// One selector type's values, and how many nodes each selects.
pub(super) struct VocabularyTestCase {
    pub(super) description: &'static str,
    /// Which of the vocabulary's lists the case reads.
    pub(super) entries_of: fn(
        &sqlbuild_orchestration::runs::models::SelectionVocabulary,
    ) -> &[sqlbuild_orchestration::runs::models::VocabularyEntry],
    pub(super) expected_entries: &'static [(&'static str, u64)],
}
