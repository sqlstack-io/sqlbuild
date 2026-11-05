use serde_json::Value;

use sqlbuild_sqltext::sql_scan::models::Unclosed;

pub(crate) struct NativeEvaluationTestCase {
    pub(crate) description: &'static str,
    pub(crate) config: Value,
    pub(crate) expected_faults: Value,
}

pub(crate) struct ModelLayerRulesTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_codes: &'static [&'static str],
}

pub(crate) struct GraphRuleTestCase {
    pub(crate) description: &'static str,
    pub(crate) exceptions: Value,
    pub(crate) expected_fault_count: usize,
    pub(crate) expected_message_fragments: &'static [&'static str],
}

pub(crate) struct DynamicPivotEvaluationTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_faults: Value,
}

pub(crate) struct DeclarationScopeTestCase {
    pub(crate) description: &'static str,
    pub(crate) scope: &'static str,
    pub(crate) path: &'static str,
    pub(crate) expected_fault_count: usize,
}

pub(crate) struct DialectEvaluationTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) query_sql: &'static str,
    pub(crate) expected_code: &'static str,
}

pub(crate) struct ExplicitOutputTypeTestCase {
    pub(crate) description: &'static str,
    pub(crate) query_sql: &'static str,
    pub(crate) columns: Value,
    pub(crate) contract: &'static str,
    pub(crate) expected_fault_count: usize,
    pub(crate) expected_message_fragment: &'static str,
}

pub(crate) struct ContractNameTypeOptionsTestCase {
    pub(crate) description: &'static str,
    pub(crate) config: Value,
    pub(crate) expected_fault_codes: &'static [&'static str],
}

pub(crate) struct NumericDecisionTestCase {
    pub(crate) description: &'static str,
    pub(crate) query_sql: &'static str,
    pub(crate) authored_sql: &'static str,
    pub(crate) expected_fault_count: usize,
}

pub(crate) struct TypedContractColumnTestCase {
    pub(crate) description: &'static str,
    pub(crate) contract: &'static str,
    pub(crate) columns: Value,
    pub(crate) expected_fault_count: usize,
    pub(crate) expected_messages: &'static [&'static str],
}

pub(crate) struct ShapeTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_calls: &'static [&'static str],
    pub(crate) expected_fragment: &'static str,
}

pub(crate) struct DeepExpressionTestCase {
    pub(crate) description: &'static str,
    pub(crate) depth: usize,
    pub(crate) expected_code: &'static str,
}

pub(crate) struct DomainLayoutTestCase {
    pub(crate) description: &'static str,
    pub(crate) code: &'static str,
    pub(crate) models: Value,
    pub(crate) thresholds: Value,
    pub(crate) layout: Value,
    pub(crate) scope_index: Value,
    pub(crate) expected_codes: &'static [&'static str],
    pub(crate) expected_message_fragments: &'static [&'static str],
    pub(crate) expected_absent_fragments: &'static [&'static str],
}

pub(crate) struct ThresholdEvaluationTestCase {
    pub(crate) description: &'static str,
    pub(crate) config: Value,
    pub(crate) query_sql: &'static str,
    pub(crate) references: Value,
    pub(crate) expected_codes: &'static [&'static str],
}

pub(crate) struct BuildIdentityTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_hash_length: usize,
}

pub(crate) struct HashedWorkspaceFileTestCase {
    pub(crate) description: &'static str,
    pub(crate) workspace_file: &'static str,
    pub(crate) expected_hashed: bool,
}

pub(crate) struct ThresholdFingerprintTestCase {
    pub(crate) description: &'static str,
    pub(crate) base_config: Value,
    pub(crate) overridden_config: Value,
    pub(crate) expected_different: bool,
}

pub(crate) struct ScopeFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) scope: Value,
    pub(crate) expected_complete: bool,
    pub(crate) expected_runtime_usage: bool,
    pub(crate) expected_declaration_count: usize,
    pub(crate) expected_visibility_count: usize,
}

pub(crate) struct MalformedScopeFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) scope: Value,
    pub(crate) expected_rejected: bool,
}

pub(crate) struct MissingScopeFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_complete: bool,
    pub(crate) expected_runtime_usage: bool,
}

pub(crate) struct SqlTestRulesTestCase {
    pub(crate) description: &'static str,
    pub(crate) code: &'static str,
    pub(crate) tests: Value,
    pub(crate) scenarios: Value,
    pub(crate) scope_index: Value,
    pub(crate) config: Value,
    pub(crate) expected_fault_count: usize,
    pub(crate) expected_evaluated_models: usize,
    pub(crate) expected_paths: &'static [&'static str],
}

pub(crate) struct SqlTestRulesCacheTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_first_misses: u64,
    pub(crate) expected_second_hits: u64,
    pub(crate) expected_second_misses: u64,
    pub(crate) expected_first_codes: &'static [&'static str],
    pub(crate) expected_second_codes: &'static [&'static str],
}

pub(crate) struct EmptyInputTestRuleTestCase {
    pub(crate) description: &'static str,
    pub(crate) test: Value,
    pub(crate) allowed_tests: Value,
    pub(crate) expected_messages: &'static [&'static str],
}

pub(crate) struct EmptyInputMinimumTestsTestCase {
    pub(crate) description: &'static str,
    pub(crate) select: Value,
    pub(crate) tests: Value,
    pub(crate) allowed_tests: Value,
    pub(crate) expected_messages: &'static [&'static str],
}

/// One SQL fragment scanned by every native quote- and comment-aware scanner.
pub(crate) struct DollarQuoteSiteTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) expected_sites: &'static [&'static str],
}

pub(crate) struct LintDialectSiteTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) fragment: &'static str,
    pub(crate) expected_lint_site: &'static str,
}

pub(crate) struct SqlScannerTestCase {
    pub(crate) description: &'static str,
    pub(crate) fragment: &'static str,
    pub(crate) expected_compiler_paren: Result<usize, Unclosed>,
    pub(crate) expected_dependency_calls: &'static [&'static str],
    pub(crate) expected_compact: &'static str,
    pub(crate) expected_snowflake_compact: &'static str,
    /// `None` when SQL lint declines non-ASCII input; `Some("")` when no macro site closes.
    pub(crate) expected_lint_site: Option<&'static str>,
}

pub(crate) struct IncrementalRulesTestCase {
    pub(crate) description: &'static str,
    pub(crate) cache_enabled: bool,
    pub(crate) compared_fields: &'static [&'static str],
    pub(crate) expected_mismatched_fields: &'static [&'static str],
}

pub(crate) struct RuleScopeTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_scopes: &'static [(&'static str, crate::models::RuleScope)],
    pub(crate) expected_sql_scope: crate::models::RuleScope,
}

pub(crate) struct ScopedEvaluationTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_codes: &'static [&'static str],
}

pub(crate) struct MalformedSplitRequestTestCase {
    pub(crate) description: &'static str,
    pub(crate) inline_model_count: usize,
    pub(crate) payload_count: usize,
    pub(crate) expected_error: &'static str,
}

pub(crate) struct ModelCacheReuseTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_hits: u64,
    pub(crate) expected_misses: u64,
}

pub(crate) struct ConfiguredCachePathTestCase {
    pub(crate) description: &'static str,
    pub(crate) cache_file: &'static str,
    pub(crate) expected_default_cache_exists: bool,
    pub(crate) expected_warm_hits: u64,
}

pub(crate) struct CachePruningTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_paths: &'static [&'static str],
}
pub(crate) struct DependencyTokenTestCase {
    pub description: &'static str,
    pub dialect: &'static str,
    pub query_sql: &'static str,
    pub expected_messages: &'static [&'static str],
    pub expected_columns: &'static [usize],
}

pub(crate) struct SpeculativeRowsTestCase {
    pub(crate) description: &'static str,
    pub(crate) cache_enabled: bool,
    pub(crate) changed_query: Option<&'static str>,
    pub(crate) expected_used: bool,
}

pub(crate) struct SpeculativeStopTestCase {
    pub(crate) description: &'static str,
    pub(crate) end: fn(crate::models::SpeculativeRows, &std::path::Path) -> Result<bool, String>,
    pub(crate) expected_stopped: bool,
}

pub(crate) struct SpeculativeOutcomeTestCase {
    pub(crate) description: &'static str,
    pub(crate) outcome: fn() -> crate::engine::_helpers::memo::SpeculativeOutcome,
    pub(crate) expected_clean: bool,
}

pub(crate) struct QueryFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) query_sql: &'static str,
    pub(crate) select: &'static [&'static str],
    pub(crate) enum_columns: &'static [&'static str],
    pub(crate) columns: Value,
    pub(crate) expected_faults: &'static [(&'static str, u64, u64, &'static str)],
}

/// A retained request evaluated again without SQL, with or without the cached findings.
pub(crate) struct RetainedRowsTestCase {
    pub(crate) description: &'static str,
    pub(crate) fresh_cache: bool,
    pub(crate) expected_same_findings: bool,
    pub(crate) expected_missed: Option<&'static str>,
}

/// Model rows written in the native record format and read back.
pub(crate) struct RowCodecTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: fn() -> Vec<crate::models::Model>,
    /// How many truncations of the rows still read back.
    pub(crate) expected_truncations_read: usize,
}

pub(crate) struct PublishedLayerTestCase {
    pub(crate) description: &'static str,
    pub(crate) code: &'static str,
    pub(crate) models: &'static [(&'static str, &'static [&'static str])],
    pub(crate) options: Value,
    pub(crate) expected_faults: &'static [(&'static str, &'static str)],
}

pub(crate) struct RelationExceptionTestCase {
    pub(crate) description: &'static str,
    pub(crate) exceptions: Value,
    pub(crate) expected_messages: &'static [&'static str],
}

pub(crate) struct StaleRelationExceptionTestCase {
    pub(crate) description: &'static str,
    pub(crate) exceptions: Value,
    pub(crate) expected_error: &'static str,
}

pub(crate) struct DialectCallsTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) expected_calls: &'static [&'static str],
}
