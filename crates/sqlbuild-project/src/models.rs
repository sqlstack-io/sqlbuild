//! The plain records the compiled project keeps for its resources.

use std::sync::Arc;

use sqlbuild_sqltext::compiler::models::CharSpan;

use sqlbuild_analysis::assembly::analysis_session::models::{ContractProof, LineageFacts};
use sqlbuild_analysis::binding_locations::models::LocatedDiagnostic;
use sqlbuild_analysis::compiled_project::models::CompiledProjectFacts;
use sqlbuild_analysis::sql_macros::models::{SqlMacroCheck, TypedCall};

use sqlbuild_core::json::models::JsonValue;

use crate::types::{InferredRow, NamedTypeRow, ObjectKeyRow, ProjectHost, SourceNamespace};

/// One reference a resource's SQL makes: kind, name, package and its call's arguments and partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceRow {
    pub kind: String,
    pub name: String,
    pub package: Option<String>,
    pub call_argument_count: Option<usize>,
    /// Whether a table function call is followed by `OVER (PARTITION BY ...)`.
    pub call_partitioned: bool,
}

/// A compiled relation location's parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationFact {
    pub database: Option<String>,
    pub schema: Option<String>,
    pub name: String,
    pub qualified_name: Option<String>,
    pub logical_schema: Option<String>,
    pub logical_database: Option<String>,
}

/// A model's name, compiled query, authored file, and whether it calls a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMetadataTexts {
    pub name: String,
    pub query_sql: String,
    pub authored_sql: String,
    pub calls_functions: bool,
}

/// The expansion passes of a model's query, from authored text to expanded SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpansionRecord {
    /// The authored query below the header.
    pub authored_sql: Arc<str>,
    /// The declaration- and macro-expanded SQL before canonicalisation.
    pub expanded_sql: Arc<str>,
    pub interpolation_spans: Vec<CharSpan>,
    pub declaration_spans: Vec<CharSpan>,
    pub macro_spans: Vec<CharSpan>,
    /// Whether interpolation left the authored query unchanged.
    pub authored: bool,
}

/// A model's attached header schema; the host keeps its columns, audits and dynamic columns.
#[derive(Debug)]
pub struct SchemaRecord<H: ProjectHost> {
    pub description: Option<String>,
    pub type_enforcement: bool,
    pub model_schema: Option<String>,
    pub host: H::Schema,
}

/// What the model loop finished for one model beyond its facts; the host keeps its user values.
#[derive(Debug)]
pub struct ModelInputRecord<H: ProjectHost> {
    /// The model's name, its file stem.
    pub name: String,
    /// Immutable text shared with discovery and expansions.
    pub query_sql: Arc<str>,
    pub macro_source_sql: Arc<str>,
    pub references: Vec<ReferenceRow>,
    pub schema: Option<SchemaRecord<H>>,
    pub sql_validation_enabled: bool,
    pub macro_deps: Vec<String>,
    pub expansion: ExpansionRecord,
    /// What the analysed SQL must show for the model's SQL macro call arguments to fit.
    pub sql_macro_checks: Vec<SqlMacroCheck>,
    /// Each expression macro call in `query_sql`, by byte range.
    pub typed_calls: Vec<TypedCall>,
    /// What assembly and the semantic checks finished, once the model is assembled.
    pub compiled: Option<CompiledRecord<H>>,
    pub host: H::ModelInput,
}

/// What one model's analysis left for its compiled model.
#[derive(Debug)]
pub struct AnalysisRecord<H: ProjectHost> {
    pub inferred: Option<Vec<InferredRow>>,
    pub lineage: Option<LineageFacts>,
    pub has_star: bool,
    pub star_resolved: bool,
    pub binding: Vec<LocatedDiagnostic>,
    pub binding_validated: bool,
    pub proof: Option<ContractProof>,
    pub host: H::Analysis,
}

/// What assembly and the semantic checks finished for one compiled model.
#[derive(Debug)]
pub struct CompiledRecord<H: ProjectHost> {
    /// `(resource type, name)` of each dependency.
    pub deps: Vec<(String, String)>,
    pub destination: DestinationFact,
    /// Columns, lineage, binding diagnostics and dynamic proof kept from the native session.
    pub analysis: AnalysisRecord<H>,
    pub macro_deps: Vec<String>,
    pub host: H::CompiledModel,
}

/// What the source loop attached for one source: its expanded metadata.
#[derive(Debug)]
pub struct AttachedSource<H: ProjectHost> {
    pub name: String,
    pub contract: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub expression: Option<String>,
    pub description: Option<String>,
    pub host: H::AttachedSource,
}

/// One attached source held by the compiled project.
#[derive(Debug)]
pub struct SourceRecord<H: ProjectHost> {
    pub attached: AttachedSource<H>,
    /// The namespace native project assembly resolved, once it ran.
    pub namespace: Option<SourceNamespace>,
    pub host: H::Source,
}

/// A Python function's runtime, entry point, packages and, for a table function, its frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonRuntime {
    pub runtime_version: Option<String>,
    pub entry_point: Option<String>,
    pub packages: Vec<String>,
    /// A table function's partition frame library, `"polars"` or `"pandas"`.
    pub dataframe: Option<String>,
}

/// What the function loop attached for one SQL or Python function.
#[derive(Debug)]
pub struct AttachedFunction<H: ProjectHost> {
    /// The file's relative path, as messages name it.
    pub relative_path: String,
    pub name: String,
    pub arguments: Vec<NamedTypeRow>,
    pub returns: String,
    pub return_columns: Vec<NamedTypeRow>,
    pub body: String,
    pub references: Vec<ReferenceRow>,
    pub destination: DestinationFact,
    pub fingerprint_destination: DestinationFact,
    pub python: Option<PythonRuntime>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub host: H::AttachedFunction,
}

/// One attached function held by the compiled project.
#[derive(Debug)]
pub struct FunctionRecord<H: ProjectHost> {
    pub attached: AttachedFunction<H>,
    /// The dependency keys native project assembly resolved, once it ran.
    pub deps: Option<Vec<ObjectKeyRow>>,
    pub host: H::Function,
}

/// The target an attached audit checks: its kind, name and column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedTarget {
    pub kind: String,
    pub name: String,
    pub column: Option<String>,
}

/// What the audit loop attached for one singular audit block or attached generic audit.
#[derive(Debug)]
pub struct AttachedAudit<H: ProjectHost> {
    /// The block's name, else the file's stem.
    pub definition_name: String,
    pub sql_body: String,
    pub measurement: bool,
    pub evidence_sql: Option<String>,
    pub references: Vec<ReferenceRow>,
    pub target: Option<AttachedTarget>,
    pub severity: &'static str,
    pub run_scope: String,
    pub always_run: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    /// The authored mapping of JSON values reported with results, or `None`.
    pub meta: Option<JsonValue>,
    pub host: H::AttachedAudit,
}

/// One attached audit held by the compiled project.
#[derive(Debug)]
pub struct AuditRecord<H: ProjectHost> {
    pub attached: AttachedAudit<H>,
    /// The scope dependency keys native project assembly resolved, once it ran.
    pub scope_deps: Option<Vec<ObjectKeyRow>>,
    pub host: H::Audit,
}

/// One seed CSV paired with its declaration; the host keeps the discovered objects.
#[derive(Debug)]
pub struct SeedRecord<H: ProjectHost> {
    pub name: String,
    /// The destination native project assembly resolved, once it ran.
    pub destination: Option<DestinationFact>,
    pub host: H::Seed,
}

/// One compile's project: the facts producers record and each resource's records.
#[derive(Debug)]
pub struct CompiledProject<H: ProjectHost> {
    pub facts: CompiledProjectFacts,
    /// What the model loop finished for each model beyond its facts, in loop order.
    pub inputs: Vec<ModelInputRecord<H>>,
    /// Lineage graphs its project views built.
    pub graph_builds: usize,
    /// Each attached seed, in attachment order.
    pub seeds: Vec<SeedRecord<H>>,
    /// Each attached source, in attachment order.
    pub sources: Vec<SourceRecord<H>>,
    /// Each attached SQL, then Python, function in attachment order.
    pub functions: Vec<FunctionRecord<H>>,
    /// Each singular, then model-, source- and seed-attached, audit in attachment order.
    pub audits: Vec<AuditRecord<H>>,
}
