pub const API_VERSION: u32 = 1;
/// A content hash of every native crate, including the release version in their manifests.
pub const NATIVE_BUILD_IDENTITY: &str = env!("SQLBUILD_NATIVE_SOURCE_HASH");
pub(crate) const SCOPE_METADATA_SCHEMA_VERSION: u32 = 3;
pub(crate) const BUILT_IN_RULE_NAMESPACE: &str = "SQBR";
pub(crate) const PIPELINE_SUBJECT: &str = "pipeline";
pub(crate) const GENERIC_TEST_NAME: &str = "test";
pub(crate) const GENERIC_WORKS_NAME: &str = "works";
pub(crate) const GENERIC_BASIC_NAME: &str = "basic";
pub(crate) const GENERIC_SCENARIO_NAME: &str = "scenario";
pub(crate) const GENERIC_CASE_NAME: &str = "case";
pub(crate) const CUSTOM_RULE_NAMESPACE: &str = "XSQBR";
pub(crate) const REFERENCE_KIND: &str = "ref";
pub(crate) const MODEL_SCHEMA_CONFIG_KEY: &str = "schema";
pub(crate) const STAGING_LAYER_DIRECTORY: &str = "staging";
pub(crate) const INTERMEDIATE_LAYER_DIRECTORY: &str = "intermediate";
pub(crate) const MART_LAYER_DIRECTORY: &str = "mart";
pub(crate) const SOURCE_REFERENCE_KIND: &str = "source";
pub(crate) const VIEW_MATERIALIZATION: &str = "view";
pub(crate) const ENFORCED_CONTRACT: &str = "enforced";
pub(crate) const SELECT_STAR_MODEL_RULE_CODE: &str = "SQBRMODEL102";
pub(crate) const BOOLEAN_TYPE: &str = "BOOLEAN";
pub(crate) const DATE_TYPE: &str = "DATE";
pub(crate) const DECLARATION_DOMAIN_COMPONENTS: usize = 3;
pub(crate) const RULE_CODE_NUMBER_LENGTH: usize = 3;
pub(crate) const SETTING_SNIPPET_INDENT: &str = "            ";
pub(crate) const NATIVE_ROWS_MEMO_VERSION: &str = "native-rules-rows-response-v2";
/// The namespace of the SQL Rules SQLClean owns, whose codes are `SQLCLEAN` and three digits.
pub(crate) const SQL_RULE_PREFIX: &str = "SQLCLEAN";
/// Threshold key setting the fewest test cases a custom rule must ship.
pub const MIN_CUSTOM_RULE_TEST_CASES_KEY: &str = "min_custom_rule_test_cases";
/// Rules whose findings name a joined relation, so an exception may cover just that relation.
pub(crate) const RELATION_EXCEPTION_RULES: [&str; 1] = ["SQLCLEAN038"];
/// The layers the `<domain>__<layer>__<entity>` model name grammar accepts.
pub(crate) const MODEL_NAME_LAYERS: [&str; 7] = [
    "stg",
    "stg_v",
    "int_clean",
    "int_v",
    "int_enriched",
    "mart",
    "mart_v",
];
/// The removed exception switch, which used to allow or forbid every escape hatch at once.
pub(crate) const REMOVED_ALLOW_EXCEPTIONS: &str = "allow_exceptions";
/// Published models must not read published models of their own domain.
pub(crate) const PUBLISHED_READ_CODE: &str = "SQBRGRAPH103";
/// Each domain's published layer uses one model name shape.
pub(crate) const PUBLISHED_SHAPE_CODE: &str = "SQBRPROJECT107";
/// The option naming the published layers.
pub(crate) const PUBLISHED_LAYERS_OPTION: &str = "published_layers";
/// The option listing qualifiers that do not count towards a domain's name shape.
pub(crate) const ALLOWED_QUALIFIERS_OPTION: &str = "allowed_qualifiers";
/// The `allowed_qualifiers` entry key whose globs match published models by file path.
pub(crate) const ALLOWED_PATHS_KEY: &str = "paths";
/// The published layers when a project does not configure them.
pub(crate) const DEFAULT_PUBLISHED_LAYERS: [&str; 2] = ["mart", "mart_v"];
