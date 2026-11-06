//! The native model analysis session and expression-source shapes.

use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyModuleMethods, PyRef, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pyfunction, pymethods};
use sqlbuild_analysis::assembly::analysis_session::main::attach_analysis_cache::attach_analysis_cache;
use sqlbuild_analysis::assembly::analysis_session::main::expression_shapes::expression_shapes;
use sqlbuild_analysis::assembly::analysis_session::main::finish_analysis_session::finish_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::main::finished_fact_models::finished_fact_models;
use sqlbuild_analysis::assembly::analysis_session::main::model_reference_facts::model_reference_facts;
use sqlbuild_analysis::assembly::analysis_session::main::names_set_operation::names_set_operation;
use sqlbuild_analysis::assembly::analysis_session::main::prove_dynamic_contracts::prove_dynamic_contracts;
use sqlbuild_analysis::assembly::analysis_session::main::prove_finished_dynamic_contracts::prove_finished_dynamic_contracts;
use sqlbuild_analysis::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use sqlbuild_analysis::assembly::analysis_session::main::run_analysis_session::run_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::main::session_sharing::session_sharing;
use sqlbuild_analysis::assembly::analysis_session::main::start_analysis_session::start_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::main::take_analysis_cache::take_analysis_cache;
use sqlbuild_analysis::assembly::analysis_session::models::{
    AnalysisCacheStats, AnalysisSession, ColumnFact, DynamicFamily, ExpressionShape,
    ExpressionShapeRequest, FinishedSession, LineageFacts, LineageRow, ModelOutcome, ModelRequest,
    PivotBatchRequest, PivotModel, PivotOutcome, PivotTables, SessionOutcome, SessionRequest,
    SessionStep,
};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_core::constants::PANIC_MESSAGE;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;
use std::path::PathBuf;

use crate::bindings::_helpers::analysis_session::nullability_rules::{
    RuleFailure, nullability_callback, raised,
};
use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::cache::native_store::{open_store, save_store};
use crate::bindings::models::Positional;
use crate::bindings::models::ProjectCatalog;
use crate::bindings::types::CompilerDetach;

/// `(name, type, nullability)`.
type ColumnRow = (String, Option<String>, String);
positional_record! {
    into;
    /// One lineage column: output column, transform and confidence codes and its sources.
    struct LineageItem {
        output_column: String,
        transform_code: u8,
        confidence_code: u8,
        /// `(resource type, resource, column)` of each source.
        sources: Vec<(String, String, String)>,
    }
}

positional_record! {
    extract;
    /// One declared dynamic column family.
    pub(crate) struct FamilyRow {
        pub(crate) name: String,
        pub(crate) pivot_column: String,
        pub(crate) value_column: String,
        pub(crate) aggregate: String,
        pub(crate) data_type: String,
        pub(crate) name_pattern: Option<String>,
    }
}

impl From<sqlbuild_model_config::header_metadata::models::DynamicFamily> for FamilyRow {
    fn from(family: sqlbuild_model_config::header_metadata::models::DynamicFamily) -> Self {
        Self {
            name: family.name,
            pivot_column: family.pivot_column,
            value_column: family.value_column,
            aggregate: family.aggregate,
            data_type: family.data_type,
            name_pattern: family.name_pattern,
        }
    }
}

/// A model's name, SQL, placeholders, references, lineage, required names, flags and pivot.
pub(crate) struct ModelRow {
    pub(crate) name: String,
    pub(crate) query_sql: String,
    pub(crate) placeholders: Pairs,
    pub(crate) reference_rows: Vec<(String, String)>,
    pub(crate) recover_cte_facts: bool,
    pub(crate) snapshot_columns: Option<(String, String)>,
    pub(crate) dynamic_families: Vec<FamilyRow>,
    pub(crate) sql_macro_checks: Vec<SqlMacroCheck>,
}

/// The profile settings, relation shapes and models one session analyses.
pub(crate) struct RequestRow {
    pub(crate) dialect: String,
    pub(crate) case_sensitive_shapes: bool,
    pub(crate) function_return_types: Pairs,
    pub(crate) nullability_rules: Option<Pairs>,
    pub(crate) rich_type_inference: bool,
    pub(crate) column_types: Shapes,
    pub(crate) column_nullability: Shapes,
    pub(crate) complete_schemas: Shapes,
    pub(crate) catalog_schemas: Shapes,
    pub(crate) dynamic_families_by_table: Vec<(String, Vec<FamilyRow>)>,
    pub(crate) models: Vec<ModelRow>,
}

/// `(publications, failures)`.
pub(crate) type StepRow = (Shapes, Vec<String>);

positional_record! {
    into;
    /// One model's analysis, lineage source and cleaned SQL.
    struct OutcomeRow {
        analysis_succeeded: bool,
        columns: Option<Vec<ColumnRow>>,
        lineage_source: &'static str,
        lineage: Vec<LineageItem>,
        has_star: bool,
        star_resolved: bool,
        binding_diagnostics: Vec<Positional<DiagnosticRow>>,
        binding_validated: bool,
        cleaned_sql: String,
    }
}

positional_record! {
    into;
    /// One dynamic pivot proof.
    pub(crate) struct ProofRow {
        output_proven: bool,
        fixed_columns: Vec<ColumnRow>,
        /// `(family, inferred type)` of each family.
        families: Vec<(String, Option<String>)>,
        input_relations: Vec<String>,
        failure_reason: Option<String>,
        bare_dynamic_pivot: bool,
    }
}

/// One model's pivot SQL and declared families.
pub(crate) type PivotModelRow = (String, Vec<FamilyRow>);

positional_record! {
    extract;
    /// The dialect, relation facts and families by table every proof reads, and the models.
    pub(crate) struct PivotRequestRow {
        pub(crate) dialect: String,
        pub(crate) column_types: Shapes,
        pub(crate) authoritative_types: Shapes,
        pub(crate) column_nullability: Shapes,
        pub(crate) families_by_table: Vec<(String, Vec<FamilyRow>)>,
        pub(crate) models: Vec<PivotModelRow>,
    }
}

/// `("absent" | "deferred" | "proof", proof)`.
pub(crate) type ContractRow = (&'static str, Option<ProofRow>);

positional_record! {
    into;
    /// The analysis cache's hits, misses, stored entries and why it was not read or saved.
    #[derive(Clone)]
    pub(crate) struct CacheStatsRow {
        pub(crate) hits: usize,
        pub(crate) misses: usize,
        pub(crate) stored: usize,
        pub(crate) failure: Option<String>,
    }
}

positional_record! {
    extract;
    /// The settings and expressions one expression-source shape request infers.
    pub(crate) struct ExpressionShapesRow {
        pub(crate) dialect: String,
        pub(crate) case_sensitive_shapes: bool,
        pub(crate) function_return_types: Pairs,
        pub(crate) nullability_rules: Option<Pairs>,
        pub(crate) expressions: Vec<String>,
    }
}
/// `(catalog schema additions, analysis-shape names, whether each model's analysis succeeded)`.
pub(crate) type FinishRow = (Shapes, Vec<String>, Vec<bool>);

/// One compile's native model analysis; an internal failure raises `NativeCompilerError`.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct NativeModelAnalysisSession {
    inner: Option<AnalysisSession>,
    finished: Option<FinishedSession>,
    /// Each requested model's outcome and dynamic pivot proof, in request order, once finished.
    outcomes: Vec<(ModelOutcome, PivotOutcome)>,
    sharing: (usize, usize),
    cache_path: Option<PathBuf>,
    cache_stats: Option<CacheStatsRow>,
    rule_failure: RuleFailure,
    /// The analysis dialect the session's models were marked and analysed in.
    dialect: String,
}

/// What an internal native failure of the session names.
const SESSION_CONTEXT: &str = "native model analysis";
const SHAPES_CONTEXT: &str = "native expression-source shapes";
/// The native store kind holding finished model analyses.
const MODEL_ANALYSIS_STORE_KIND: &str = "model-analyses";

impl NativeModelAnalysisSession {
    /// The finished session later compile stages read, once `finish` has succeeded.
    pub(crate) fn finished_session(&self) -> Option<&FinishedSession> {
        self.finished.as_ref()
    }

    /// The finished outcome and dynamic pivot proof of the model requested at `index`.
    pub(crate) fn outcome(&self, index: usize) -> PyResult<&(ModelOutcome, PivotOutcome)> {
        self.outcomes
            .get(index)
            .ok_or_else(|| internal_error("the session returned no analysis for a model"))
    }

    /// The running session, or `NativeCompilerError` once it has failed or finished.
    pub(crate) fn dialect(&self) -> &str {
        &self.dialect
    }

    fn running(&mut self) -> PyResult<AnalysisSession> {
        self.inner
            .take()
            .ok_or_else(|| internal_error("the session is not running"))
    }
}

/// An internal native failure of the session as `NativeCompilerError`.
fn internal_error(reason: &str) -> PyErr {
    if is_compiler_panic(reason) && reason != PANIC_MESSAGE {
        return compiler_error(reason);
    }
    compiler_error(native_failure(SESSION_CONTEXT, reason))
}

impl NativeModelAnalysisSession {
    /// Analyse every model.
    pub(crate) fn run(&mut self, py: Python<'_>) -> PyResult<StepRow> {
        let mut session: AnalysisSession = self.running()?;
        let result: Result<(AnalysisSession, SessionStep), String> =
            py.compiler_detach(move || {
                run_analysis_session(&mut session).map(|step| (session, step))
            });
        if let Some(error) = raised(&self.rule_failure) {
            return Err(error);
        }
        let (session, step) = result.map_err(|reason| internal_error(&reason))?;
        self.sharing = session_sharing(&session);
        self.inner = Some(session);
        Ok(step_row(step))
    }

    /// Every model's outcome; saves the cache best-effort.
    pub(crate) fn finish(&mut self, py: Python<'_>) -> PyResult<FinishRow> {
        let mut session: AnalysisSession = self.running()?;
        let cache: Option<(NativeStore, AnalysisCacheStats)> = take_analysis_cache(&mut session);
        let (outcome, finished) = catch_compiler_panic(|| finish_analysis_session(session))
            .map_err(|reason| internal_error(&reason))?;
        self.finished = Some(finished);
        if let (Some((store, stats)), Some(path)) = (cache, self.cache_path.as_ref()) {
            let saved: Option<String> = save_store(py, &store, path, &[])
                .err()
                .map(|error| error.to_string());
            self.cache_stats = Some(CacheStatsRow {
                hits: stats.hits,
                misses: stats.misses,
                stored: stats.stored,
                failure: saved,
            });
        }
        let SessionOutcome {
            models,
            schema_additions,
            analysis_names,
            dynamic_contracts,
        } = outcome;
        let succeeded: Vec<bool> = models
            .iter()
            .map(|model| model.analysis.analysis_succeeded)
            .collect();
        self.outcomes = models.into_iter().zip(dynamic_contracts).collect();
        Ok((schema_additions, analysis_names, succeeded))
    }
}

#[pymethods]
impl NativeModelAnalysisSession {
    /// Models whose output names and lineage the finished session keeps for later stages.
    #[getter]
    pub(crate) fn fact_models(&self) -> Vec<String> {
        self.finished
            .as_ref()
            .map(finished_fact_models)
            .unwrap_or_default()
    }

    /// Each requested model's raw binding diagnostic rows, in request order.
    pub(crate) fn binding_rows(&self) -> Vec<Vec<Positional<DiagnosticRow>>> {
        self.outcomes
            .iter()
            .map(|(outcome, _)| Positional::wrap_all(outcome.analysis.binding_diagnostics.clone()))
            .collect()
    }

    /// The model at `index`'s outcome and proof rows, for hand-built model inputs only.
    fn outcome_rows(&self, index: usize) -> PyResult<(OutcomeRow, ContractRow)> {
        let (outcome, contract) = self.outcome(index)?;
        let cleaned: String = resolved_cleaned_sql(outcome)
            .map_err(compiler_error)?
            .into_owned();
        Ok((
            outcome_row(outcome.clone(), cleaned),
            contract_row(contract.clone()),
        ))
    }

    /// The analysis cache's `(hits, misses, stored, failure)`; None without a cache.
    #[getter]
    pub(crate) fn cache_stats(&self) -> Option<CacheStatsRow> {
        self.cache_stats.clone()
    }

    /// Each model's dynamic pivot proof from the finished session's tables.
    pub(crate) fn prove_dynamic_contracts(
        &self,
        py: Python<'_>,
        models: Vec<PivotModelRow>,
    ) -> PyResult<Vec<ContractRow>> {
        let session: &FinishedSession = self
            .finished
            .as_ref()
            .ok_or_else(|| internal_error("the session has not finished"))?;
        let models: Vec<PivotModel> = models.into_iter().map(pivot_model).collect();
        let outcomes: Vec<PivotOutcome> = py
            .compiler_detach(|| prove_finished_dynamic_contracts(session, &models))
            .map_err(|reason| internal_error(&reason))?;
        Ok(outcomes.into_iter().map(contract_row).collect())
    }

    /// `(shared members, shared members re-analysed alone)` as of the last step.
    #[getter]
    pub(crate) fn sharing(&self) -> (usize, usize) {
        self.sharing
    }
}

/// Start a session on `catalog`.
pub(crate) fn start_model_analysis_session(
    catalog: PyRef<'_, ProjectCatalog>,
    request: RequestRow,
    cache: Option<(PathBuf, String)>,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<NativeModelAnalysisSession> {
    let py: Python<'_> = catalog.py();
    let mut request: SessionRequest =
        crate::bindings::_helpers::compile_context::delta::inject_relations(session_request(
            request,
        ))?;
    let rule_failure: RuleFailure = RuleFailure::default();
    let dialect: String = request.dialect.clone();
    request.nullability_callback = adapter_rules
        .map(|(rules, nullability)| nullability_callback(rules, nullability, rule_failure.clone()));
    let catalog = &catalog.inner;
    let mut session: AnalysisSession = py
        .compiler_detach(|| start_analysis_session(request, catalog))
        .map_err(|reason| internal_error(&reason))?;
    let cache: Option<(PathBuf, String)> =
        cache.filter(|_| crate::bindings::_helpers::compile_context::delta::injection().is_none());
    let mut cache_path: Option<PathBuf> = None;
    let mut cache_stats: Option<CacheStatsRow> = None;
    if let Some((path, environment)) = cache {
        match open_store(py, &path, MODEL_ANALYSIS_STORE_KIND, &environment) {
            Ok(store) => {
                attach_analysis_cache(&mut session, store);
                cache_path = Some(path);
            }
            Err(error) => {
                cache_stats = Some(CacheStatsRow {
                    hits: 0,
                    misses: 0,
                    stored: 0,
                    failure: Some(error.to_string()),
                });
            }
        }
    }
    Ok(NativeModelAnalysisSession {
        inner: Some(session),
        finished: None,
        outcomes: Vec::new(),
        sharing: (0, 0),
        cache_path,
        cache_stats,
        rule_failure,
        dialect,
    })
}

/// Each expression's shape, `None` where it has none; adapter rules run as in the session.
#[pyfunction]
#[pyo3(signature = (catalog, request, adapter_rules=None))]
pub(crate) fn infer_expression_source_shapes(
    py: Python<'_>,
    catalog: PyRef<'_, ProjectCatalog>,
    request: ExpressionShapesRow,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<Vec<Option<Pairs>>> {
    let ExpressionShapesRow {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        expressions,
    } = request;
    let rule_failure: RuleFailure = RuleFailure::default();
    let request = ExpressionShapeRequest {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        nullability_callback: adapter_rules.map(|(rules, nullability)| {
            nullability_callback(rules, nullability, rule_failure.clone())
        }),
        expressions,
    };
    let catalog = &catalog.inner;
    let shapes = py.compiler_detach(|| expression_shapes(catalog, &request));
    if let Some(error) = raised(&rule_failure) {
        return Err(error);
    }
    Ok(shapes
        .map_err(|reason| compiler_error(native_failure(SHAPES_CONTEXT, &reason)))?
        .into_iter()
        .map(|shape| match shape {
            ExpressionShape::Inferred(pairs) => Some(pairs),
            ExpressionShape::Absent => None,
        })
        .collect())
}

fn session_request(request: RequestRow) -> SessionRequest {
    let RequestRow {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        rich_type_inference,
        column_types,
        column_nullability,
        complete_schemas,
        catalog_schemas,
        dynamic_families_by_table,
        models,
    } = request;
    SessionRequest {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        nullability_callback: None,
        rich_type_inference,
        column_types,
        column_nullability,
        complete_schemas,
        catalog_schemas,
        dynamic_families_by_table: families_by_table(dynamic_families_by_table),
        models: models.into_iter().map(model_request).collect(),
        forced_order: None,
        retain_relations: crate::bindings::_helpers::compile_context::delta::capturing(),
    }
}

fn model_request(row: ModelRow) -> ModelRequest {
    let ModelRow {
        name,
        query_sql,
        placeholders,
        reference_rows,
        recover_cte_facts,
        snapshot_columns,
        dynamic_families,
        sql_macro_checks,
    } = row;
    let has_set_operation = names_set_operation(&query_sql);
    let facts = model_reference_facts(&reference_rows);
    let pivot_sql = if placeholders.is_empty() {
        query_sql.clone()
    } else {
        placeholder_defaults(&query_sql, &placeholders)
    };
    ModelRequest {
        name,
        query_sql,
        placeholders,
        references: facts.references,
        lineage_references: facts.lineage_references,
        required_names: facts.required_names,
        recover_cte_facts,
        has_set_operation,
        snapshot_columns,
        pivot_sql,
        dynamic_families: dynamic_families.into_iter().map(dynamic_family).collect(),
        sql_macro_checks,
    }
}

fn step_row(step: SessionStep) -> StepRow {
    (step.publications, step.failures)
}

fn column_row(column: ColumnFact) -> ColumnRow {
    (column.name, column.data_type, column.nullability)
}

fn lineage_items(rows: Vec<LineageRow>) -> Vec<LineageItem> {
    rows.into_iter()
        .map(|row| LineageItem {
            output_column: row.output_column,
            transform_code: row.transform_code,
            confidence_code: row.confidence_code,
            sources: row.sources,
        })
        .collect()
}

fn families_by_table(rows: Vec<(String, Vec<FamilyRow>)>) -> Vec<(String, Vec<DynamicFamily>)> {
    rows.into_iter()
        .map(|(name, families)| (name, families.into_iter().map(dynamic_family).collect()))
        .collect()
}

fn dynamic_family(row: FamilyRow) -> DynamicFamily {
    let FamilyRow {
        name,
        pivot_column,
        value_column,
        aggregate,
        data_type,
        name_pattern,
    } = row;
    DynamicFamily {
        name,
        pivot_column,
        value_column,
        aggregate,
        data_type,
        name_pattern,
    }
}

fn contract_row(outcome: PivotOutcome) -> ContractRow {
    match outcome {
        PivotOutcome::Absent => ("absent", None),
        PivotOutcome::Proof(proof) => (
            "proof",
            Some(ProofRow {
                output_proven: proof.output_proven,
                fixed_columns: proof.fixed_columns.into_iter().map(column_row).collect(),
                families: proof.families,
                input_relations: proof.input_relations,
                failure_reason: proof.failure_reason,
                bare_dynamic_pivot: proof.bare_dynamic_pivot,
            }),
        ),
    }
}

fn outcome_row(outcome: ModelOutcome, cleaned_sql: String) -> OutcomeRow {
    let analysis = outcome.analysis;
    let (source, lineage) = match analysis.lineage {
        LineageFacts::Native(rows) => ("native", lineage_items(rows)),
        LineageFacts::NativeFacts(rows) => ("facts", lineage_items(rows)),
    };
    OutcomeRow {
        analysis_succeeded: analysis.analysis_succeeded,
        columns: analysis
            .columns
            .map(|columns| columns.into_iter().map(column_row).collect()),
        lineage_source: source,
        lineage,
        has_star: analysis.has_star,
        star_resolved: analysis.star_resolved,
        binding_diagnostics: Positional::wrap_all(analysis.binding_diagnostics),
        binding_validated: analysis.binding_validated,
        cleaned_sql,
    }
}

/// Each model's dynamic pivot proof outside any session.
#[pyfunction]
pub(crate) fn prove_dynamic_column_contracts(
    py: Python<'_>,
    request: PivotRequestRow,
) -> PyResult<Vec<ContractRow>> {
    let PivotRequestRow {
        dialect,
        column_types,
        authoritative_types,
        column_nullability,
        families_by_table: by_table,
        models,
    } = request;
    let request = PivotBatchRequest {
        tables: PivotTables {
            dialect,
            column_types,
            authoritative_types,
            column_nullability,
            families_by_table: families_by_table(by_table),
        },
        models: models.into_iter().map(pivot_model).collect(),
    };
    let outcomes: Vec<PivotOutcome> = py
        .compiler_detach(|| prove_dynamic_contracts(&request))
        .map_err(|reason| internal_error(&reason))?;
    Ok(outcomes.into_iter().map(contract_row).collect())
}

fn pivot_model(row: PivotModelRow) -> PivotModel {
    let (sql, families) = row;
    PivotModel {
        sql,
        families: families.into_iter().map(dynamic_family).collect(),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelAnalysisSession>()?;
    Ok(())
}
