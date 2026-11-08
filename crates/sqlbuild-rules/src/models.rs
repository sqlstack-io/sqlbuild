use serde::{Deserialize, Serialize};
use sqlbuild_analysis::sql_macros::models::TypedCall;

use crate::errors::RowsError;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::constants::{
    API_VERSION, BUILT_IN_RULE_NAMESPACE, CUSTOM_RULE_NAMESPACE, RULE_CODE_NUMBER_LENGTH,
    SCOPE_METADATA_SCHEMA_VERSION, SQL_RULE_PREFIX,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RulesCodeGrammar;

impl fensu_policy::policy::types::RuleCodeGrammar for RulesCodeGrammar {
    fn rule_code_is_exact(&self, value: &str) -> bool {
        let Some((namespace, remainder)) = Self::namespace_and_remainder(value) else {
            return false;
        };
        if remainder.len() < RULE_CODE_NUMBER_LENGTH {
            return false;
        }
        let (family, number) = remainder.split_at(remainder.len() - RULE_CODE_NUMBER_LENGTH);
        let family_valid = family.bytes().all(|value| value.is_ascii_uppercase());
        let number_valid = number.bytes().all(|value| value.is_ascii_digit());
        let family_shape = match namespace {
            CUSTOM_RULE_NAMESPACE => true,
            SQL_RULE_PREFIX => family.is_empty(),
            _ => !family.is_empty(),
        };
        number_valid && family_valid && family_shape
    }

    fn rule_selector_is_valid(&self, value: &str) -> bool {
        let Some((_, remainder)) = Self::namespace_and_remainder(value) else {
            return false;
        };
        remainder.is_empty()
            || remainder.bytes().all(|value| value.is_ascii_uppercase())
            || self.rule_code_is_exact(value)
    }
}

impl RulesCodeGrammar {
    fn namespace_and_remainder(value: &str) -> Option<(&'static str, &str)> {
        for namespace in [
            CUSTOM_RULE_NAMESPACE,
            SQL_RULE_PREFIX,
            BUILT_IN_RULE_NAMESPACE,
        ] {
            if let Some(remainder) = value.strip_prefix(namespace) {
                return Some((namespace, remainder));
            }
        }
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub enabled: bool,
}

impl CacheConfig {
    pub(crate) fn enabled_by_default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleException {
    pub rule: String,
    pub path: String,
    pub reason: String,
    /// The joined relation a relation-level exception covers, for Rules that report one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    /// The top-level CTE that contains the covered relation, when the exception is that narrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cte: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct GraphEdgeException {
    pub consumer: String,
    pub dependency: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleIgnore {
    pub rules: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    pub reason: String,
    #[serde(default)]
    pub selectors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectStarAllow {
    pub paths: Vec<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdOverride {
    pub paths: Vec<String>,
    pub thresholds: BTreeMap<String, u32>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SqlTestRulesConfig {
    pub pipeline_directory: String,
}

impl Default for SqlTestRulesConfig {
    fn default() -> Self {
        Self {
            pipeline_directory: "pipelines".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutConfig {
    pub levels: Vec<String>,
    pub domain_roots: Vec<String>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            levels: vec![
                "staging".into(),
                "intermediate/clean".into(),
                "intermediate/enriched".into(),
                "mart".into(),
            ],
            domain_roots: vec![],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    /// Exact codes or family prefixes; `None` runs SQLClean's default SQL Rules and nothing else.
    pub select: Option<Vec<String>>,
    pub ignore: Vec<String>,
    pub thresholds: BTreeMap<String, u32>,
    pub threshold_overrides: Vec<ThresholdOverride>,
    pub rule_options: BTreeMap<String, BTreeMap<String, Value>>,
    pub rule_exceptions: Vec<RuleException>,
    pub graph_edge_exceptions: Vec<GraphEdgeException>,
    pub rule_ignores: Vec<RuleIgnore>,
    pub select_star_allow: Vec<SelectStarAllow>,
    pub domains: Vec<String>,
    pub approved_source_tokens: Vec<String>,
    pub retired_source_tokens: BTreeMap<String, String>,
    pub sql_tests: SqlTestRulesConfig,
    pub layout: LayoutConfig,
    pub cache: CacheConfig,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            select: None,
            ignore: vec![],
            thresholds: BTreeMap::new(),
            threshold_overrides: vec![],
            rule_options: BTreeMap::new(),
            rule_exceptions: vec![],
            graph_edge_exceptions: vec![],
            rule_ignores: vec![],
            select_star_allow: vec![],
            domains: vec![],
            approved_source_tokens: vec![],
            retired_source_tokens: BTreeMap::new(),
            sql_tests: SqlTestRulesConfig::default(),
            layout: LayoutConfig::default(),
            cache: CacheConfig::enabled_by_default(),
        }
    }
}

impl RulesConfig {
    fn rule_option(&self, code: &str, option: &str) -> Option<&Value> {
        self.rule_options
            .get(code)
            .and_then(|options| options.get(option))
    }

    pub(crate) fn rule_option_enabled(&self, code: &str, option: &str) -> bool {
        self.rule_option(code, option)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub(crate) fn rule_option_strings(&self, code: &str, option: &str) -> Vec<String> {
        self.rule_option(code, option)
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reference {
    pub ref_kind: String,
    pub ref_name: String,
    pub ref_package: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type", alias = "data_type")]
    pub data_type: String,
    pub nullable: Option<bool>,
    pub audit_count: u32,
    pub type_proven: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DynamicColumnFamily {
    pub name: String,
    pub pivot_column: String,
    pub value_column: String,
    pub aggregate: String,
    #[serde(rename = "type", alias = "data_type")]
    pub data_type: String,
    pub name_pattern: Option<String>,
    pub type_proven: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeclarationMember {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Declaration {
    pub name: String,
    pub relative_path: String,
    pub members: Vec<DeclarationMember>,
    pub value: Option<Value>,
    pub value_type: Option<String>,
    pub render_as: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Model,
    Test,
    Scenario,
    Function,
    Source,
    Seed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationKind {
    Macro,
    Enum,
    Constant,
    Audit,
    SingularAudit,
    Schema,
    SqlHook,
    PythonHook,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Global,
    Inherited,
    Local,
    Private,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    Runtime,
    Generated,
    DeclarationDependency,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantKind {
    ExpectedModel,
    TestedMacro,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityReason {
    Global,
    InheritedAncestor,
    LocalOwner,
    PrivateOwner,
    ExpectedModel,
    TestedMacro,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InaccessibleReason {
    LocalBoundary,
    SiblingScope,
    DescendantScope,
    UnrelatedScope,
    PrivateOwner,
    UnsupportedResourceKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ScopeDiagnosticCode {
    S001,
    S002,
    S003,
    S004,
    S005,
    S006,
    S007,
    S008,
    S009,
    S010,
    S011,
    S012,
    S013,
    S014,
    S015,
    S016,
    S017,
    S018,
    S019,
    S020,
    S021,
    S022,
    S023,
    S024,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeOwnershipRoot {
    pub path: String,
    pub resource_kind: Option<ResourceKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeResource {
    pub identity: String,
    pub kind: ResourceKind,
    pub name: String,
    pub path: String,
    pub ownership_root: String,
    pub ownership_root_kind: Option<ResourceKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeMacroMetadata {
    pub parameters: Vec<String>,
    pub dependencies: Vec<String>,
    pub source_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeEnumMember {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeEnumMetadata {
    pub members: Vec<ScopeEnumMember>,
    pub scalar_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeConstantMetadata {
    pub logical_type: String,
    pub collection_kind: Option<String>,
    pub item_count: Option<u64>,
    pub nullable: bool,
    pub render_as: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScopeDeclarationMetadata {
    pub r#macro: Option<ScopeMacroMetadata>,
    pub r#enum: Option<ScopeEnumMetadata>,
    pub constant: Option<ScopeConstantMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeDeclaration {
    pub identity: String,
    pub kind: DeclarationKind,
    pub name: String,
    pub owner: Option<String>,
    pub path: String,
    pub line: u64,
    pub column: u64,
    pub scope: ScopeKind,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub role_root: Option<String>,
    #[serde(default)]
    pub bucket_path: Option<String>,
    pub ownership_root: String,
    pub owning_path: Option<String>,
    pub metadata: ScopeDeclarationMetadata,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeUsage {
    pub consumer: String,
    pub declaration: String,
    pub kind: UsageKind,
    pub through: Option<String>,
    pub enum_member: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeGrant {
    pub resource: String,
    pub declaration: String,
    pub through: String,
    pub kind: GrantKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVisibility {
    pub resource: String,
    pub declaration: String,
    pub reason: VisibilityReason,
    pub through: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeInaccessible {
    pub resource: String,
    pub declaration: String,
    pub reason: InaccessibleReason,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeDiagnostic {
    pub code: ScopeDiagnosticCode,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub declaration: Option<String>,
    pub resource: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeCompleteness {
    pub discovery: bool,
    pub static_visibility: bool,
    pub runtime_usage: bool,
    pub relationships: bool,
    pub placement: bool,
    pub promotion_impact: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeIndexFacts {
    #[serde(deserialize_with = "crate::scope_metadata::main::deserialize_scope_schema_version")]
    pub schema_version: u32,
    pub ownership_roots: Vec<ScopeOwnershipRoot>,
    pub resources: Vec<ScopeResource>,
    pub declarations: Vec<ScopeDeclaration>,
    pub usages: Vec<ScopeUsage>,
    pub grants: Vec<ScopeGrant>,
    pub visibility: Vec<ScopeVisibility>,
    pub inaccessible: Vec<ScopeInaccessible>,
    pub diagnostics: Vec<ScopeDiagnostic>,
    pub complete: bool,
    pub completeness: ScopeCompleteness,
}

impl Default for ScopeIndexFacts {
    fn default() -> Self {
        Self {
            schema_version: SCOPE_METADATA_SCHEMA_VERSION,
            ownership_roots: vec![],
            resources: vec![],
            declarations: vec![],
            usages: vec![],
            grants: vec![],
            visibility: vec![],
            inaccessible: vec![],
            diagnostics: vec![],
            complete: false,
            completeness: ScopeCompleteness {
                discovery: false,
                static_visibility: false,
                runtime_usage: false,
                relationships: false,
                placement: false,
                promotion_impact: false,
            },
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Model {
    #[serde(default)]
    pub sql_analysis_disabled: bool,
    pub name: String,
    pub relative_path: String,
    pub query_sql: String,
    pub authored_sql: String,
    /// The lines of the model file above `query_sql`, which locations inside it skip.
    #[serde(default)]
    pub query_line_offset: u64,
    /// The characters before `query_sql`'s first line on its file line, when it shares one.
    #[serde(default)]
    pub query_column_offset: u64,
    /// How `query_sql` maps back to the authored query, when its expansion is known.
    #[serde(default)]
    pub authored_query: Option<AuthoredQueryMap>,
    pub config: BTreeMap<String, Value>,
    pub authored_config_keys: Vec<String>,
    pub logical_schema: Option<String>,
    pub references: Vec<Reference>,
    pub columns: Vec<Column>,
    pub dynamic_columns: Vec<DynamicColumnFamily>,
    pub dynamic_columns_proven: bool,
    pub bare_dynamic_pivot: bool,
    pub enum_columns: Vec<String>,
    pub enum_declarations: Vec<Declaration>,
    pub constant_declarations: Vec<Declaration>,
    /// Each expression SQL macro call in `query_sql`, by byte range.
    #[serde(default)]
    pub typed_calls: Vec<TypedCall>,
    pub declared_audit_count: u32,
    pub targeting_test_count: u32,
    #[serde(skip_deserializing)]
    pub empty_input_only_test_count: u32,
    #[serde(skip)]
    pub payload_digest: Option<String>,
    /// Whether this row's SQL text was dropped, so only a cached result may stand for it.
    #[serde(skip)]
    pub stripped: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SqlTestMode {
    Model,
    Macro,
    Udf,
    TableFn,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectTestResourceKind {
    Macro,
    Udf,
    TableFn,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestedResource {
    pub kind: DirectTestResourceKind,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SqlTestParameterFact {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub nullable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SqlTestParameterValueFact {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SqlTestFact {
    pub source_path: String,
    pub ownership_root: String,
    pub block_index: u64,
    pub name: String,
    pub explicit_name: Option<String>,
    #[serde(default)]
    pub parent_name: Option<String>,
    #[serde(default)]
    pub case_name: Option<String>,
    #[serde(default)]
    pub case_index: Option<u64>,
    #[serde(default)]
    pub case_fingerprint: Option<String>,
    #[serde(default)]
    pub parameter_schema: Vec<SqlTestParameterFact>,
    #[serde(default)]
    pub parameters: Vec<SqlTestParameterValueFact>,
    pub mode: SqlTestMode,
    pub expected_model_names: Vec<String>,
    pub assertion_names: Vec<String>,
    pub assertion_target_model_names: Vec<String>,
    pub target_model_names: Vec<String>,
    pub tested_resources: Vec<TestedResource>,
    #[serde(default)]
    pub authored_ctes: Vec<SqlTestCteFact>,
    #[serde(default)]
    pub expected_ctes: Vec<SqlTestCteFact>,
    #[serde(default)]
    pub assertion_ctes: Vec<SqlTestCteFact>,
    #[serde(default)]
    pub has_macro_mocks: bool,
    #[serde(default)]
    pub has_model_query_overrides: bool,
    #[serde(default, skip_deserializing)]
    pub empty_input_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SqlTestCteFact {
    pub name: String,
    pub sql: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SqlScenarioFact {
    pub source_path: String,
    pub ownership_root: String,
    pub name: String,
    pub description: Option<String>,
    pub expected_model_names: Vec<String>,
    pub assertion_names: Vec<String>,
    pub assertion_target_model_names: Vec<String>,
    pub target_model_names: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRule {
    pub code: String,
    pub family: String,
    pub slug: String,
    pub message: String,
    pub remediation: String,
    #[serde(default)]
    pub enabled_by_default: bool,
    pub implementation_fingerprint: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_line: u64,
    #[serde(default)]
    pub source_column: u64,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub project_wide: bool,
    #[serde(default)]
    pub check_name: String,
    #[serde(default)]
    pub test_case_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unevaluated: bool,
    pub code: String,
    pub path: String,
    pub line: u64,
    pub column: u64,
    pub message: String,
    pub remediation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluateRequest {
    pub version: u32,
    pub project_dir: String,
    pub dialect: String,
    pub config: RulesConfig,
    pub models: Vec<Model>,
    pub sql_tests: Vec<SqlTestFact>,
    pub sql_scenarios: Vec<SqlScenarioFact>,
    pub public_enums: Vec<Declaration>,
    pub public_constants: Vec<Declaration>,
    pub scope_index: ScopeIndexFacts,
    pub initial_findings: Vec<Fault>,
    pub defer_suppressions: bool,
    pub custom_rules: Vec<CustomRule>,
    pub rules_cache_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CustomHostSpec {
    pub program: String,
    pub arguments: Vec<String>,
    pub timeout_millis: u64,
    pub runtime_version: String,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveRulesRequest {
    pub version: u32,
    pub config: RulesConfig,
    pub custom_rules: Vec<CustomRule>,
}

impl Default for EvaluateRequest {
    fn default() -> Self {
        Self {
            version: API_VERSION,
            project_dir: ".".into(),
            dialect: "generic".into(),
            config: RulesConfig::default(),
            models: vec![],
            sql_tests: vec![],
            sql_scenarios: vec![],
            public_enums: vec![],
            public_constants: vec![],
            scope_index: ScopeIndexFacts::default(),
            initial_findings: vec![],
            defer_suppressions: false,
            custom_rules: vec![],
            rules_cache_path: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct EvaluateResponse {
    pub version: u32,
    pub faults: Vec<Fault>,
    pub evaluated_models: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub ruleset_fingerprint: String,
    pub selected_codes: Vec<String>,
    pub built_in_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleGuidance {
    pub good_example: String,
    pub anti_tautology: String,
    pub mutation_check: String,
}

impl RuleGuidance {
    pub(crate) fn remediation(&self, introduction: &str) -> String {
        format!(
            "{introduction}\n\n{}\n\n{}\n\n{}",
            self.good_example, self.anti_tautology, self.mutation_check
        )
    }
}

/// Inputs that can change a rule's findings; only `Model` findings are cached per model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    Model,
    Project,
    ModelAndProject,
    SqlLint,
}

impl RuleScope {
    pub(crate) fn evaluates_models(self) -> bool {
        matches!(self, Self::Model | Self::ModelAndProject)
    }

    pub(crate) fn evaluates_project(self) -> bool {
        matches!(self, Self::Project | Self::ModelAndProject)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleMetadata {
    pub code: String,
    pub family: String,
    pub slug: String,
    pub message: String,
    pub remediation: String,
    pub guidance: Option<RuleGuidance>,
    pub implementation_fingerprint: String,
    pub enabled_by_default: bool,
    pub project_wide: bool,
    pub custom: bool,
    pub scope: RuleScope,
}

impl fensu_policy::policy::types::PolicyRule for RuleMetadata {
    type Applicability = ();

    fn code(&self) -> &str {
        &self.code
    }

    fn enabled_by_default(&self) -> bool {
        self.enabled_by_default
    }

    fn is_applicable(&self, _applicability: &Self::Applicability) -> bool {
        true
    }
}

/// The expansion passes from a model's authored query to `query_sql`, and the query's file place.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AuthoredQueryMap {
    /// Each pass's substitutions in characters, the first pass applied to the authored query.
    pub passes: Vec<Vec<crate::lint_text::models::ExpansionSpan>>,
    /// The lines of the model file above the authored query.
    pub line_offset: u64,
    /// The characters before the authored query on its first file line.
    pub column_offset: u64,
}

/// One compiled model's facts as the rules request builder receives them.
#[derive(Clone, Debug, Default)]
pub struct ModelRow {
    pub name: String,
    pub relative_path: String,
    pub query_sql: String,
    pub authored_sql: String,
    /// The lines of the model file above `query_sql`.
    pub query_line_offset: u64,
    /// The characters before `query_sql`'s first line on its file line.
    pub query_column_offset: u64,
    /// How `query_sql` maps back to the authored query, when its expansion is known.
    pub authored_query: Option<AuthoredQueryMap>,
    pub config: BTreeMap<String, Value>,
    pub authored_config_keys: Vec<String>,
    pub logical_schema: Option<String>,
    pub references: Vec<Reference>,
    pub schema: Option<SchemaRow>,
    /// Each inferred output column's name and type, in output order.
    pub inferred_columns: Vec<(String, Option<String>)>,
    pub dynamic_proof: Option<DynamicProofRow>,
    pub enum_columns: Vec<String>,
    pub enum_declarations: Vec<Declaration>,
    pub constant_declarations: Vec<Declaration>,
    /// Each expression SQL macro call in `query_sql`, by byte range.
    pub typed_calls: Vec<TypedCall>,
    /// Whether analysis was skipped because the dialect's native parser cannot read the model.
    pub sql_analysis_skipped: bool,
}

/// A model's declared schema entry.
#[derive(Clone, Debug, Default)]
pub struct SchemaRow {
    /// Audits declared on the entry itself, not on its columns.
    pub audit_count: u32,
    pub columns: Vec<SchemaColumnRow>,
    pub dynamic_columns: Vec<DynamicFamilyRow>,
}

#[derive(Clone, Debug, Default)]
pub struct SchemaColumnRow {
    pub name: String,
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    pub audit_count: u32,
}

/// One declared dynamic-column family; `key` is its name casefolded.
#[derive(Clone, Debug, Default)]
pub struct DynamicFamilyRow {
    pub key: String,
    pub name: String,
    pub pivot_column: String,
    pub value_column: String,
    pub aggregate: String,
    pub data_type: String,
    pub name_pattern: Option<String>,
}

/// A model's dynamic-column contract proof; each family is keyed by its casefolded name.
#[derive(Clone, Debug, Default)]
pub struct DynamicProofRow {
    pub output_proven: bool,
    pub bare_dynamic_pivot: bool,
    pub families: Vec<(String, Option<String>)>,
}

/// Project facts the model builder needs beyond each model's own row.
#[derive(Clone, Copy, Debug)]
pub struct ModelRowsContext<'a> {
    pub sql_analysis_enabled: bool,
    pub include_type_proof: bool,
    pub dialect: &'a str,
    /// The target model of every compiled audit attached to one.
    pub attached_audit_targets: &'a [String],
    pub sql_tests: &'a [SqlTestFact],
}

/// A natively built rules request and the response memo it may be answered from.
#[derive(Debug)]
pub struct RowsRequest {
    pub request: EvaluateRequest,
    /// The compiled-code identity and memo path, when the rules cache is enabled.
    pub memo: Option<(String, std::path::PathBuf)>,
    /// Keep the request, its models without SQL text, for the project image.
    pub retain: bool,
}

/// A request kept for the project image: its model-free JSON and its rows without SQL text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetainedRows {
    pub request_json: Vec<u8>,
    /// Each model's row without its SQL text, in the native record format, in model order.
    pub rows: Vec<u8>,
    /// Each model's payload digest, which keys its cached findings, in model order.
    pub digests: Vec<String>,
}

/// A built-in rules evaluation running speculatively on its own thread; dropping it cancels it.
#[derive(Debug)]
pub struct SpeculativeRows {
    pub(crate) key: std::sync::Mutex<std::sync::mpsc::Receiver<Option<String>>>,
    pub(crate) thread: Option<
        std::thread::JoinHandle<
            Result<Option<crate::engine::_helpers::memo::SpeculativeOutcome>, String>,
        >,
    >,
    pub(crate) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// How a request was answered beside a speculative evaluation.
#[derive(Debug)]
pub enum SpeculativeAnswer {
    /// The request's evaluation, and whether the speculation evaluated exactly this request.
    Answered {
        evaluation: Result<RowsEvaluation, RowsError>,
        used: bool,
    },
    /// `stop` was set while waiting; nothing was evaluated or written.
    Stopped,
}

/// A rows request's decoded response.
#[derive(Debug, Deserialize)]
pub struct RowsEvaluation {
    pub faults: Vec<Fault>,
    pub selected_codes: Vec<String>,
    pub evaluated_models: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub built_in_ms: u64,
    #[serde(skip)]
    pub reused: bool,
    #[serde(skip)]
    pub retained: Option<RetainedRows>,
}
