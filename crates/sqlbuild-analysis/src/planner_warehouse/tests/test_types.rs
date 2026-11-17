//! Cases for warehouse-inspection planning decisions.

pub(super) struct CursorGroupTestCase {
    pub description: &'static str,
    pub requests: Vec<crate::planner_warehouse::models::CursorTagRequest>,
    pub expected_sql: Vec<&'static str>,
    pub expected_identities: Vec<&'static str>,
    pub expected_batch_sql: &'static str,
}

pub(super) struct RetentionDirectionTestCase {
    pub description: &'static str,
    pub delta_values: [Option<i64>; 2],
    pub effective_days: i64,
    pub desired_days: i64,
    pub expected_direction: &'static str,
}

pub(super) struct RetentionChangeTestCase {
    pub description: &'static str,
    pub change_phase: &'static str,
    pub fallback: &'static str,
    pub expected_change: (&'static str, &'static str),
}

pub(super) struct CopyNameTestCase {
    pub description: &'static str,
    pub target_name: &'static str,
    pub identifier_limit: i64,
    pub expected_name: &'static str,
}

pub(super) struct SourceCallTestCase {
    pub description: &'static str,
    pub sql: &'static str,
    pub expected_names: Vec<&'static str>,
}

pub(super) struct DeferralFieldTestCase {
    pub description: &'static str,
    pub target_value: Option<&'static str>,
    pub logical_value: Option<&'static str>,
    pub expected_value: Option<&'static str>,
}

pub(super) struct FingerprintLookupTestCase {
    pub description: &'static str,
    pub stored_fingerprint: Option<&'static str>,
    pub tamper: Option<(&'static str, &'static str)>,
    pub lookup_key: &'static str,
    pub expected_lookup: Option<Option<&'static str>>,
}

pub(super) struct MissingSourceTestCase {
    pub description: &'static str,
    pub missing: Vec<crate::planner_warehouse::models::MissingSourceTable>,
    pub expected_message: &'static str,
    pub expected_help: &'static str,
}

pub(super) struct FreshnessMessageTestCase {
    pub description: &'static str,
    pub reason: &'static str,
    pub names: Vec<String>,
    pub expected_message: Option<String>,
}

pub(super) struct RebuildCauseTestCase {
    pub description: &'static str,
    pub facts: crate::planner_warehouse::models::FullRebuildFacts,
    pub expected_cause: &'static str,
}

pub(super) struct FingerprintDigestTestCase {
    pub description: &'static str,
    pub key: crate::planner_warehouse::models::FingerprintCacheKey,
    pub expected_key: &'static str,
    pub expected_shared: &'static str,
    pub expected_entry: &'static str,
}
