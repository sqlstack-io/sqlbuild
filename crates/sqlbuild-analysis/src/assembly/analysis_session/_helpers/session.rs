//! The session's phases: Python's uncached model analysis, wave by wave.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::ThreadPool;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::errors::StoreDecodeError;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::assembly::analysis_session::_helpers::analysis_cache::{
    CachedOutcome, decode_outcome, encode_outcome,
};
use crate::assembly::analysis_session::_helpers::catalog_state::SessionCatalog;
use crate::assembly::analysis_session::_helpers::catalog_state::{ShapeOptions, ShapeSource};
use crate::assembly::analysis_session::_helpers::compact_batch::{
    Batch, BatchMember, MemberAnalysis, MemberResult,
};
use crate::assembly::analysis_session::_helpers::cte_facts::{
    LegacyAnalysis, LegacyInput, RecoveryProfile, legacy_analysis,
};
use crate::assembly::analysis_session::_helpers::dynamic_pivot::{PivotFacts, pivot_outcome};
use crate::assembly::analysis_session::_helpers::enrichment::{
    Enrichment, EnrichmentInput, enrichment,
};
use crate::assembly::analysis_session::_helpers::mappings::{
    ShapeTable, dict_from_pairs, same_keys, same_relations,
};
use crate::assembly::analysis_session::_helpers::mappings::{producers, waves};
use crate::assembly::analysis_session::_helpers::resolution_rows::with_resolution_rows;
use crate::assembly::analysis_session::constants::{
    CONFIDENCE_HIGH, CONFIDENCE_MEDIUM, CONFIDENCE_UNKNOWN, ERROR_SEVERITY, QUOTE,
    SQLQUACK_NAMES_ONLY_DIALECTS, TRANSFORM_AGGREGATION, TRANSFORM_CAST, TRANSFORM_CONSTANT,
    TRANSFORM_DIRECT, TRANSFORM_EXPRESSION, TRANSFORM_STAR, UNKNOWN_COLUMN_CODE,
    UNKNOWN_NULLABILITY, UNKNOWN_TYPE,
};
use crate::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use crate::assembly::analysis_session::models::{
    AnalysisSession, ColumnFact, ContractProof, DeferredCleaning, FinishedSession, LineageFacts,
    LineageRow, ModelAnalysis, ModelOutcome, ModelRequest, Phase, PivotOutcome, PivotTables,
    RelationTables, SessionModelFacts, SessionOutcome, SessionRequest, SessionStep,
};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::lineage::models::{LineageConfidence, LineageTransformKind};
use crate::semantic_validation::_helpers::catalog::FindingInput;
use crate::semantic_validation::models::DiagnosticRow;
use crate::semantic_validation::models::ProjectCatalog;
use crate::sql_macros::main::check_sql_macro_arguments::check_sql_macro_arguments;
use crate::sql_macros::models::SqlMacroFinding;
use crate::sql_macros::types::ArgumentCheckInput;
use crate::sqlquack_analysis::main::compile_lineage::compile_lineage;
use crate::sqlquack_analysis::main::reads_dialect::reads_dialect;
use crate::sqlquack_analysis::models::{
    CompileFunctions, CompileLineageColumn, CompiledLineage, CompiledModelSql,
};

/// What an internal native failure of the session names.
const SESSION_CONTEXT: &str = "native model analysis";

impl AnalysisSession {
    /// A session over `request`; a `ref` cycle is one wave.
    pub(crate) fn start(request: SessionRequest, catalog: &ProjectCatalog) -> Result<Self, String> {
        let producers: Vec<Vec<usize>> = producers(&request.models)
            .ok_or_else(|| native_failure(SESSION_CONTEXT, "two analysed models share a name"))?;
        let scheduled: Option<Vec<Vec<usize>>> = waves(&producers);
        let complete_shapes: ShapeTable = ShapeTable::from_shapes(&request.complete_schemas);
        let (dependency_ordered, cyclic): (bool, bool) = match request.forced_order {
            Some(order) => order,
            None => (
                scheduled.is_some()
                    && request
                        .models
                        .iter()
                        .flat_map(|model| model.required_names.iter())
                        .any(|name| complete_shapes.get(name).is_none_or(Vec::is_empty)),
                scheduled.is_none(),
            ),
        };
        let waves: Vec<Vec<usize>> = scheduled.unwrap_or_default();
        Ok(Self {
            catalog: SessionCatalog::new(catalog, &request.catalog_schemas),
            available_types: ShapeTable::from_shapes(&request.column_types),
            available_nullability: ShapeTable::from_shapes(&request.column_nullability),
            complete_shapes,
            outcomes: vec![None; request.models.len()],
            pending_cleaned: vec![false; request.models.len()],
            routed: vec![false; request.models.len()],
            name_findings: HashMap::new(),
            waves: if dependency_ordered || request.models.is_empty() {
                waves
            } else {
                vec![(0..request.models.len()).collect()]
            },
            dependency_ordered,
            cyclic,
            next_wave: 0,
            phase: Phase::Analyze,
            publications: Vec::new(),
            failures: Vec::new(),
            cache: None,
            legacy_models: Vec::new(),
            request,
        })
    }

    /// Analyse every wave; any internal native failure fails the whole analysis.
    pub(crate) fn run(&mut self) -> Result<SessionStep, String> {
        loop {
            match std::mem::replace(&mut self.phase, Phase::Done) {
                Phase::Done => return Ok(self.step()),
                Phase::Analyze => {
                    let Some(models) = self.waves.get(self.next_wave).cloned() else {
                        return Ok(self.step());
                    };
                    let wave: usize = self.next_wave;
                    self.next_wave += 1;
                    self.analyze_wave(&models)?;
                    self.phase = Phase::Complete(wave);
                }
                Phase::Complete(wave) => {
                    self.complete_wave(wave)?;
                    self.phase = Phase::Finish(wave);
                }
                Phase::Finish(wave) => {
                    self.finish_wave(wave)?;
                    self.phase = Phase::Analyze;
                }
            }
        }
    }

    /// Every model's outcome and the binding catalog changes Python records.
    pub(crate) fn finish(self) -> Result<(SessionOutcome, FinishedSession), String> {
        if !matches!(self.phase, Phase::Done) {
            return Err("the session has not finished".to_owned());
        }
        let dynamic_contracts: Vec<PivotOutcome> = self.dynamic_contracts()?;
        let pool: Result<Arc<ThreadPool>, String> = self.catalog.native.analysis_pool();
        let mut models: Vec<ModelOutcome> = self
            .outcomes
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("the session left a model unanalysed")?;
        let relations: Option<RelationTables> =
            self.request.retain_relations.then(|| RelationTables {
                types: owned_rows(&self.available_types),
                nullability: owned_rows(&self.available_nullability),
                complete: owned_rows(&self.complete_shapes),
                schemas: self.catalog.schema_rows(),
                dependency_ordered: self.dependency_ordered,
                cyclic: self.cyclic,
                legacy_models: self.legacy_models.clone(),
                families: self.request.dynamic_families_by_table.clone(),
            });
        let (schema_additions, analysis_names) = self.catalog.into_changes();
        let mut request: SessionRequest = self.request;
        for ((outcome, model), pending) in models
            .iter_mut()
            .zip(request.models.iter_mut())
            .zip(self.pending_cleaned)
        {
            if pending {
                outcome.deferred_cleaning = Some(DeferredCleaning {
                    dialect: request.dialect.clone(),
                    query_sql: std::mem::take(&mut model.query_sql),
                    placeholders: std::mem::take(&mut model.placeholders),
                });
            }
        }
        let mut model_facts: HashMap<String, SessionModelFacts> = HashMap::new();
        let mut dynamic_proofs: HashMap<String, ContractProof> = HashMap::new();
        for ((model, outcome), proof) in request.models.iter().zip(&models).zip(&dynamic_contracts)
        {
            if let Some(facts) = session_model_facts(&outcome.analysis) {
                model_facts.insert(model.name.clone(), facts);
            }
            if let PivotOutcome::Proof(proof) = proof {
                dynamic_proofs.insert(model.name.clone(), proof.clone());
            }
        }
        let finished = FinishedSession {
            tables: PivotTables {
                dialect: request.dialect,
                column_types: request.column_types,
                authoritative_types: request.complete_schemas,
                column_nullability: request.column_nullability,
                families_by_table: request.dynamic_families_by_table,
            },
            pool,
            models: model_facts,
            dynamic_proofs,
            relations,
        };
        let outcome = SessionOutcome {
            models,
            schema_additions,
            analysis_names,
            dynamic_contracts,
        };
        Ok((outcome, finished))
    }

    /// Python's dynamic pivot proof of each model over the relation facts before analysis.
    fn dynamic_contracts(&self) -> Result<Vec<PivotOutcome>, String> {
        let models: &[ModelRequest] = &self.request.models;
        if models.iter().all(|model| model.dynamic_families.is_empty()) {
            return Ok(vec![PivotOutcome::Absent; models.len()]);
        }
        let facts = PivotFacts {
            dialect: &self.request.dialect,
            column_types: &self.request.column_types,
            authoritative_types: &self.request.complete_schemas,
            column_nullability: &self.request.column_nullability,
            families_by_table: &self.request.dynamic_families_by_table,
        };
        let pool = self.catalog.native.analysis_pool()?;
        let outcomes: Vec<Result<PivotOutcome, String>> = pool.install(|| {
            models
                .par_iter()
                .map(|model| pivot_outcome(&model.pivot_sql, &model.dynamic_families, &facts))
                .collect()
        });
        outcomes.into_iter().collect()
    }

    fn step(&mut self) -> SessionStep {
        SessionStep {
            publications: std::mem::take(&mut self.publications),
            failures: std::mem::take(&mut self.failures),
        }
    }

    fn outcome_mut(&mut self, model: usize) -> Result<&mut ModelOutcome, String> {
        self.outcomes
            .get_mut(model)
            .and_then(Option::as_mut)
            .ok_or_else(|| native_failure(SESSION_CONTEXT, "a model was left unanalysed"))
    }

    /// Python's `_analyze_model_sql_requests` for one wave, less the models the cache answers.
    fn analyze_wave(&mut self, models: &[usize]) -> Result<(), String> {
        let schemas: Vec<Shapes> = models
            .iter()
            .map(|model| self.binding_schema(&self.request.models[*model]))
            .collect();
        let misses: Vec<usize> = self.read_cache(models, &schemas)?;
        let misses: Vec<usize> = self.sqlquack_analyses(models, misses, &schemas)?;
        let batch = Batch {
            dialect: &self.request.dialect,
            function_return_types: &self.request.function_return_types,
            rich_type_inference: self.request.rich_type_inference,
            members: misses
                .iter()
                .map(|position| {
                    batch_member(&self.request.models[models[*position]], &schemas[*position])
                })
                .collect(),
            types: &self.available_types,
            nullability: &self.available_nullability,
        };
        let results: Vec<MemberResult> = self.catalog.analyze_batch(&batch)?;
        let mut tables: Option<ContentDigest> = None;
        for (position, result) in misses.into_iter().zip(results) {
            let model: &usize = &models[position];
            let schema: Shapes = schemas[position].clone();
            if let Some(failure) = result.failure {
                self.failures.push(failure);
            }
            let diagnostics: Vec<DiagnosticRow> = result.binding_diagnostics.unwrap_or_default();
            let analysis: ModelAnalysis = match result.analysis {
                MemberAnalysis::Projected {
                    columns,
                    lineage,
                    has_star,
                    star_resolved,
                } => ModelAnalysis {
                    analysis_succeeded: true,
                    columns: Some(columns),
                    lineage: LineageFacts::Native(lineage),
                    has_star,
                    star_resolved,
                    binding_diagnostics: diagnostics,
                    binding_validated: true,
                },
                MemberAnalysis::Failed => {
                    failed_analysis(LineageFacts::Native(Vec::new()), diagnostics)
                }
                MemberAnalysis::Legacy { lineage } => {
                    let request: &ModelRequest = &self.request.models[*model];
                    let legacy: LegacyAnalysis =
                        self.native_legacy(request, &result.cleaned_sql)
                            .map_err(|reason| native_failure(&model_context(request), &reason))?;
                    let name: String = request.name.clone();
                    self.record_legacy_tables(*model, &mut tables);
                    self.legacy_models.push(name);
                    legacy_outcome(legacy, lineage, diagnostics)
                }
            };
            self.outcomes[*model] = Some(ModelOutcome {
                sql_macro_findings: Vec::new(),
                analysis,
                cleaned_sql: result.cleaned_sql,
                deferred_cleaning: None,
                validated_schema: schema,
                fused_binding_validated: true,
            });
        }
        self.check_sql_macros(models)
    }

    /// Each model's SQL macro call arguments checked against its analysed types, cache hits too.
    fn check_sql_macros(&mut self, models: &[usize]) -> Result<(), String> {
        let checked: Vec<usize> = models
            .iter()
            .copied()
            .filter(|model| {
                let request: &ModelRequest = &self.request.models[*model];
                !request.sql_macro_checks.is_empty() && request.placeholders.is_empty()
            })
            .collect();
        self.ensure_cleaned(&checked)?;
        let mut inputs: Vec<(usize, String, Shapes)> = Vec::with_capacity(checked.len());
        for model in checked {
            let Some(outcome) = self.outcomes[model].as_ref() else {
                continue;
            };
            let Ok(cleaned) = resolved_cleaned_sql(outcome) else {
                continue;
            };
            let schema: Shapes = self.binding_schema(&self.request.models[model]);
            inputs.push((model, cleaned.into_owned(), schema));
        }
        let batch: Vec<ArgumentCheckInput<'_>> = inputs
            .iter()
            .map(|(model, cleaned, schema)| {
                (
                    cleaned.as_str(),
                    schema.as_slice(),
                    self.request.models[*model].sql_macro_checks.as_slice(),
                )
            })
            .collect();
        let findings: Vec<Vec<SqlMacroFinding>> =
            check_sql_macro_arguments(&self.request.dialect, &batch);
        for ((model, _, _), found) in inputs.iter().zip(findings) {
            if let Some(outcome) = self.outcomes[*model].as_mut() {
                outcome.sql_macro_findings = found;
            }
        }
        Ok(())
    }

    /// Python's legacy analysis of a model the engine handed back, or why Python must answer.
    fn native_legacy(
        &self,
        model: &ModelRequest,
        cleaned_sql: &str,
    ) -> Result<LegacyAnalysis, String> {
        catch_compiler_panic(|| {
            legacy_analysis(&LegacyInput {
                cleaned_sql,
                lineage_references: &model.lineage_references,
                recover: model.recover_cte_facts,
                full_nullability: false,
                types: &self.available_types,
                nullability: &self.available_nullability,
                profile: RecoveryProfile {
                    dialect: &self.request.dialect,
                    function_return_types: &self.request.function_return_types,
                    rules: self.request.nullability_rules.as_ref(),
                    callback: self.request.nullability_callback.as_ref(),
                },
            })
        })
    }

    /// The closed shape of each relation the model binds, open relations empty.
    fn binding_schema(&self, model: &ModelRequest) -> Shapes {
        model
            .required_names
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    self.complete_shapes.get(name).cloned().unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Python's `_complete_inferred_bindings` star and type pass for one wave.
    fn complete_wave(&mut self, wave: usize) -> Result<(), String> {
        if self.cyclic {
            return Ok(());
        }
        let mut candidates: Vec<(usize, Shapes, bool)> = Vec::new();
        for model in self.waves[wave].clone() {
            if self.cached(model) || self.routed[model] {
                continue;
            }
            let request: &ModelRequest = &self.request.models[model];
            let required: Vec<String> = request.required_names.clone();
            let unmarked_set_operation: bool = request.has_set_operation
                && !request
                    .query_sql
                    .contains(polyglot_sql::expressions::TYPED_EXPRESSION_FUNCTION);
            let inputs_known: bool = !required.is_empty()
                && required
                    .iter()
                    .all(|name| self.complete_shapes.contains(name));
            let stars_expanded_natively: bool =
                required.iter().all(|name| self.supplied_relation(name));
            let input_schemas: Shapes = self.input_schemas(&required);
            let analysis: &mut ModelAnalysis = &mut self.outcome_mut(model)?.analysis;
            let mut star_pending: bool =
                analysis.has_star && !analysis.star_resolved && inputs_known;
            if star_pending && has_columns(analysis.columns.as_ref()) && stars_expanded_natively {
                analysis.star_resolved = true;
                star_pending = false;
            }
            let untyped: bool = analysis
                .columns
                .iter()
                .flatten()
                .any(|column| column.data_type.is_none());
            let marked: bool = inputs_known
                && self.request.models[model]
                    .query_sql
                    .contains(polyglot_sql::expressions::TYPED_EXPRESSION_FUNCTION);
            let analysis: &ModelAnalysis = &self.outcome_mut(model)?.analysis;
            if !required.is_empty()
                && (untyped || star_pending || marked && analysis.analysis_succeeded)
                && !unmarked_set_operation
            {
                candidates.push((model, input_schemas, star_pending));
            }
        }
        let enrichments: Vec<Enrichment> = self.native_enrichments(&candidates)?;
        for ((model, _, star_pending), native) in candidates.into_iter().zip(enrichments) {
            let marked: bool = self.request.models[model]
                .query_sql
                .contains(polyglot_sql::expressions::TYPED_EXPRESSION_FUNCTION);
            let outcome: &mut ModelOutcome = self.outcome_mut(model)?;
            outcome.analysis = enriched_analysis(&outcome.analysis, native, (star_pending, marked));
        }
        Ok(())
    }

    /// Python's re-analysis of each candidate; the first failure in model order fails it.
    fn native_enrichments(
        &self,
        candidates: &[(usize, Shapes, bool)],
    ) -> Result<Vec<Enrichment>, String> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let inputs: Vec<EnrichmentInput<'_>> = candidates
            .iter()
            .map(|(model, input_schemas, _)| EnrichmentInput {
                model: &self.request.models[*model],
                input_schemas,
                dialect: &self.request.dialect,
                function_return_types: &self.request.function_return_types,
                nullability_rules: self.request.nullability_rules.as_ref(),
                nullability_callback: self.request.nullability_callback.as_ref(),
            })
            .collect();
        let pool = self.catalog.native.analysis_pool()?;
        let enrichments: Vec<Result<Enrichment, String>> =
            pool.install(|| inputs.par_iter().map(enrichment).collect());
        enrichments
            .into_iter()
            .zip(&inputs)
            .map(|(enrichment, input)| {
                enrichment.map_err(|reason| {
                    native_failure(
                        &format!("{} enrichment", model_context(input.model)),
                        &reason,
                    )
                })
            })
            .collect()
    }

    fn input_schemas(&self, required: &[String]) -> Shapes {
        let mut schemas: Shapes = Vec::with_capacity(required.len());
        for name in required {
            if let Some(shape) = self.complete_shapes.get(name) {
                schemas.push((name.clone(), shape.clone()));
            }
        }
        schemas
    }

    /// Whether the dataflow supplied `name`'s closed shape to the wave's analysis.
    fn supplied_relation(&self, name: &str) -> bool {
        let Some(shape) = self.complete_shapes.get(name) else {
            return false;
        };
        self.dependency_ordered
            && same_keys(self.available_types.get(name).unwrap_or(&Vec::new()), shape)
    }

    /// Python's deferred binding validation, then the dataflow's shape publication.
    fn finish_wave(&mut self, wave: usize) -> Result<(), String> {
        let models: Vec<usize> = self.waves[wave].clone();
        let validated: &[usize] = if self.cyclic { &[] } else { &models };
        let mut revalidated: Vec<(usize, Shapes)> = Vec::new();
        for model in validated {
            let schema: Shapes = self.binding_schema(&self.request.models[*model]);
            let outcome: &ModelOutcome = self.outcomes[*model]
                .as_ref()
                .ok_or("a wave model was not analysed")?;
            if !outcome.fused_binding_validated
                || !same_relations(&outcome.validated_schema, &schema)
            {
                revalidated.push((*model, schema));
            }
        }
        let pending: Vec<usize> = revalidated.iter().map(|(model, _)| *model).collect();
        self.ensure_cleaned(&pending)?;
        let mut validations: Vec<(usize, String, Shapes)> = Vec::new();
        for (model, schema) in revalidated {
            let outcome: &ModelOutcome = self.outcomes[model]
                .as_ref()
                .ok_or("a wave model was not analysed")?;
            validations.push((model, outcome.cleaned_sql.clone(), schema));
        }
        if !validations.is_empty() {
            let requests: Vec<(&str, &Shapes)> = validations
                .iter()
                .map(|(_, sql, schema)| (sql.as_str(), schema))
                .collect();
            let prepared: Vec<_> = self.catalog.prepare(&requests);
            let rows: Vec<Vec<DiagnosticRow>> = self.catalog.native.binding_results(
                prepared,
                crate::semantic_validation::models::UnknownFunctions::Notes,
            )?;
            for ((model, _, _), diagnostics) in validations.iter().zip(rows) {
                let analysis: &mut ModelAnalysis = &mut self.outcome_mut(*model)?.analysis;
                analysis.binding_diagnostics = diagnostics;
                analysis.binding_validated = true;
            }
        }
        self.merge_name_findings(&models)?;
        self.sqlquack_lineage(&models)?;
        if self.dependency_ordered {
            if self.request.case_sensitive_shapes {
                self.ensure_cleaned(&models)?;
            }
            for model in &models {
                self.publish(*model)?;
            }
        }
        self.store_cached(&models);
        Ok(())
    }

    /// SQLQuack's lineage in place of the analysis's, for the wave's models the cache did not answer.
    fn sqlquack_lineage(&mut self, models: &[usize]) -> Result<(), String> {
        let analysed: Vec<usize> = models
            .iter()
            .copied()
            .filter(|model| !self.cached(*model) && !self.routed[*model])
            .collect();
        if analysed.is_empty() || !self.sqlquack_reads() || self.names_only() {
            return Ok(());
        }
        let pool: Arc<ThreadPool> = self.catalog.native.analysis_pool()?;
        let nullability: Vec<Shapes> = analysed
            .iter()
            .map(|model| self.input_nullability(*model))
            .collect();
        let types: Vec<Shapes> = analysed
            .iter()
            .map(|model| self.input_types(*model))
            .collect();
        let mut inputs: Vec<CompiledModelSql<'_>> = Vec::with_capacity(analysed.len());
        for ((model, nullability), types) in analysed.iter().zip(&nullability).zip(&types) {
            let outcome: &ModelOutcome = self.outcomes[*model]
                .as_ref()
                .ok_or("a wave model was not analysed")?;
            inputs.push(CompiledModelSql {
                sql: &outcome.cleaned_sql,
                references: &self.request.models[*model].lineage_references,
                schema: &outcome.validated_schema,
                types,
                nullability,
            });
        }
        let Some(lineages) = compile_lineage(
            &self.request.dialect,
            &inputs,
            self.compile_functions(),
            &pool,
        )?
        else {
            return Ok(());
        };
        for (model, lineage) in analysed.into_iter().zip(lineages) {
            let outcome: &mut ModelOutcome = self.outcome_mut(model)?;
            let (Some(lineage), true) = (lineage, outcome.analysis.analysis_succeeded) else {
                continue;
            };
            outcome.analysis.columns = Some(lineage.columns.iter().map(column_fact).collect());
            let rows: Vec<LineageRow> = lineage.columns.into_iter().map(lineage_row).collect();
            outcome.analysis.lineage = match &outcome.analysis.lineage {
                LineageFacts::Native(_) => LineageFacts::Native(rows),
                LineageFacts::NativeFacts(_) => LineageFacts::NativeFacts(rows),
            };
        }
        Ok(())
    }

    /// The catalog nullability of each relation `model` reads.
    fn input_nullability(&self, model: usize) -> Shapes {
        self.request.models[model]
            .required_names
            .iter()
            .filter_map(|name| {
                let pairs: &Pairs = self.available_nullability.get(name)?;
                Some((name.clone(), pairs.clone()))
            })
            .collect()
    }

    /// The catalog types of each relation `model` reads, its column list complete or not.
    fn input_types(&self, model: usize) -> Shapes {
        self.request.models[model]
            .required_names
            .iter()
            .filter_map(|name| {
                let pairs: &Pairs = self.available_types.get(name)?;
                Some((name.clone(), pairs.clone()))
            })
            .collect()
    }

    /// Answer `models` from the cache where it can; returns the positions it cannot answer.
    fn read_cache(&mut self, models: &[usize], schemas: &[Shapes]) -> Result<Vec<usize>, String> {
        let Some(mut cache) = self.cache.take() else {
            return Ok((0..models.len()).collect());
        };
        let mut required: Vec<String> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        for model in models {
            for reference in &self.request.models[*model].references {
                if seen.insert(reference.analysis_name.as_str()) {
                    required.push(reference.analysis_name.clone());
                }
            }
        }
        self.catalog.prepare_analysis(
            &required,
            &self.available_types,
            &self.available_nullability,
        );
        for schema in schemas {
            let _ = self.catalog.prepare(&[("", schema)]);
        }
        let pool: Arc<ThreadPool> = self.catalog.native.analysis_pool()?;
        let session_digest: ContentDigest = cache.session_digest;
        let names: Vec<&str> = models
            .iter()
            .flat_map(|model| self.model_relation_names(*model))
            .collect::<HashSet<&str>>()
            .into_iter()
            .collect();
        let keys: Vec<ContentDigest> = pool.install(|| {
            let relations: HashMap<&str, ContentDigest> = names
                .par_iter()
                .map(|name| (*name, self.relation_digest(name)))
                .collect();
            models
                .par_iter()
                .zip(schemas)
                .map(|(model, schema)| self.model_key(&session_digest, *model, schema, &relations))
                .collect()
        });
        let stored: Vec<Option<Vec<u8>>> = keys
            .iter()
            .map(|key| cache.store.get(key).map(<[u8]>::to_vec))
            .collect();
        let decoded: Vec<Option<Result<CachedOutcome, StoreDecodeError>>> = pool.install(|| {
            stored
                .par_iter()
                .zip(schemas)
                .map(|(bytes, schema)| Some(decode_outcome(bytes.as_ref()?, schema)))
                .collect()
        });
        let mut tables: Option<ContentDigest> = None;
        let mut hits: Vec<(usize, ModelOutcome)> = Vec::new();
        let mut misses: Vec<usize> = Vec::new();
        for (position, (model, found)) in models.iter().zip(decoded).enumerate() {
            cache.keys[*model] = Some(keys[position]);
            let hit: Option<ModelOutcome> = match found {
                Some(Ok((outcome, None))) => Some(outcome),
                Some(Ok((outcome, Some(required)))) => {
                    let current: ContentDigest =
                        *tables.get_or_insert_with(|| self.tables_digest());
                    (current == required).then_some(outcome)
                }
                Some(Err(StoreDecodeError)) | None => None,
            };
            match hit {
                Some(outcome) => hits.push((position, outcome)),
                None => misses.push(position),
            }
        }
        for (position, outcome) in hits {
            let model: usize = models[position];
            self.outcomes[model] = Some(outcome);
            self.pending_cleaned[model] = true;
            cache.hits[model] = true;
        }
        misses.sort_unstable();
        cache.stats.hits += models.len() - misses.len();
        cache.stats.misses += misses.len();
        self.cache = Some(cache);
        Ok(misses)
    }

    /// Normalize the cleaned SQL of the cache hits among `models` that still lack it.
    fn ensure_cleaned(&mut self, models: &[usize]) -> Result<(), String> {
        let pending: Vec<usize> = models
            .iter()
            .copied()
            .filter(|model| self.pending_cleaned[*model])
            .collect();
        if pending.is_empty() {
            return Ok(());
        }
        let schemas: Vec<Shapes> = pending
            .iter()
            .map(|model| self.binding_schema(&self.request.models[*model]))
            .collect();
        let members: Vec<BatchMember<'_>> = pending
            .iter()
            .zip(&schemas)
            .map(|(model, schema)| batch_member(&self.request.models[*model], schema))
            .collect();
        let cleaned: Vec<Result<String, String>> = self
            .catalog
            .normalize_each(&self.request.dialect, &members)?;
        for (model, sql) in pending.into_iter().zip(cleaned) {
            let sql: String = sql.map_err(|reason| {
                native_failure(&model_context(&self.request.models[model]), &reason)
            })?;
            self.outcome_mut(model)?.cleaned_sql = sql;
            self.pending_cleaned[model] = false;
        }
        Ok(())
    }

    /// Store each finished native outcome of `models` the cache did not answer.
    fn store_cached(&mut self, models: &[usize]) {
        let Some(cache) = self.cache.as_mut() else {
            return;
        };
        for model in models {
            if cache.hits[*model] {
                continue;
            }
            let (Some(key), Some(outcome)) = (cache.keys[*model], self.outcomes[*model].as_ref())
            else {
                continue;
            };
            let bytes: Vec<u8> = encode_outcome(outcome, cache.legacy_tables[*model].as_ref());
            cache.store.put(key, bytes);
            cache.stats.stored += 1;
        }
    }

    /// Record the whole-table digest a legacy analysis of `model` read, computed once a wave.
    fn record_legacy_tables(&mut self, model: usize, tables: &mut Option<ContentDigest>) {
        if self.cache.is_none() {
            return;
        }
        let digest: ContentDigest = *tables.get_or_insert_with(|| self.tables_digest());
        if let Some(cache) = self.cache.as_mut() {
            cache.legacy_tables[model] = Some(digest);
        }
    }

    /// The dataflow's `_publish` for one analysed model.
    fn publish(&mut self, model: usize) -> Result<(), String> {
        let request: &ModelRequest = &self.request.models[model];
        let outcome: &ModelOutcome = self.outcomes[model]
            .as_ref()
            .ok_or("a published model was not analysed")?;
        let analysis: &ModelAnalysis = &outcome.analysis;
        let required_known: bool = request
            .required_names
            .iter()
            .all(|name| self.complete_shapes.contains(name));
        let star_known: bool = !analysis.has_star || (analysis.star_resolved && required_known);
        let Some(columns) = analysis
            .columns
            .as_ref()
            .filter(|columns| !columns.is_empty())
        else {
            return Ok(());
        };
        if !star_known {
            return Ok(());
        }
        let inputs: Shapes = self.binding_schema(request);
        let shape: Pairs = self.catalog.published_model_shape(
            &ShapeOptions {
                dialect: &self.request.dialect,
                case_sensitive: self.request.case_sensitive_shapes,
            },
            ShapeSource {
                sql: if outcome.cleaned_sql.is_empty() {
                    &request.query_sql
                } else {
                    &outcome.cleaned_sql
                },
                columns: shape_columns(columns),
                inputs: &inputs,
                snapshot_columns: request.snapshot_columns.as_ref(),
            },
        )?;
        let name: String = request.name.clone();
        let nullability: Pairs = shape
            .iter()
            .map(|(column, _)| (column.clone(), UNKNOWN_NULLABILITY.to_owned()))
            .collect();
        self.complete_shapes.set_default(&name, shape.clone());
        self.available_types.set_default(&name, shape.clone());
        self.available_nullability.set_default(&name, nullability);
        self.publications.push((name, shape));
        Ok(())
    }
}

fn batch_member<'a>(model: &'a ModelRequest, schema: &'a Shapes) -> BatchMember<'a> {
    BatchMember {
        query_sql: &model.query_sql,
        placeholders: &model.placeholders,
        analysis_names: model
            .references
            .iter()
            .map(|reference| reference.analysis_name.as_str())
            .collect(),
        lineage_references: &model.lineage_references,
        recover_cte_facts: model.recover_cte_facts,
        binding_schema: Some(schema),
    }
}

/// A table's shapes in insertion order, owned.
fn owned_rows(table: &ShapeTable) -> Shapes {
    table
        .ordered()
        .into_iter()
        .map(|(name, shape)| (name.to_owned(), shape.clone()))
        .collect()
}

/// Python's `{column.name: column.type or "UNKNOWN"}`.
pub(crate) fn shape_columns(columns: &[ColumnFact]) -> Pairs {
    dict_from_pairs(columns.iter().map(shape_column))
}

fn shape_column(column: &ColumnFact) -> (String, String) {
    let data_type: &str = match column.data_type.as_deref() {
        Some(data_type) if !data_type.is_empty() => data_type,
        _ => UNKNOWN_TYPE,
    };
    (column.name.clone(), data_type.to_owned())
}

fn has_columns(columns: Option<&Vec<ColumnFact>>) -> bool {
    columns.is_some_and(|columns| !columns.is_empty())
}

fn failed_analysis(lineage: LineageFacts, diagnostics: Vec<DiagnosticRow>) -> ModelAnalysis {
    ModelAnalysis {
        analysis_succeeded: false,
        columns: None,
        lineage,
        has_star: false,
        star_resolved: false,
        binding_diagnostics: diagnostics,
        binding_validated: true,
    }
}

/// Python's `analyze_deferred` result, answered natively.
fn legacy_outcome(
    legacy: LegacyAnalysis,
    native_lineage: Option<Vec<LineageRow>>,
    diagnostics: Vec<DiagnosticRow>,
) -> ModelAnalysis {
    if !legacy.succeeded {
        return ModelAnalysis {
            analysis_succeeded: false,
            columns: None,
            lineage: LineageFacts::NativeFacts(Vec::new()),
            has_star: false,
            star_resolved: false,
            binding_diagnostics: Vec::new(),
            binding_validated: false,
        };
    }
    ModelAnalysis {
        analysis_succeeded: true,
        columns: legacy.columns,
        lineage: match native_lineage {
            Some(rows) => LineageFacts::Native(rows),
            None => LineageFacts::NativeFacts(legacy.lineage),
        },
        has_star: legacy.has_star,
        star_resolved: false,
        binding_diagnostics: diagnostics,
        binding_validated: true,
    }
}

/// Python's merge of a re-analysis with known inputs into the model's analysis.
fn enriched_analysis(
    analysis: &ModelAnalysis,
    enriched: Enrichment,
    (star_pending, marked): (bool, bool),
) -> ModelAnalysis {
    let mut recovered: HashMap<String, String> = HashMap::new();
    for column in enriched
        .columns
        .iter()
        .flatten()
        .chain(analysis.columns.iter().flatten())
    {
        if let Some(data_type) = &column.data_type {
            recovered.insert(column.name.clone(), data_type.clone());
        }
    }
    let retyped: HashMap<String, String> = if marked && enriched.analysis_succeeded {
        enriched
            .columns
            .iter()
            .flatten()
            .filter_map(|column| Some((column.name.clone(), column.data_type.clone()?)))
            .collect()
    } else {
        HashMap::new()
    };
    let star_expanded: bool = star_pending && enriched.analysis_succeeded;
    let replaced: bool =
        (!has_columns(analysis.columns.as_ref()) || star_expanded) && enriched.analysis_succeeded;
    let mut merged: ModelAnalysis = if replaced {
        ModelAnalysis {
            analysis_succeeded: enriched.analysis_succeeded,
            columns: enriched.columns,
            lineage: LineageFacts::NativeFacts(enriched.lineage),
            has_star: enriched.has_star,
            star_resolved: false,
            binding_diagnostics: analysis.binding_diagnostics.clone(),
            binding_validated: analysis.binding_validated,
        }
    } else {
        analysis.clone()
    };
    let mut columns: Vec<ColumnFact> = merged.columns.take().unwrap_or_default();
    for column in &mut columns {
        if let Some(data_type) = retyped.get(&column.name) {
            column.data_type = Some(data_type.clone());
        } else if column.data_type.as_deref().is_none_or(str::is_empty) {
            column.data_type = recovered.get(&column.name).cloned();
        }
    }
    merged.columns = Some(columns);
    merged.star_resolved = merged.star_resolved || star_expanded;
    merged
}

/// A model's typed output columns and lineage from a successful native-lineage analysis.
fn session_model_facts(analysis: &ModelAnalysis) -> Option<SessionModelFacts> {
    let (LineageFacts::Native(rows) | LineageFacts::NativeFacts(rows)) = &analysis.lineage;
    analysis.analysis_succeeded.then(|| SessionModelFacts {
        columns: analysis.columns.clone(),
        lineage: rows.iter().map(output_sources).collect(),
    })
}

/// One lineage row as `(output column, [(resource name, column name)])`.
fn output_sources(row: &LineageRow) -> (String, Vec<(String, String)>) {
    let mut sources: Vec<(String, String)> = Vec::with_capacity(row.sources.len());
    for (_, name, column) in &row.sources {
        sources.push((name.clone(), column.clone()));
    }
    (row.output_column.clone(), sources)
}

/// The failure context naming one model.
fn model_context(model: &ModelRequest) -> String {
    format!("{SESSION_CONTEXT} of model '{}'", model.name)
}

/// A compile lineage column as the session's lineage row.
fn lineage_row(column: CompileLineageColumn) -> LineageRow {
    LineageRow {
        output_column: column.output_column,
        transform_code: match column.transform {
            LineageTransformKind::Direct => TRANSFORM_DIRECT,
            LineageTransformKind::Cast => TRANSFORM_CAST,
            LineageTransformKind::Expression => TRANSFORM_EXPRESSION,
            LineageTransformKind::Aggregation => TRANSFORM_AGGREGATION,
            LineageTransformKind::Star => TRANSFORM_STAR,
            LineageTransformKind::Constant => TRANSFORM_CONSTANT,
        },
        confidence_code: match column.confidence {
            LineageConfidence::High => CONFIDENCE_HIGH,
            LineageConfidence::Medium => CONFIDENCE_MEDIUM,
            LineageConfidence::Unknown => CONFIDENCE_UNKNOWN,
        },
        sources: column.sources,
    }
}

/// SQLQuack's type and nullability of one output column.
fn column_fact(column: &CompileLineageColumn) -> ColumnFact {
    ColumnFact {
        name: column.output_column.clone(),
        data_type: column.data_type.clone(),
        nullability: column.nullability.to_owned(),
    }
}

/// Whether polyglot reported an unknown column SQLQuack does not, so polyglot misread the query
/// and its output types stay withheld, as when the error stood.
fn dropped_unknown_columns(
    diagnostics: &[DiagnosticRow],
    resolved: &[(u8, DiagnosticRow)],
) -> bool {
    diagnostics.iter().any(|row| {
        row.code == UNKNOWN_COLUMN_CODE
            && row.severity == ERROR_SEVERITY
            && !resolved.iter().any(|(_, found)| {
                found.code == row.code && found.line == row.line && found.column == row.column
            })
    })
}

/// One wave model SQLQuack read: its position among the misses, cleaned SQL and lineage.
struct ReadModel {
    position: usize,
    cleaned_sql: String,
    lineage: CompiledLineage,
}

impl AnalysisSession {
    /// Analyse the cache misses SQLQuack reads; returns the positions it leaves to polyglot.
    fn sqlquack_analyses(
        &mut self,
        models: &[usize],
        misses: Vec<usize>,
        schemas: &[Shapes],
    ) -> Result<Vec<usize>, String> {
        if misses.is_empty() || !self.sqlquack_reads() {
            return Ok(misses);
        }
        let read: Vec<ReadModel> = self.read_models(models, &misses, schemas)?;
        if read.is_empty() {
            return Ok(misses);
        }
        if self.names_only() {
            for model in read {
                let index: usize = models[model.position];
                let resolved: Vec<(u8, DiagnosticRow)> =
                    self.resolved_findings(&model, &schemas[model.position])?;
                self.name_findings.insert(index, resolved);
            }
            return Ok(misses);
        }
        let requests: Vec<(&str, &Shapes)> = read
            .iter()
            .map(|model| (model.cleaned_sql.as_str(), &schemas[model.position]))
            .collect();
        let prepared: Vec<_> = self.catalog.prepare(&requests);
        let diagnostics: Vec<Vec<DiagnosticRow>> = self.catalog.native.binding_results(
            prepared,
            crate::semantic_validation::models::UnknownFunctions::Notes,
        )?;
        let answered: Vec<usize> = read.iter().map(|model| model.position).collect();
        for (model, diagnostics) in read.into_iter().zip(diagnostics) {
            let index: usize = models[model.position];
            let resolved: Vec<(u8, DiagnosticRow)> =
                self.resolved_findings(&model, &schemas[model.position])?;
            let diagnostics: Vec<DiagnosticRow> =
                with_resolution_rows(diagnostics, resolved, false);
            let inputs_known: bool = self.request.models[index]
                .required_names
                .iter()
                .all(|name| self.complete_shapes.contains(name));
            self.routed[index] = true;
            self.outcomes[index] = Some(ModelOutcome {
                sql_macro_findings: Vec::new(),
                analysis: read_analysis(model.lineage, diagnostics, inputs_known),
                cleaned_sql: model.cleaned_sql,
                deferred_cleaning: None,
                validated_schema: schemas[model.position].clone(),
                fused_binding_validated: true,
            });
        }
        Ok(misses
            .into_iter()
            .filter(|position| !answered.contains(position))
            .collect())
    }

    /// SQLQuack's findings for one model it read, as diagnostic rows with their phases.
    fn resolved_findings(
        &self,
        model: &ReadModel,
        schema: &Shapes,
    ) -> Result<Vec<(u8, DiagnosticRow)>, String> {
        let findings: Vec<FindingInput> = model
            .lineage
            .findings
            .iter()
            .map(|finding| {
                (
                    finding.code.clone(),
                    finding.message.clone(),
                    finding.location,
                    finding.written.clone(),
                    finding.phase,
                )
            })
            .collect();
        let exact_columns: bool = schema
            .iter()
            .any(|(_, columns)| columns.iter().any(|(name, _)| name.starts_with(QUOTE)));
        self.catalog
            .native
            .finding_rows(&model.cleaned_sql, exact_columns, &findings)
    }

    /// SQLQuack's name findings in place of polyglot's for the wave's models it read, in a
    /// names-only dialect, once polyglot's diagnostics for them are final.
    fn merge_name_findings(&mut self, models: &[usize]) -> Result<(), String> {
        for model in models {
            let Some(resolved) = self.name_findings.remove(model) else {
                continue;
            };
            let analysis: &mut ModelAnalysis = &mut self.outcome_mut(*model)?.analysis;
            let diagnostics: Vec<DiagnosticRow> = std::mem::take(&mut analysis.binding_diagnostics);
            if dropped_unknown_columns(&diagnostics, &resolved)
                && let Some(columns) = analysis.columns.as_mut()
            {
                for column in columns {
                    column.data_type = None;
                }
            }
            analysis.binding_diagnostics = with_resolution_rows(diagnostics, resolved, true);
        }
        Ok(())
    }

    /// Whether SQLQuack reports only names for this dialect, polyglot analysing the rest.
    fn names_only(&self) -> bool {
        SQLQUACK_NAMES_ONLY_DIALECTS.contains(&self.request.dialect.as_str())
    }

    /// The misses whose SQL cleans and SQLQuack reads into named columns, with their lineage.
    fn read_models(
        &mut self,
        models: &[usize],
        misses: &[usize],
        schemas: &[Shapes],
    ) -> Result<Vec<ReadModel>, String> {
        let mut required: Vec<String> = Vec::new();
        for position in misses {
            for reference in &self.request.models[models[*position]].references {
                if !required.contains(&reference.analysis_name) {
                    required.push(reference.analysis_name.clone());
                }
            }
        }
        self.catalog.prepare_analysis(
            &required,
            &self.available_types,
            &self.available_nullability,
        );
        let members: Vec<BatchMember<'_>> = misses
            .iter()
            .map(|position| {
                batch_member(&self.request.models[models[*position]], &schemas[*position])
            })
            .collect();
        let cleaned: Vec<Result<String, String>> = self
            .catalog
            .normalize_each(&self.request.dialect, &members)?;
        let clean: Vec<(usize, String)> = misses
            .iter()
            .zip(cleaned)
            .filter_map(|(position, sql)| match sql {
                Ok(sql) => Some((*position, sql)),
                Err(_uncleaned) => None,
            })
            .collect();
        let types: Vec<Shapes> = clean
            .iter()
            .map(|(position, _)| self.input_types(models[*position]))
            .collect();
        let nullability: Vec<Shapes> = clean
            .iter()
            .map(|(position, _)| self.input_nullability(models[*position]))
            .collect();
        let inputs: Vec<CompiledModelSql<'_>> = clean
            .iter()
            .zip(types.iter().zip(&nullability))
            .map(|((position, sql), (types, nullability))| CompiledModelSql {
                sql,
                references: &self.request.models[models[*position]].lineage_references,
                schema: &schemas[*position],
                types,
                nullability,
            })
            .collect();
        let pool: Arc<ThreadPool> = self.catalog.native.analysis_pool()?;
        let lineages: Vec<Option<CompiledLineage>> = compile_lineage(
            &self.request.dialect,
            &inputs,
            self.compile_functions(),
            &pool,
        )?
        .unwrap_or_default();
        Ok(clean
            .into_iter()
            .zip(lineages)
            .filter_map(|((position, cleaned_sql), lineage)| {
                Some(ReadModel {
                    position,
                    cleaned_sql,
                    lineage: lineage.filter(|lineage| !lineage.columns.is_empty())?,
                })
            })
            .collect())
    }

    /// Whether SQLQuack analyses this session's models: its dialect, without a project-local
    /// adapter's Python nullability rules, which only polyglot's analysis calls.
    fn sqlquack_reads(&self) -> bool {
        reads_dialect(&self.request.dialect) && self.request.nullability_callback.is_none()
    }

    /// The adapter's function facts, as SQLQuack reads them.
    fn compile_functions(&self) -> CompileFunctions<'_> {
        CompileFunctions {
            return_types: &self.request.function_return_types,
            nullability_rules: self.request.nullability_rules.as_deref().unwrap_or(&[]),
            quoted_ignore_case: self.catalog.native.quoted_ignore_case,
            known_functions: &self.catalog.native.options.known_functions,
            known_types: &self.catalog.native.options.known_types,
            duckdb_arities: self.catalog.native.dialect == polyglot_sql::DialectType::DuckDB,
        }
    }
}

/// SQLQuack's analysis with polyglot's diagnostics; a star is unresolved only over unknown inputs.
fn read_analysis(
    lineage: CompiledLineage,
    diagnostics: Vec<DiagnosticRow>,
    inputs_known: bool,
) -> ModelAnalysis {
    ModelAnalysis {
        analysis_succeeded: true,
        columns: Some(lineage.columns.iter().map(column_fact).collect()),
        has_star: lineage.projects_star,
        star_resolved: !(lineage.projects_star && lineage.open_star && !inputs_known),
        lineage: LineageFacts::Native(lineage.columns.into_iter().map(lineage_row).collect()),
        binding_diagnostics: diagnostics,
        binding_validated: true,
    }
}
