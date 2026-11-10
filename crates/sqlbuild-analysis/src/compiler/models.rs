use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqlTestFixtureFacts {
    pub mock: bool,
    pub empty_fixture_marker: bool,
}

/// One batch of SQL tests to plan against the project's models and functions.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanBatch {
    pub models: Vec<SqlTestPlanModel>,
    #[serde(default)]
    pub functions: Vec<SqlTestPlanFunction>,
    pub tests: Vec<SqlTestPlanTest>,
    #[serde(default = "crate::compiler::_helpers::sql_tests::glue::default_true")]
    pub sql_analysis_enabled: bool,
    #[serde(default)]
    pub sql_analysis_dialect: Option<String>,
    #[serde(default = "crate::compiler::_helpers::sql_tests::glue::default_set_difference")]
    pub set_difference_operator: String,
    #[serde(default)]
    pub requires_derived_table_aliases: bool,
    #[serde(default = "crate::compiler::_helpers::sql_tests::glue::default_workers")]
    pub workers: usize,
    #[serde(default = "crate::compiler::_helpers::sql_tests::glue::default_true")]
    pub render_sql: bool,
    #[serde(default = "crate::compiler::_helpers::sql_tests::glue::default_true")]
    pub include_plan: bool,
    pub lexical_syntax: LexicalSyntax,
    #[serde(default)]
    pub max_unit_test_real_models: Option<usize>,
}

/// The tests whose model chains to resolve; chain ordering reads only declared dependencies.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestChainBatch {
    pub models: Vec<SqlTestChainModel>,
    pub tests: Vec<SqlTestPlanTest>,
    pub lexical_syntax: LexicalSyntax,
}

/// One project model's name and the models it depends on.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestChainModel {
    pub name: String,
    #[serde(default)]
    pub model_dependencies: Vec<String>,
}

/// One project model as SQL tests read it: its SQL and the models it depends on.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanModel {
    pub name: String,
    pub query_sql: String,
    #[serde(default)]
    pub model_dependencies: Vec<String>,
}

/// One project function and the adapter's call templates around its arguments.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanFunction {
    pub name: String,
    #[serde(default)]
    pub udf_prefix: Option<String>,
    #[serde(default)]
    pub udf_suffix: Option<String>,
    #[serde(default)]
    pub table_function_prefix: Option<String>,
    #[serde(default)]
    pub table_function_suffix: Option<String>,
}

/// One compiled SQL test to plan.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanTest {
    pub name: String,
    pub file_label: String,
    pub payload: SqlTestPlanPayload,
}

/// A model-mode test's CTEs and overrides, or a direct-logic test's actual and expected CTEs.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SqlTestPlanPayload {
    Model {
        #[serde(default)]
        authored_ctes: Vec<SqlTestCte>,
        #[serde(default)]
        model_query_overrides: BTreeMap<String, String>,
        #[serde(default)]
        expected_ctes: Vec<SqlTestCte>,
        #[serde(default)]
        expected_model_names: Vec<String>,
        #[serde(default)]
        assertion_ctes: Vec<SqlTestCte>,
        #[serde(default)]
        read_helper_names: Option<Vec<String>>,
        #[serde(default)]
        reference_target_model_names: Option<Vec<String>>,
    },
    Direct {
        mode: String,
        actual_cte: SqlTestCte,
        expected_cte: SqlTestCte,
        #[serde(default)]
        helper_ctes: Vec<SqlTestCte>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestCte {
    pub name: String,
    pub sql_body: String,
}

/// One planned SQL test: the executable chain and assertion steps plus optional rendered SQL.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlan {
    pub sql: Option<String>,
    pub chain: Vec<SqlTestChainStep>,
    pub assertions: Vec<SqlTestAssertionStep>,
    pub model_names: Vec<String>,
    pub warnings: Vec<SqlTestPlanWarning>,
}

/// Every planned test in request order, with the batch's wall time split by phase.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanBatchOutcome {
    #[serde(rename = "artifacts")]
    pub plans: Vec<SqlTestPlan>,
    pub planning_ns: u64,
    pub rendering_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestPlanWarning {
    pub model_name: Option<String>,
    pub severity: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestChainStep {
    pub model_name: String,
    pub resolved_sql: String,
    #[serde(default)]
    pub expected_cte_sql: Option<String>,
    #[serde(default)]
    pub expected_lifted_ctes: Vec<(String, String)>,
    #[serde(default)]
    pub lifted_ctes: Vec<(String, String)>,
    #[serde(default)]
    pub comparison_body_sql: Option<String>,
    #[serde(default)]
    pub expected_columns: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestAssertionStep {
    pub name: String,
    pub resolved_sql: String,
    #[serde(default)]
    pub lifted_ctes: Vec<(String, String)>,
    #[serde(default)]
    pub comparison_body_sql: Option<String>,
}

/// The compiled SQL tests to assemble, with the facts of every model the project compiles.
#[derive(Debug)]
pub struct SqlTestAssemblyBatch {
    pub models: Vec<SqlTestAssemblyModel>,
    pub tests: Vec<SqlTestAssemblyTest>,
    pub lexical_syntax: LexicalSyntax,
    /// The project's SQL analysis dialect, whose native parser reads helper references.
    pub sql_analysis_dialect: Option<String>,
}

/// What assembly reads of one model: its name, macro dependencies and references.
#[derive(Debug)]
pub struct SqlTestAssemblyModel {
    pub name: String,
    pub macro_deps: Vec<String>,
    /// Pre-macro SQL Python scans for calls: set only without macro deps and with a macro token.
    pub unscanned_macro_source: Option<String>,
    pub references: Vec<SqlTestAssemblyReference>,
}

/// One logical reference: Python `SqlReferenceKind` value, name and optional package.
#[derive(Clone, Debug)]
pub struct SqlTestAssemblyReference {
    pub kind: String,
    pub name: String,
    pub package: Option<String>,
}

/// One expanded SQL test case as compile inputs carry it.
#[derive(Debug)]
pub struct SqlTestAssemblyTest {
    pub block_name: Option<String>,
    pub file_stem: String,
    pub relative_path: String,
    pub relative_stem: String,
    pub contents: String,
    pub block_sql: String,
    pub block_index: i64,
    pub sql_body: String,
    pub case_name: Option<String>,
    /// `(name, value kind, nullable)` per declared parameter.
    pub parameter_schema: Vec<(String, String, bool)>,
    pub parameter_values: Vec<(String, SqlTestParameterValue)>,
    pub payload: SqlTestAssemblyPayload,
}

/// A typed SQL value as its kind and payload; decimals keep their unnormalized digits.
#[derive(Clone, Debug)]
pub enum SqlTestParameterValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Float(f64),
    Decimal {
        negative: bool,
        digits: Vec<u8>,
        exponent: i64,
    },
    Null,
    List(Vec<SqlTestParameterValue>),
    Set(Vec<SqlTestParameterValue>),
    Object(Vec<(String, SqlTestParameterValue)>),
}

/// The part of a test's payload assembly reads.
#[derive(Debug)]
pub enum SqlTestAssemblyPayload {
    /// A macro, UDF or table-function test of the named resources.
    Direct {
        mode: String,
        tested_resource_names: Vec<String>,
    },
    Model(SqlTestAssemblyModelPayload),
}

#[derive(Debug)]
pub struct SqlTestAssemblyModelPayload {
    pub authored_ctes: Vec<SqlTestCte>,
    pub expected_ctes: Vec<SqlTestCte>,
    pub assertion_ctes: Vec<SqlTestCte>,
    pub expected_model_names: Vec<String>,
    pub assertion_target_model_names: Vec<String>,
    pub reference_target_model_names: Vec<String>,
    pub mock_model_names: Vec<String>,
}

/// One test's assembled facts, or why its assembly raises.
#[derive(Debug, PartialEq)]
pub enum SqlTestAssemblyOutcome {
    Assembled(AssembledSqlTestFacts),
    /// Python's decimal context overflows normalizing a case parameter.
    FingerprintOverflow(AssembledSqlTestFacts),
    Failed(SqlTestAssemblyFailure),
}

/// The facts Python's `_assemble_compiled_sql_test` computes for one test.
#[derive(Debug, PartialEq)]
pub struct AssembledSqlTestFacts {
    pub name: String,
    /// `(resource type, name)` per scope dependency, in Python's order.
    pub scope_deps: Vec<(&'static str, String)>,
    pub target_model_names: Vec<String>,
    pub case_fingerprint: Option<String>,
    /// Resource name of the test's diagnostics: the block name, else the relative path's stem.
    pub diagnostic_resource_name: String,
    pub diagnostics: Vec<SqlTestHelperDiagnostic>,
}

/// One P013 diagnostic for a mock that reads a helper calling a reference.
#[derive(Debug, PartialEq, Eq)]
pub struct SqlTestHelperDiagnostic {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub message: String,
    pub help: String,
}

/// Why one test's assembly raises, as Python's assembly would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqlTestAssemblyFailure {
    /// The message of the `CompileInputError` Python raises before the test's macro-mock queries.
    Input(String),
    /// An internal native failure.
    Internal(String),
}

/// One SQL test block to extract: its SQL, file label, mode and extraction stage.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTestRequest {
    pub sql: String,
    pub file_label: String,
    pub mode: String,
    /// The test before macro and variable expansion, where macro calls are still authored.
    #[serde(default)]
    pub raw: bool,
    /// Read only the CTE names and body offsets of the authored block, before any expansion.
    #[serde(default)]
    pub authored: bool,
    /// The authored text is a scenario rather than a SQL test block.
    #[serde(default)]
    pub scenario: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlTestCteText(pub String, pub String);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SqlTestClassification {
    /// Each CTE's name, its code-point offset in the authored block and its stripped body.
    Authored { ctes: Vec<(String, usize, String)> },
    Model {
        mode: String,
        authored: Vec<SqlTestCteText>,
        #[serde(rename = "macroMocks")]
        macro_mocks: Vec<(String, String)>,
        #[serde(rename = "mockModels")]
        mock_models: Vec<String>,
        #[serde(rename = "mockSources")]
        mock_sources: Vec<String>,
        #[serde(rename = "mockSeeds")]
        mock_seeds: Vec<String>,
        #[serde(rename = "mockDbtRefs")]
        mock_dbt_refs: Vec<String>,
        #[serde(rename = "mockTableFunctions")]
        mock_table_functions: Vec<String>,
        expected: Vec<SqlTestCteText>,
        #[serde(rename = "expectedModels")]
        expected_models: Vec<String>,
        assertions: Vec<SqlTestCteText>,
        #[serde(rename = "assertionNames")]
        assertion_names: Vec<String>,
    },
    Direct {
        mode: String,
        helpers: Vec<SqlTestCteText>,
        actual: SqlTestCteText,
        expected: SqlTestCteText,
        /// Whether a helper or the expected CTE has a malformed reference call (P012).
        #[serde(rename = "invalidCalls")]
        invalid_calls: bool,
    },
}

/// Why extracting one test of a batch failed, and the authored text the author can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlTestExtractionFailure {
    /// The failing test's position in the batch.
    pub index: usize,
    pub message: String,
    pub help: Option<String>,
    /// The offending text and its code-point offset in the test SQL.
    pub token: Option<(String, usize)>,
}

/// One scenario's top-level CTEs, classified by role.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlScenarioCtes {
    pub authored: Vec<(String, String)>,
    pub expected: Vec<(String, String)>,
    pub assertions: Vec<(String, String)>,
    pub source_fixtures: Vec<String>,
    pub ref_fixtures: Vec<String>,
    pub seed_fixtures: Vec<String>,
    pub dbt_ref_fixtures: Vec<String>,
    pub expected_models: Vec<String>,
    pub assertion_names: Vec<String>,
}

/// A scenario expected CTE's authored columns and how they differ from its model's output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScenarioExpectedColumns {
    /// The columns the CTE lists as authored, or `None` when they are not explicit.
    pub listed: Option<Vec<String>>,
    /// Model output columns the CTE does not list.
    pub unlisted: Vec<String>,
    /// Listed columns the model does not output.
    pub unknown: Vec<String>,
}

/// The CTEs a scenario's checks may read: helpers as authored, fixtures by their stand-in bodies.
#[derive(Debug)]
pub struct ScenarioScopeInput<'a> {
    /// Every non-check CTE by authored name; a fixture body is what a check reading it sees.
    pub ctes: &'a [(String, String)],
    /// The scenario's `__expected__` and `__assert__` CTEs, checked against helper names.
    pub checks: &'a [(String, String)],
    pub lexical: LexicalSyntax,
    pub dialect: Option<&'a str>,
    pub file_label: &'a str,
}

/// One scenario file's helper and fixture CTEs, indexed once for its fixtures and checks.
#[derive(Debug)]
pub struct ScenarioHelperScope {
    pub(crate) graph: crate::compiler::_helpers::sql_tests::helper_scope::ScopeGraph,
    pub(crate) patterns: crate::compiler::_helpers::sql_tests::planning::SqlTestPatterns,
}
