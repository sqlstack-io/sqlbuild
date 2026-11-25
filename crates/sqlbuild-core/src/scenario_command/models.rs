//! Plain data of scenario command inputs, results and runtime artifacts.

use std::path::PathBuf;

/// The effective scenario run namespace and the configuration source it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioNamespace {
    pub value: Option<String>,
    pub source: String,
}

/// Every place a scenario run namespace can be set, from highest precedence to lowest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamespaceCandidates {
    pub cli: Option<String>,
    pub env: Option<String>,
    pub local_config: Option<String>,
    pub project_config: Option<String>,
}

/// The `sqb scenario test` flags validated before the project is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScenarioTestFlags {
    /// The run replays every scenario locally (`--run-on local`).
    pub local: bool,
    /// The run sends every scenario to the warehouse (`--run-on warehouse`).
    pub warehouse: bool,
    pub retain: bool,
    pub sync_snapshots: bool,
    pub refresh: bool,
}

/// Which scenario command needs SQL analysis, as its error messages name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlAnalysisCommand {
    LocalTest,
    Capture,
}

/// A coded result error: its message with optional code and help.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodedText {
    pub code: Option<String>,
    pub message: String,
    pub help: Option<String>,
}

/// One expected-relation comparison of a scenario result; counts are `None` if it did not run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpectedRow {
    pub model_name: String,
    pub failed: bool,
    pub actual_row_count: Option<i64>,
    pub expected_row_count: Option<i64>,
    pub mismatched_row_count: Option<i64>,
    pub missing_row_count: Option<i64>,
    pub error: Option<CodedText>,
}

/// One zero-row assertion of a scenario result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssertionRow {
    pub name: String,
    pub failed: bool,
    /// `None` when the assertion query did not run.
    pub failing_row_count: Option<i64>,
    /// `str()` of the first failing sample row, when there is one.
    pub first_sample: Option<String>,
    pub error: Option<CodedText>,
}

/// A retained scenario relation: its artifact kind, logical name and physical relation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetainedRelation {
    pub kind: String,
    pub logical_name: String,
    pub physical_name: String,
}

/// One finished scenario run as its result row shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScenarioResultRow {
    pub name: String,
    /// `PASS`/`FAIL`, or the local replay status.
    pub status: String,
    /// The run replayed snapshots locally.
    pub local: bool,
    pub duration_ms: Option<f64>,
    pub error: Option<CodedText>,
    pub retained: bool,
    /// The retained relation map, when the run has one.
    pub retained_relations: Option<Vec<RetainedRelation>>,
    pub local_duckdb_path: Option<String>,
    pub expected: Vec<ExpectedRow>,
    pub assertions: Vec<AssertionRow>,
}

/// One captured scenario input relation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureRelationRow {
    pub kind: String,
    pub logical_name: String,
    pub passed: bool,
    pub row_count: i64,
    pub byte_count: i64,
}

/// The scenario-owned inputs a retained capture kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetainedInputs {
    pub fixtures: Vec<RetainedRelation>,
    pub seeds: Vec<String>,
}

/// One finished scenario snapshot capture as its result row shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureRow {
    pub name: String,
    pub passed: bool,
    pub error: Option<CodedText>,
    pub retained: bool,
    /// The captured relations, when the capture wrote a snapshot result.
    pub relations: Option<Vec<CaptureRelationRow>>,
    /// The snapshot manifest path shown to the user, when there is one.
    pub snapshot_path: Option<String>,
    /// The retained inputs, when the capture was retained and planned.
    pub retained_inputs: Option<RetainedInputs>,
}

/// Snapshot limit overrides from the command line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotLimitInputs {
    pub max_rows_per_relation: Option<i64>,
    pub max_total_rows: Option<i64>,
    pub max_bytes_per_relation: Option<i64>,
    pub max_total_bytes: Option<i64>,
    pub force: bool,
}

/// Snapshot limits configured under `[scenario.snapshot_limits]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotLimitConfig {
    pub max_rows_per_relation: Option<i64>,
    pub max_total_rows: Option<i64>,
    pub max_bytes_per_relation: Option<i64>,
    pub max_total_bytes: Option<i64>,
}

/// The effective snapshot capture limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_rows_per_relation: i64,
    pub max_total_rows: i64,
    pub max_bytes_per_relation: i64,
    pub max_total_bytes: i64,
    pub force: bool,
}

/// One compiled scenario as selectors see it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScenarioFile {
    pub name: String,
    pub file_path: PathBuf,
    /// The scenario file path relative to the project.
    pub relative_path: PathBuf,
}

/// Scenario selectors and exclusions with the project they are relative to.
#[derive(Clone, Copy, Debug)]
pub struct ScenarioSelection<'a> {
    pub selectors: &'a [String],
    pub exclude: &'a [String],
    pub project_dir: &'a std::path::Path,
}

/// One runtime SQL artifact: its path under the scenario run directory and its SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeArtifact {
    pub relative_path: PathBuf,
    pub sql: String,
}

/// SQL statements a fixture executed, with its artifact kind and logical name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FixtureStatements {
    pub kind: String,
    pub logical_name: String,
    pub statements: Vec<String>,
}

/// SQL statements a seed or model executed, with its path or name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedStatements {
    pub name: String,
    pub statements: Vec<String>,
}

/// A planned model whose executed SQL is written under `models/`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelArtifactEntry {
    pub name: String,
    pub relative_path: PathBuf,
}

/// The planned expected comparisons and assertions of a scenario.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpectationSql {
    /// Each expected model with its rendered comparison SQL.
    pub expected: Vec<(String, String)>,
    /// Each assertion with its authored SQL.
    pub assertions: Vec<(String, String)>,
}

/// Everything a warehouse scenario run writes under `target/run/scenarios/<name>/`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WarehouseRuntimeInputs {
    pub prepare_cleanup: Option<Vec<String>>,
    pub fixtures: Vec<FixtureStatements>,
    pub seeds: Vec<NamedStatements>,
    pub models: Vec<NamedStatements>,
    pub model_entries: Vec<ModelArtifactEntry>,
    pub expectations: ExpectationSql,
    pub final_cleanup: Option<Vec<String>>,
}

/// A snapshot relation a local replay loaded into DuckDB.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedRelation {
    pub kind: String,
    pub logical_name: String,
    /// The snapshot file, as a POSIX path.
    pub file_path: String,
    pub table_name: String,
    pub row_count: i64,
}

/// A function a local replay created, with its rendered statements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionStatements {
    pub relative_path: PathBuf,
    pub language: String,
    pub statements: Vec<String>,
}

/// Everything a local scenario replay writes under `target/run/scenarios/<name>/local/`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalRuntimeInputs {
    pub loaded_relations: Vec<LoadedRelation>,
    pub functions: Vec<FunctionStatements>,
    /// Each planned model's relative path and resolved SQL.
    pub models: Vec<(PathBuf, String)>,
    pub expectations: ExpectationSql,
}
