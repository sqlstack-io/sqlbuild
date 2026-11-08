//! Plain-data request, deferrals and outcomes of the native model analysis session.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use rayon::ThreadPool;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;
use sqlquack_semantics::lineage::models::SchemaColumn;
use sqlquack_semantics::lineage::types::LineageSchema;

use crate::assembly::analysis_session::_helpers::catalog_state::SessionCatalog;
use crate::assembly::analysis_session::_helpers::mappings::ShapeTable;
use crate::assembly::analysis_session::types::{NullabilityRuleFn, OutputSources, Pairs, Shapes};
use crate::semantic_validation::models::DiagnosticRow;
use crate::sql_macros::models::{SqlMacroCheck, SqlMacroFinding};

/// One `ref`, `source`, `seed`, `table_fn` or `udf` call a model's SQL makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReference {
    /// Python's `_analysis_reference_name`.
    pub analysis_name: String,
    /// Whether the call is a `ref`, so names a model the analysis orders by.
    pub model_ref: bool,
}

/// One analysed model, with the facts Python derives from its compile input.
#[derive(Debug, Clone, Default)]
pub struct ModelRequest {
    pub name: String,
    /// Cursor-intrinsic analysis SQL.
    pub query_sql: String,
    pub placeholders: Pairs,
    pub references: Vec<ModelReference>,
    /// Python's `lineage_reference_map`: `(analysis name, resource type, resource name)`.
    pub lineage_references: Vec<(String, String, String)>,
    /// Sorted `binding_relation_names`.
    pub required_names: Vec<String>,
    pub recover_cte_facts: bool,
    /// Whether the SQL names `UNION`, `INTERSECT` or `EXCEPT`, as Python's search decides it.
    pub has_set_operation: bool,
    /// The snapshot validity columns a published shape appends.
    pub snapshot_columns: Option<(String, String)>,
    /// The SQL Python's dynamic pivot proof parses: placeholders replaced by their defaults.
    pub pivot_sql: String,
    pub dynamic_families: Vec<DynamicFamily>,
    /// What the model's analysed SQL must show for its SQL macro calls' arguments to fit.
    pub sql_macro_checks: Vec<SqlMacroCheck>,
}

/// The reference-derived fields of a `ModelRequest`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelReferenceFacts {
    pub references: Vec<ModelReference>,
    pub lineage_references: Vec<(String, String, String)>,
    pub required_names: Vec<String>,
}

/// One declared family of columns a runtime-dynamic pivot generates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicFamily {
    pub name: String,
    pub pivot_column: String,
    pub value_column: String,
    pub aggregate: String,
    pub data_type: String,
    pub name_pattern: Option<String>,
}

/// Python's `DynamicColumnContractProof`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractProof {
    pub output_proven: bool,
    pub fixed_columns: Vec<ColumnFact>,
    /// Each family's name and inferred type.
    pub families: Vec<(String, Option<String>)>,
    pub input_relations: Vec<String>,
    pub failure_reason: Option<String>,
    pub bare_dynamic_pivot: bool,
}

/// The relation facts every dynamic pivot proof reads: Python's tables before analysis.
#[derive(Debug, Clone, Default)]
pub struct PivotTables {
    pub dialect: String,
    pub column_types: Shapes,
    pub authoritative_types: Shapes,
    pub column_nullability: Shapes,
    pub families_by_table: Vec<(String, Vec<DynamicFamily>)>,
}

/// One model's pivot SQL and declared families.
#[derive(Debug, Clone, Default)]
pub struct PivotModel {
    pub sql: String,
    pub families: Vec<DynamicFamily>,
}

/// Dynamic pivot proofs for models no session analysed.
#[derive(Debug, Clone, Default)]
pub struct PivotBatchRequest {
    pub tables: PivotTables,
    pub models: Vec<PivotModel>,
}

/// A finished session's pivot tables, analysis pool and model facts, kept for later stages.
#[derive(Debug)]
pub struct FinishedSession {
    pub(crate) tables: PivotTables,
    pub(crate) pool: Result<Arc<ThreadPool>, String>,
    pub(crate) models: HashMap<String, SessionModelFacts>,
    /// Each model's dynamic pivot proof from the session's own relation facts, by model name.
    pub(crate) dynamic_proofs: HashMap<String, ContractProof>,
    /// The relation facts the session ended with, when the request asked to keep them.
    pub(crate) relations: Option<RelationTables>,
    /// Each requested model's name, in request order, as the session's outcomes are.
    pub(crate) model_names: Vec<String>,
    /// The adapter's declared functions, which the native analysis knew.
    pub(crate) declared_functions: Vec<String>,
}

/// The session's typed result for one compiled model, borrowed from the finished session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionModelAnalysis<'a> {
    /// Whether the session's analysis of the model succeeded; false also for unanalysed names.
    pub succeeded: bool,
    /// A proven pivot's fixed columns, else a successful analysis's; None where later decided.
    pub columns: Option<&'a [ColumnFact]>,
    /// The session's dynamic pivot proof, for a model declaring dynamic families.
    pub dynamic_proof: Option<&'a ContractProof>,
}

impl FinishedSession {
    /// The relation facts the session ended with, when its request asked to keep them.
    #[must_use]
    pub fn relation_tables(&self) -> Option<&RelationTables> {
        self.relations.as_ref()
    }

    /// The session's typed columns and dynamic proof for model `name`, without re-analysis.
    #[must_use]
    pub fn model_analysis(&self, name: &str) -> SessionModelAnalysis<'_> {
        let dynamic_proof: Option<&ContractProof> = self.dynamic_proofs.get(name);
        let proven: Option<&[ColumnFact]> = dynamic_proof
            .filter(|proof| proof.output_proven)
            .map(|proof| proof.fixed_columns.as_slice());
        let facts: Option<&SessionModelFacts> = self.models.get(name);
        let analysed: Option<&[ColumnFact]> = facts.and_then(|facts| facts.columns.as_deref());
        SessionModelAnalysis {
            succeeded: facts.is_some(),
            columns: proven.or(analysed),
            dynamic_proof,
        }
    }
}

/// A compiled model's typed output columns and lineage from a native session analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionModelFacts {
    pub columns: Option<Vec<ColumnFact>>,
    pub lineage: OutputSources,
}

/// A model's dynamic pivot proof: none declared, or proven natively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PivotOutcome {
    Absent,
    Proof(ContractProof),
}

/// Everything one compile's model analysis reads.
#[derive(Debug, Clone, Default)]
pub struct SessionRequest {
    /// The analysis dialect, `generic` when the adapter names none.
    pub dialect: String,
    /// Whether published shapes keep authored quoting, Python's `inferred_binding_shape` test.
    pub case_sensitive_shapes: bool,
    pub function_return_types: Pairs,
    /// Adapter nullability rules as `(function name, rule id)`.
    pub nullability_rules: Option<Pairs>,
    /// Runs the adapter's own rules, those with the `python` rule id.
    pub nullability_callback: Option<NullabilityCallback>,
    pub rich_type_inference: bool,
    pub column_types: Shapes,
    pub column_nullability: Shapes,
    pub complete_schemas: Shapes,
    /// The Python binding catalog's `schemas` before analysis.
    pub catalog_schemas: Shapes,
    /// Python's `dynamic_families_by_table`, in dict order.
    pub dynamic_families_by_table: Vec<(String, Vec<DynamicFamily>)>,
    pub models: Vec<ModelRequest>,
    /// The whole project's `(dependency ordered, cyclic)`, for a session of only some models.
    pub forced_order: Option<(bool, bool)>,
    /// Keep the relation facts the session ends with, for the project image.
    pub retain_relations: bool,
}

/// The relation facts a finished session ends with, and how it ordered its analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelationTables {
    pub types: Shapes,
    pub nullability: Shapes,
    pub complete: Shapes,
    /// The binding catalog's closed shapes, sorted by name.
    pub schemas: Shapes,
    pub dependency_ordered: bool,
    pub cyclic: bool,
    /// Models whose analysis took the legacy path, which reads every relation's facts.
    pub legacy_models: Vec<String>,
    /// The request's dynamic column families by table, in request order.
    pub families: Vec<(String, Vec<DynamicFamily>)>,
}

/// One inferred output column; nullability is Python's `InferredNullability` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnFact {
    pub name: String,
    pub data_type: Option<String>,
    pub nullability: String,
}

/// One compact lineage row with resolved names; codes index Python's enum tuples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageRow {
    pub output_column: String,
    pub transform_code: u8,
    pub confidence_code: u8,
    pub sources: Vec<(String, String, String)>,
}

/// Where a model's lineage facts live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageFacts {
    /// Native compact rows.
    Native(Vec<LineageRow>),
    /// Plain lineage facts of a native re-analysis or legacy analysis.
    NativeFacts(Vec<LineageRow>),
}

/// Python's `PolyglotAnalysisResult` for one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAnalysis {
    pub analysis_succeeded: bool,
    pub columns: Option<Vec<ColumnFact>>,
    pub lineage: LineageFacts,
    pub has_star: bool,
    pub star_resolved: bool,
    pub binding_diagnostics: Vec<DiagnosticRow>,
    pub binding_validated: bool,
}

/// The inputs a cache hit normalizes into its cleaned SQL only when a reader needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredCleaning {
    pub(crate) dialect: String,
    pub(crate) query_sql: String,
    pub(crate) placeholders: Pairs,
}

/// Python's `ModelSqlAnalysis` for one model, without the placeholders Python keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOutcome {
    pub analysis: ModelAnalysis,
    /// Empty while `deferred_cleaning` is set; read through `resolved_cleaned_sql`.
    pub cleaned_sql: String,
    pub deferred_cleaning: Option<DeferredCleaning>,
    pub validated_schema: Shapes,
    pub fused_binding_validated: bool,
    /// The SQL macro call arguments the analysed SQL shows do not fit; never cached.
    pub sql_macro_findings: Vec<SqlMacroFinding>,
}

/// The finished run of the session: what Python records from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionStep {
    /// Shapes published by the run, in publication order.
    pub publications: Shapes,
    /// Native analysis failures Python logs at debug level.
    pub failures: Vec<String>,
}

/// The session's results once every deferral is answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOutcome {
    pub models: Vec<ModelOutcome>,
    /// Relations the Python binding catalog adds to `schemas`, in order.
    pub schema_additions: Shapes,
    /// Relations whose analysis shapes the Python binding catalog records.
    pub analysis_names: Vec<String>,
    /// Each model's dynamic pivot proof, in request order.
    pub dynamic_contracts: Vec<PivotOutcome>,
}

/// One expression source's shape: inferred, absent, or left to Python.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionShape {
    Inferred(Pairs),
    Absent,
}

/// What expression-source shape inference reads from the inference profile.
#[derive(Debug, Clone, Default)]
pub struct ExpressionShapeRequest {
    pub dialect: String,
    pub case_sensitive_shapes: bool,
    pub function_return_types: Pairs,
    /// Adapter nullability rules as `(function name, rule id)`.
    pub nullability_rules: Option<Pairs>,
    /// Runs the adapter's own rules, those with the `python` rule id.
    pub nullability_callback: Option<NullabilityCallback>,
    pub expressions: Vec<String>,
}

/// Where the session is within its waves.
#[derive(Debug)]
pub(crate) enum Phase {
    Analyze,
    Complete(usize),
    Finish(usize),
    Done,
}

/// How many analysed models a session took from its cache, analysed itself, and stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalysisCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub stored: usize,
}

/// A session's per-model analysis cache of finished native outcomes in a store.
#[derive(Debug)]
pub struct AnalysisCache {
    pub(crate) store: NativeStore,
    /// Digest of the session-wide request facts every model key starts with.
    pub(crate) session_digest: ContentDigest,
    pub(crate) keys: Vec<Option<ContentDigest>>,
    pub(crate) hits: Vec<bool>,
    /// The whole-table digest each legacy analysis read; its entry also requires it.
    pub(crate) legacy_tables: Vec<Option<ContentDigest>>,
    pub(crate) stats: AnalysisCacheStats,
}

/// The relations one model reads, as the session knows them, for a native route's analysis.
#[derive(Debug, Default)]
pub(crate) struct SessionSchema {
    /// Columns of each relation with a known shape, by the analysis name the query reads.
    pub(crate) relations: HashMap<String, Vec<SchemaColumn>>,
}

impl LineageSchema for SessionSchema {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.relations
            .get(relation)
            .or_else(|| {
                self.relations
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(relation))
                    .map(|(_, columns)| columns)
            })
            .map(Vec::as_slice)
    }
}

/// One compile's model analysis, advanced phase by phase.
#[derive(Debug)]
pub struct AnalysisSession {
    pub(crate) request: SessionRequest,
    pub(crate) catalog: SessionCatalog,
    pub(crate) dependency_ordered: bool,
    /// Whether models `ref` each other in a cycle, so Python's completion pass is skipped.
    pub(crate) cyclic: bool,
    pub(crate) waves: Vec<Vec<usize>>,
    pub(crate) next_wave: usize,
    pub(crate) available_types: ShapeTable,
    pub(crate) available_nullability: ShapeTable,
    pub(crate) complete_shapes: ShapeTable,
    pub(crate) outcomes: Vec<Option<ModelOutcome>>,
    /// Cache hits whose cleaned SQL is still unnormalized.
    pub(crate) pending_cleaned: Vec<bool>,
    pub(crate) phase: Phase,
    pub(crate) publications: Shapes,
    pub(crate) failures: Vec<String>,
    pub(crate) cache: Option<AnalysisCache>,
    /// Models whose analysis took the legacy path.
    pub(crate) legacy_models: Vec<String>,
}

/// Which of Python's column analyses one query outside the session takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryColumnsMode {
    /// The precomputed compact batch: a failed analysis has no columns.
    Batch,
    /// Compact re-analysis over the input types, then the parsed tree where it declines.
    Reanalysis,
    /// `infer_columns_with_sql_analysis`: the parsed tree only.
    Parse,
    /// `analyze_columns_and_lineage_with_polyglot` with compact analysis off: the parsed tree.
    Legacy,
}

/// One query outside the session whose columns Python inferred.
#[derive(Debug, Clone)]
pub struct QueryColumnsInput {
    /// The model SQL; in `Parse` and `Legacy` modes, already through Python's normalization.
    pub sql: String,
    pub placeholders: Pairs,
    /// Every reference's analysis name, in reference order.
    pub analysis_names: Vec<String>,
    /// Python's `lineage_reference_map` items.
    pub lineage_references: Vec<(String, String, String)>,
    pub recover_cte_facts: bool,
    pub mode: QueryColumnsMode,
}

/// The queries, with the profile settings and relation facts their analysis reads.
#[derive(Debug, Clone, Default)]
pub struct QueryColumnsRequest {
    pub dialect: String,
    /// The unaliased `profile.sql_analysis_dialect or "generic"` normalization reads.
    pub normalization_dialect: String,
    pub function_return_types: Pairs,
    /// Adapter nullability rules as `(function name, rule id)`.
    pub nullability_rules: Option<Pairs>,
    /// Runs the adapter's own rules, those with the `python` rule id.
    pub nullability_callback: Option<NullabilityCallback>,
    pub types: Shapes,
    pub nullability: Shapes,
    pub queries: Vec<QueryColumnsInput>,
}

/// One query's analysis: whether it succeeded, its columns and whether it projects a star.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryColumns {
    pub succeeded: bool,
    pub columns: Option<Vec<ColumnFact>>,
    pub has_star: bool,
}

/// One query's CTE fact recovery, as Python's compact enrichment runs it.
#[derive(Debug, Clone)]
pub struct CteFactRequest {
    pub cleaned_sql: String,
    pub dialect: String,
    /// Input relation types; every input column's nullability is unknown, as in enrichment.
    pub input_schemas: Shapes,
    pub function_return_types: Pairs,
    /// Adapter nullability rules as `(function name, rule id)`; None where one is not Python's.
    pub nullability_rules: Option<Pairs>,
    /// Whether Python runs `_polyglot_cte_passthrough_facts` past its early return.
    pub recover: bool,
    /// Whether a filter reads NULL, so Python looks for filtered non-null outputs.
    pub null_filter: bool,
}

/// Python's recovered CTE pass-through facts and filtered non-null outputs for one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteFacts {
    pub types: Pairs,
    pub nullability: Pairs,
    /// Sorted direct CTE output names.
    pub direct_outputs: Vec<String>,
    /// Sorted outputs a filter proves non-null.
    pub non_null_outputs: Vec<String>,
}

/// The callback that runs project-local adapter nullability rules, the `python` rule id.
#[derive(Clone)]
pub struct NullabilityCallback(pub Arc<NullabilityRuleFn>);

impl fmt::Debug for NullabilityCallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NullabilityCallback")
    }
}

/// Python's `_simple_projection_facts` of a fixture query; names are casefolded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionFacts {
    /// `(name, type)` in projection order; a later projection of a name wins.
    pub types: Vec<(String, String)>,
    pub null_literal_names: Vec<String>,
    pub quoted_names: Vec<String>,
}

/// One model's analysis results, which `model_columns` applies in `_assemble_model`'s order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelColumnsInput<'a> {
    /// The model's analysis is enabled (`sql_analysis_enabled` for this model).
    pub analysis_enabled: bool,
    /// The finished session's result for the model.
    pub session: SessionModelAnalysis<'a>,
    /// The `Parse` mode `query_columns` result, required when enabled and the session failed.
    pub fallback: Option<&'a QueryColumns>,
    /// A standalone pivot proof, for a model with families the session did not analyse.
    pub standalone_proof: Option<&'a ContractProof>,
    /// Output columns type recovery poisoned for this model (`TypeRecoveryPlan::poisoned`).
    pub poisoned: &'a [String],
}

/// A model's final output columns, as `CompiledModel` carries them after type recovery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelColumns {
    /// `inferred_columns`; a poisoned column has no type.
    pub columns: Option<Vec<ColumnFact>>,
    /// A proven pivot forces `fast_lineage_has_star`.
    pub proven_star: bool,
    /// `unchecked_output_columns`: every poisoned name, in plan order.
    pub unchecked: Vec<String>,
}

impl ModelColumnsInput<'_> {
    /// `_assemble_model`'s columns, then `_recovered_model`'s; Err where the fallback is missing.
    pub fn model_columns(&self) -> Result<ModelColumns, String> {
        let analysed: Option<Vec<ColumnFact>> = if !self.analysis_enabled {
            None
        } else if self.session.succeeded {
            self.session.columns.map(<[ColumnFact]>::to_vec)
        } else {
            self.fallback
                .ok_or("a failed session analysis without its parse fallback")?
                .columns
                .clone()
        };
        let proven: Option<&ContractProof> = self
            .session
            .dynamic_proof
            .or(self.standalone_proof)
            .filter(|proof| proof.output_proven);
        let columns: Option<Vec<ColumnFact>> = proven
            .map(|proof| proof.fixed_columns.clone())
            .or(analysed)
            .map(|mut columns| {
                for column in &mut columns {
                    if self.poisoned.contains(&column.name) {
                        column.data_type = None;
                    }
                }
                columns
            });
        Ok(ModelColumns {
            columns,
            proven_star: proven.is_some(),
            unchecked: self.poisoned.to_vec(),
        })
    }
}
