//! Public records of analysis session.

use crate::analysis::models::Positional;
use crate::analysis_session::_helpers::fixture_columns::infer_fixture_column_facts;
use crate::analysis_session::_helpers::nullability_rules::RuleFailure;
use crate::analysis_session::_helpers::nullability_rules::raised;
use crate::analysis_session::_helpers::session::ColumnRow;
use crate::analysis_session::_helpers::session::ContractRow;
use crate::analysis_session::_helpers::session::FinishRow;
use crate::analysis_session::_helpers::session::OutcomeRow;
use crate::analysis_session::_helpers::session::PivotModelRow;
use crate::analysis_session::_helpers::session::StepRow;
use crate::analysis_session::_helpers::session::contract_row;
use crate::analysis_session::_helpers::session::internal_error;
use crate::analysis_session::_helpers::session::outcome_row;
use crate::analysis_session::_helpers::session::pivot_model;
use crate::analysis_session::_helpers::session::step_row;
use crate::analysis_session::types::PivotRequestTuple;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use sqlbuild_analysis::assembly::analysis_session::main::finish_analysis_session::finish_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::main::finished_fact_models::finished_fact_models;
use sqlbuild_analysis::assembly::analysis_session::main::prove_finished_dynamic_contracts::prove_finished_dynamic_contracts;
use sqlbuild_analysis::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use sqlbuild_analysis::assembly::analysis_session::main::run_analysis_session::run_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::main::session_sharing::session_sharing;
use sqlbuild_analysis::assembly::analysis_session::main::take_analysis_cache::take_analysis_cache;
use sqlbuild_analysis::assembly::analysis_session::models::AnalysisCacheStats;
use sqlbuild_analysis::assembly::analysis_session::models::AnalysisSession;
use sqlbuild_analysis::assembly::analysis_session::models::FinishedSession;
use sqlbuild_analysis::assembly::analysis_session::models::ModelOutcome;
use sqlbuild_analysis::assembly::analysis_session::models::PivotModel;
use sqlbuild_analysis::assembly::analysis_session::models::PivotOutcome;
use sqlbuild_analysis::assembly::analysis_session::models::SessionOutcome;
use sqlbuild_analysis::assembly::analysis_session::models::SessionStep;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;
use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::cache::main::save_store::save_store;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;
use std::path::PathBuf;
use std::sync::Mutex;

/// Reuse inference for identical fixture SQL under an equal, catalog-free profile.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug, Default)]
pub struct FixtureColumnInferences {
    pub(crate) by_profile: Mutex<Vec<(Py<PyAny>, Py<PyDict>)>>,
}

#[pymethods]
impl FixtureColumnInferences {
    #[new]
    pub fn new() -> Self {
        Self {
            by_profile: Mutex::new(Vec::new()),
        }
    }

    /// The inference for one fixture query, computing each distinct query once.
    #[pyo3(signature = (*, query_sql, inference_profile))]
    pub(crate) fn infer<'py>(
        &self,
        query_sql: &Bound<'py, PyAny>,
        inference_profile: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = query_sql.py();
        let compute = || {
            infer_fixture_column_facts(Some(&keywords(
                py,
                &[
                    ("query_sql", query_sql.clone()),
                    ("inference_profile", inference_profile.clone()),
                ],
            )?))
        };
        if !native_field(inference_profile, "binding_catalog")?.is_none() {
            return compute();
        }
        let inferred = self.profile_results(py, inference_profile)?;
        if let Some(found) = inferred.get_item(query_sql)? {
            return Ok(found);
        }
        let computed = compute()?;
        inferred.set_item(query_sql, &computed)?;
        Ok(computed)
    }
}

impl FixtureColumnInferences {
    pub(crate) fn profile_results<'py>(
        &self,
        py: Python<'py>,
        inference_profile: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let known: Vec<(Py<PyAny>, Py<PyDict>)> = self
            .by_profile
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("fixture inference cache"))?
            .iter()
            .map(|(profile, results)| (profile.clone_ref(py), results.clone_ref(py)))
            .collect();
        for (profile, results) in known {
            if profile.bind(py).eq(inference_profile)? {
                return Ok(results.into_bound(py));
            }
        }
        let results = PyDict::new(py);
        self.by_profile
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("fixture inference cache"))?
            .push((inference_profile.clone().unbind(), results.clone().unbind()));
        Ok(results)
    }
}

/// Fixture column inferences computed since the last reset.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct FixtureInferenceCount {
    pub computed: u64,
}

/// One declared dynamic column family.
#[derive(Debug)]
pub struct FamilyRow {
    pub name: String,
    pub pivot_column: String,
    pub value_column: String,
    pub aggregate: String,
    pub data_type: String,
    pub name_pattern: Option<String>,
}

impl<'py> pyo3::FromPyObject<'_, 'py> for FamilyRow {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let value: &pyo3::Bound<'py, pyo3::PyAny> = &value;
        let (name, pivot_column, value_column, aggregate, data_type, name_pattern): (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            name,
            pivot_column,
            value_column,
            aggregate,
            data_type,
            name_pattern,
        })
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
#[derive(Debug)]
pub struct ModelRow {
    pub name: String,
    pub query_sql: String,
    pub placeholders: Pairs,
    pub reference_rows: Vec<(String, String)>,
    pub recover_cte_facts: bool,
    pub snapshot_columns: Option<(String, String)>,
    pub dynamic_families: Vec<FamilyRow>,
    pub sql_macro_checks: Vec<SqlMacroCheck>,
}

/// The profile settings, relation shapes and models one session analyses.
#[derive(Debug)]
pub struct RequestRow {
    pub dialect: String,
    pub case_sensitive_shapes: bool,
    pub quoted_ignore_case: bool,
    pub function_return_types: Pairs,
    pub known_functions: Vec<String>,
    pub nullability_rules: Option<Pairs>,
    pub rich_type_inference: bool,
    pub column_types: Shapes,
    pub column_nullability: Shapes,
    pub complete_schemas: Shapes,
    pub catalog_schemas: Shapes,
    pub dynamic_families_by_table: Vec<(String, Vec<FamilyRow>)>,
    pub models: Vec<ModelRow>,
}

/// One dynamic pivot proof.
#[derive(Debug)]
pub struct ProofRow {
    pub output_proven: bool,
    pub fixed_columns: Vec<ColumnRow>,
    /// `(family, inferred type)` of each family.
    pub families: Vec<(String, Option<String>)>,
    pub input_relations: Vec<String>,
    pub failure_reason: Option<String>,
    pub bare_dynamic_pivot: bool,
}

impl<'py> pyo3::IntoPyObject<'py> for ProofRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.output_proven,
                self.fixed_columns,
                self.families,
                self.input_relations,
                self.failure_reason,
                self.bare_dynamic_pivot,
            ),
            py,
        )
    }
}

/// The dialect, relation facts and families by table every proof reads, and the models.
#[derive(Debug)]
pub struct PivotRequestRow {
    pub dialect: String,
    pub column_types: Shapes,
    pub authoritative_types: Shapes,
    pub column_nullability: Shapes,
    pub families_by_table: Vec<(String, Vec<FamilyRow>)>,
    pub models: Vec<PivotModelRow>,
}

impl<'py> pyo3::FromPyObject<'_, 'py> for PivotRequestRow {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let value: &pyo3::Bound<'py, pyo3::PyAny> = &value;
        let (
            dialect,
            column_types,
            authoritative_types,
            column_nullability,
            families_by_table,
            models,
        ): PivotRequestTuple = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            dialect,
            column_types,
            authoritative_types,
            column_nullability,
            families_by_table,
            models,
        })
    }
}

/// The analysis cache's hits, misses, stored entries and why it was not read or saved.
#[derive(Clone, Debug)]
pub struct CacheStatsRow {
    pub hits: usize,
    pub misses: usize,
    pub stored: usize,
    pub failure: Option<String>,
}

impl<'py> pyo3::IntoPyObject<'py> for CacheStatsRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject((self.hits, self.misses, self.stored, self.failure), py)
    }
}

/// One compile's native model analysis; an internal failure raises `NativeCompilerError`.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeModelAnalysisSession {
    pub(crate) inner: Option<AnalysisSession>,
    pub(crate) finished: Option<FinishedSession>,
    /// Each requested model's outcome and dynamic pivot proof, in request order, once finished.
    pub(crate) outcomes: Vec<(ModelOutcome, PivotOutcome)>,
    pub(crate) sharing: (usize, usize),
    pub(crate) cache_path: Option<PathBuf>,
    pub(crate) cache_stats: Option<CacheStatsRow>,
    pub(crate) rule_failure: RuleFailure,
    /// The analysis dialect the session's models were marked and analysed in.
    pub(crate) dialect: String,
}

impl NativeModelAnalysisSession {
    /// The finished session later compile stages read, once `finish` has succeeded.
    pub fn finished_session(&self) -> Option<&FinishedSession> {
        self.finished.as_ref()
    }

    /// Each requested model's finished outcome and dynamic pivot proof, in request order.
    pub fn outcomes(&self) -> &[(ModelOutcome, PivotOutcome)] {
        &self.outcomes
    }

    /// The finished outcome and dynamic pivot proof of the model requested at `index`.
    pub fn outcome(&self, index: usize) -> PyResult<&(ModelOutcome, PivotOutcome)> {
        self.outcomes
            .get(index)
            .ok_or_else(|| internal_error("the session returned no analysis for a model"))
    }

    /// The running session, or `NativeCompilerError` once it has failed or finished.
    pub fn dialect(&self) -> &str {
        &self.dialect
    }

    pub(crate) fn running(&mut self) -> PyResult<AnalysisSession> {
        self.inner
            .take()
            .ok_or_else(|| internal_error("the session is not running"))
    }
}

impl NativeModelAnalysisSession {
    /// Analyse every model.
    pub fn run(&mut self, py: Python<'_>) -> PyResult<StepRow> {
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
    pub fn finish(&mut self, py: Python<'_>) -> PyResult<FinishRow> {
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
    pub fn fact_models(&self) -> Vec<String> {
        self.finished
            .as_ref()
            .map(finished_fact_models)
            .unwrap_or_default()
    }

    /// Each requested model's raw binding diagnostic rows, in request order.
    pub fn binding_rows(&self) -> Vec<Vec<Positional<DiagnosticRow>>> {
        self.outcomes
            .iter()
            .map(|(outcome, _)| Positional::wrap_all(outcome.analysis.binding_diagnostics.clone()))
            .collect()
    }

    /// The model at `index`'s outcome and proof rows, for hand-built model inputs only.
    pub(crate) fn outcome_rows(&self, index: usize) -> PyResult<(OutcomeRow, ContractRow)> {
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
    pub fn cache_stats(&self) -> Option<CacheStatsRow> {
        self.cache_stats.clone()
    }

    /// Each model's dynamic pivot proof from the finished session's tables.
    pub fn prove_dynamic_contracts(
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
    pub fn sharing(&self) -> (usize, usize) {
        self.sharing
    }
}
