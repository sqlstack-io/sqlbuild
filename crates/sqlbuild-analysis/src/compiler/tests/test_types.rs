use sqlbuild_sqltext::sql_scan::models::Unclosed;

pub(crate) struct SqlTestExtractionTestCase {
    pub(crate) description: &'static str,
    pub(crate) run: fn() -> bool,
    pub(crate) expected_success: bool,
}

pub(crate) struct SqlTestPlanningTestCase {
    pub(crate) description: &'static str,
    pub(crate) run: fn() -> bool,
    pub(crate) expected_success: bool,
}

pub(crate) struct SqlTestRenderingTestCase {
    pub(crate) description: &'static str,
    pub(crate) run: fn() -> bool,
    pub(crate) expected_success: bool,
}

pub(crate) struct ExpectedColumnsTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_columns: Option<&'static [&'static str]>,
}

pub(crate) struct TopLevelScanTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_unions: Result<Vec<&'static str>, String>,
    pub(crate) expected_commas: Result<Vec<&'static str>, String>,
    pub(crate) expected_from: Result<Option<usize>, String>,
}

pub(crate) type CteSlices = Option<(Vec<(&'static str, &'static str)>, &'static str)>;

pub(crate) struct CteSliceTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_slices: Result<CteSlices, Unclosed>,
}

pub(crate) struct CteRenameTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) renamed_index: usize,
    pub(crate) expected_sql: Option<&'static str>,
}

pub(crate) struct DefinedCteKeysTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_keys: &'static [&'static str],
}

pub(crate) struct MarkerNamesTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_names: &'static [&'static str],
    pub(crate) expected_replaced_sql: &'static str,
}

/// A table function call replaced by a name, with or without dropping a `LATERAL` before it.
pub(crate) struct CallableMarkerTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) drop_lateral: bool,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct ProtectedRangeTestCase {
    pub(crate) description: &'static str,
    pub(crate) ranges: &'static [(usize, usize)],
    pub(crate) expected_protected: &'static [usize],
}

pub(crate) struct RelationMarkersTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_calls: &'static [(&'static str, &'static str)],
}

pub(crate) struct RelationMarkerOracleTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) expected_mismatches: &'static [&'static str],
}

/// The rendered shape one analysis mode must produce: CTE order pairs and SQL fragments.
pub(crate) struct RenderedShapeTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) expected_order: &'static [(&'static str, &'static str)],
    pub(crate) expected_fragments: &'static [&'static str],
    pub(crate) expected_absent_fragments: &'static [&'static str],
}

pub(crate) struct HelperRenameTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_sql: Option<&'static str>,
}

/// One model and the helper named after something it reads.
pub(crate) struct SharedNameTestCase {
    pub(crate) description: &'static str,
    pub(crate) model_sql: &'static str,
    pub(crate) helper_name: &'static str,
    pub(crate) expected_model_reads: &'static str,
    pub(crate) expected_isolated: bool,
}

pub(crate) struct NameRefusalTestCase {
    pub(crate) description: &'static str,
    pub(crate) shape: PlanShape<'static>,
    pub(crate) expected_message: &'static str,
}

pub(crate) struct PlanShape<'a> {
    pub(crate) dialect: &'a str,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) model_sql: &'a str,
    pub(crate) helper_name: &'a str,
    pub(crate) expected_sql: &'a str,
}

pub(crate) struct HelperReferenceTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) helpers: &'static [(&'static str, &'static str)],
    pub(crate) expected: &'static [(&'static str, &'static str)],
    pub(crate) assertions: &'static [(&'static str, &'static str)],
    pub(crate) read_helpers: &'static [&'static str],
    pub(crate) reference_targets: &'static [&'static str],
    pub(crate) expected_chain: &'static [&'static str],
    pub(crate) expected_order: &'static [(&'static str, &'static str)],
    pub(crate) expected_fragments: &'static [&'static str],
    pub(crate) expected_absent_fragments: &'static [&'static str],
}

pub(crate) struct UnresolvedReaderReferenceTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) helpers: &'static [(&'static str, &'static str)],
    pub(crate) expected: &'static [(&'static str, &'static str)],
    pub(crate) assertions: &'static [(&'static str, &'static str)],
    pub(crate) read_helpers: &'static [&'static str],
    pub(crate) reference_targets: &'static [&'static str],
    pub(crate) sends_compiler_reads: bool,
    pub(crate) expected_error_fragments: &'static [&'static str],
}

pub(crate) struct ExtractionRuleTestCase {
    pub(crate) description: &'static str,
    pub(crate) mode: &'static str,
    pub(crate) raw: bool,
    pub(crate) syntax: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_outcome: String,
}

pub(crate) struct AuthoredCtesTestCase {
    pub(crate) description: &'static str,
    pub(crate) scenario: bool,
    pub(crate) sql: &'static str,
    pub(crate) expected_response: serde_json::Value,
}

pub(crate) struct GluePlanParityTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: Option<&'static str>,
    pub(crate) render_sql: bool,
    pub(crate) expected_plan_count: usize,
    pub(crate) expected_missing_mock_severity: &'static str,
}

pub(crate) struct GluePlannerErrorTestCase {
    pub(crate) description: &'static str,
    pub(crate) payload_field: &'static str,
    pub(crate) expected_error_prefix: &'static str,
}

pub(crate) struct GlueErrorProjectionTestCase {
    pub(crate) description: &'static str,
    pub(crate) warnings: &'static [(&'static str, &'static str)],
    pub(crate) expected_messages: &'static [&'static str],
}

pub(crate) struct SqlTestAssemblyTestCase {
    pub(crate) description: &'static str,
    pub(crate) batch: crate::compiler::models::SqlTestAssemblyBatch,
    pub(crate) expected_outcomes: Vec<crate::compiler::models::SqlTestAssemblyOutcome>,
}
#[derive(Debug)]
pub(crate) struct ScenarioExpectedColumnsTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_sql: &'static str,
    pub(crate) model_columns: Option<&'static [&'static str]>,
    pub(crate) all_columns: bool,
    pub(crate) expected_message: Option<&'static str>,
}

pub(crate) struct ScenarioHelperScopeTestCase {
    pub(crate) description: &'static str,
    pub(crate) reader_sql: &'static str,
    pub(crate) fixtures: bool,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct ScenarioHelperShadowTestCase {
    pub(crate) description: &'static str,
    pub(crate) check_name: &'static str,
    pub(crate) check_sql: &'static str,
    pub(crate) expected_rejected: bool,
}

pub(super) struct ProjectedUdfTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: &'static str,
    pub(super) expected_call: &'static str,
}
