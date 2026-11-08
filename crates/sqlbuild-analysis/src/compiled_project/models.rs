//! Facts a compile's producers retain about each resource, in production order.

use crate::sql_macros::models::TypedCall;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde_json::Value;
use sqlbuild_sqltext::compiler::models::CharSpan;

/// One compile's project facts; models keep the order the compile-input stage produced them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompiledProjectFacts {
    pub(crate) models: Vec<CompiledModelFacts>,
    pub(crate) model_index: HashMap<String, usize>,
}

/// What the compile-input stage produced for one model, plus its analysis once transferred.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompiledModelFacts {
    pub name: String,
    pub relative_path: String,
    /// SQL after variable, declaration and macro expansion.
    pub query_sql: Arc<str>,
    /// The authored query below the `MODEL (...)` header.
    pub authored_query_sql: Arc<str>,
    /// The whole authored file.
    pub authored_sql: Arc<str>,
    pub config: ModelConfigFacts,
    pub references: Vec<ModelReferenceFact>,
    pub schema: Option<ModelSchemaFact>,
    pub enum_columns: Vec<String>,
    pub enum_declarations: Vec<DeclarationFact>,
    pub constant_declarations: Vec<DeclarationFact>,
    /// The typed analysis result; `None` until the assembly stage transfers it.
    pub analysis: Option<ModelAnalysisFact>,
    /// Each expression SQL macro call in `query_sql`, by byte range.
    pub typed_calls: Vec<TypedCall>,
    /// The expansion passes from `authored_query_sql` to `query_sql`; empty when unknown.
    pub expansion_passes: Vec<Vec<CharSpan>>,
    /// Whether analysis was skipped because the dialect's native parser cannot read the model.
    pub sql_analysis_skipped: bool,
}

/// A model's effective config values with sorted keys, as the rules request has always read them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelConfigFacts {
    pub values: BTreeMap<String, Value>,
    /// Why the values could not be encoded, raised only when a consumer reads them.
    pub encoding_error: Option<String>,
    pub header_keys: Vec<String>,
    pub layer_schema: Option<String>,
    /// The `contract` value when it is text.
    pub contract: Option<String>,
    /// The `materialized` value when it is text; `table` when it is absent.
    pub materialized: Option<String>,
    /// The `incremental_mode` value when it is text.
    pub incremental_mode: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelReferenceFact {
    pub kind: String,
    pub name: String,
    pub package: Option<String>,
}

/// A model's declared schema entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelSchemaFact {
    /// Audits declared on the entry itself, not on its columns.
    pub audit_count: u32,
    pub columns: Vec<SchemaColumnFact>,
    pub dynamic_columns: Vec<DynamicFamilyFact>,
    /// Whether a column or family declares a type, so the contract checks types.
    pub type_enforcement: bool,
    /// Whether the entry comes from a reusable named schema.
    pub named_schema: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchemaColumnFact {
    pub name: String,
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    /// Whether the column declares a description.
    pub described: bool,
    pub migrate_from: Option<String>,
    /// Each column audit's definition name, in declaration order.
    pub audit_definitions: Vec<String>,
    /// The file of a column declared outside the model file, in a named schema.
    pub location_path: Option<String>,
}

/// One declared dynamic-column family; `key` is its name casefolded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicFamilyFact {
    pub key: String,
    pub name: String,
    pub pivot_column: String,
    pub value_column: String,
    pub aggregate: String,
    pub data_type: String,
    pub name_pattern: Option<String>,
}

/// An enum or constant declaration a model can see.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclarationFact {
    pub name: String,
    pub relative_path: String,
    pub members: Vec<(String, Value)>,
    pub value: Option<Value>,
    pub value_type: Option<String>,
    pub render_as: Option<String>,
}

/// A model's typed analysis outcome: inferred output columns and its dynamic pivot proof.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelAnalysisFact {
    /// Each inferred output column's name and type, in output order.
    pub inferred_columns: Vec<(String, Option<String>)>,
    pub dynamic_proof: Option<DynamicProofFact>,
    /// Whether analysis inferred output columns at all.
    pub inferred: bool,
    /// Whether each inferred output column may be null, in output order.
    pub nullable_columns: Vec<bool>,
    pub fast_lineage_has_star: bool,
    pub unchecked_output_columns: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicProofFact {
    pub output_proven: bool,
    pub bare_dynamic_pivot: bool,
    /// `(family key, inferred type)` per declared family.
    pub families: Vec<(String, Option<String>)>,
    pub failure_reason: Option<String>,
    /// `(family name, inferred type)` per declared family, as authored.
    pub named_families: Vec<(String, Option<String>)>,
}

/// The project-wide switches the model loop reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelLoopSettings {
    pub sql_analysis: bool,
    pub no_sql_validation: bool,
    pub defer_sql_validation: bool,
    pub require_sql_analysis: bool,
}

/// The per-model `sql_analysis` / legacy `sql_validation` override.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalysisOverride {
    #[default]
    Inherit,
    Enabled,
    Disabled,
    /// Both keys are set to different values.
    Conflict,
}

/// Whether a model's SQL is analysed, after the project switches and its override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisGate {
    Enabled,
    Disabled,
    /// The override keys disagree, which the attachment reports as P003.
    Conflict,
}

/// A known declaration the consumer cannot see, as its scope record describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InaccessibleDeclaration {
    pub path: String,
    pub line: u64,
    pub column: u64,
    pub scope: String,
    pub owner: String,
    /// `(kind, name)` of the owning resource, when the declaration is resource-owned.
    pub owner_identity: Option<(String, String)>,
}

/// One scanned declaration reference's character range and the text that replaces it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclarationSplice {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// A substituted region: its source and output character ranges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplicedSpan {
    pub source_start: usize,
    pub source_end: usize,
    pub output_start: usize,
    pub output_end: usize,
}

/// SQL with declaration references replaced, and where each replacement landed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SplicedSql {
    pub sql: String,
    pub spans: Vec<SplicedSpan>,
}

/// A natively finished model's facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlainModel {
    pub name: String,
    pub relative_path: String,
    pub query_sql: Arc<str>,
    pub authored_query_sql: Arc<str>,
    pub authored_sql: Arc<str>,
    pub config: ModelConfigFacts,
    /// The attached schema entry; present when the header declares description, columns or audits.
    pub schema: Option<ModelSchemaFact>,
    pub references: Vec<ModelReferenceFact>,
    /// Header columns typed as an enum, in column order.
    pub enum_columns: Vec<String>,
    pub enum_declarations: Vec<DeclarationFact>,
    pub constant_declarations: Vec<DeclarationFact>,
    /// Each expression SQL macro call in `query_sql`, by byte range.
    pub typed_calls: Vec<TypedCall>,
    /// The expansion passes from `authored_query_sql` to `query_sql`; empty when unknown.
    pub expansion_passes: Vec<Vec<CharSpan>>,
    /// Whether analysis was skipped because the dialect's native parser cannot read the model.
    pub sql_analysis_skipped: bool,
}

/// A one-based line and column, counted in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

/// The authored model a reference-scan fault is located in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanFaultOrigin<'a> {
    /// The POSIX relative path the error names.
    pub relative_path: &'a str,
    pub contents: &'a str,
    pub query_sql: &'a str,
    /// The declaration and macro expansion passes, or `None` when the SQL was not authored text.
    pub passes: Option<Vec<Vec<SplicedSpan>>>,
}
