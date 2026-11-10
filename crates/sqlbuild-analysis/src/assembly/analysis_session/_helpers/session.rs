//! The session's phases: Python's uncached model analysis, wave by wave.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::ThreadPool;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::errors::StoreDecodeError;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_outcomes::native_outcomes;
use crate::analysis_route::models::{AnalysisRoute, ShareCounts};
use crate::analysis_route::types::NativeDialect;
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
    ShapeTable, declared_functions, dict_from_pairs, same_keys, same_relations, session_functions,
    session_schema,
};
use crate::assembly::analysis_session::_helpers::mappings::{producers, waves};
use crate::assembly::analysis_session::constants::{
    NATIVE_ANALYSIS_SHARED, NATIVE_ANALYSIS_UNSHARED, UNKNOWN_NULLABILITY, UNKNOWN_TYPE,
};
use crate::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use crate::assembly::analysis_session::models::{
    AnalysisSession, ColumnFact, ContractProof, DeferredCleaning, FinishedSession, LineageFacts,
    LineageRow, ModelAnalysis, ModelOutcome, ModelRequest, Phase, PivotOutcome, PivotTables,
    RelationTables, SessionFunctions, SessionModelFacts, SessionOutcome, SessionRequest,
    SessionSchema, SessionStep,
};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::main::normalize_native::normalize_native_sql;
use crate::semantic_validation::models::DiagnosticRow;
use crate::semantic_validation::models::ProjectCatalog;
use crate::sql_macros::main::check_sql_macro_arguments::check_sql_macro_arguments;
use crate::sql_macros::models::SqlMacroFinding;
use crate::sql_macros::types::ArgumentCheckInput;

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
        let model_names: Vec<String> = request
            .models
            .iter()
            .map(|model| model.name.clone())
            .collect();
        let declared_functions: Vec<String> = declared_functions(&request);
        let functions: SessionFunctions = session_functions(&request);
        let quoted_ignore_case: bool = request.quoted_ignore_case;
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
            model_names,
            declared_functions,
            functions,
            quoted_ignore_case,
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
        if let AnalysisRoute::Native(dialect) = self.route() {
            return self.analyze_native_wave(dialect, models, &misses, &schemas);
        }
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

    /// Each model of the wave the cache did not answer, analysed by its dialect's native module.
    fn analyze_native_wave(
        &mut self,
        dialect: &'static dyn NativeDialect,
        models: &[usize],
        misses: &[usize],
        schemas: &[Shapes],
    ) -> Result<(), String> {
        let pool = self.catalog.native.analysis_pool()?;
        let request: &SessionRequest = &self.request;
        let tables: (&ShapeTable, &ShapeTable) =
            (&self.available_types, &self.available_nullability);
        let declared: Vec<String> = declared_functions(request);
        let functions: SessionFunctions = session_functions(request);
        let relations_of = |schema: &Shapes| -> SessionSchema {
            session_schema(dialect, schema, tables, &functions)
        };
        let wave: Vec<(&ModelRequest, &Shapes)> = misses
            .iter()
            .map(|position| (&request.models[models[*position]], &schemas[*position]))
            .collect();
        let settings = (declared.as_slice(), request.quoted_ignore_case);
        let (outcomes, counts): (Vec<(ModelOutcome, Option<String>)>, ShareCounts) =
            native_outcomes(dialect, &pool, wave, (&relations_of, settings));
        for (metric, value) in [
            (NATIVE_ANALYSIS_SHARED, counts.shared),
            (NATIVE_ANALYSIS_UNSHARED, counts.unshared),
        ] {
            record_compile_metric(metric, i64::try_from(value).unwrap_or(i64::MAX));
        }
        for (position, (outcome, failure)) in misses.iter().zip(outcomes) {
            self.failures.extend(failure);
            self.outcomes[models[*position]] = Some(outcome);
        }
        self.check_sql_macros(models)
    }

    /// The route of the session's dialect: a native module, or Polyglot.
    pub(crate) fn route(&self) -> AnalysisRoute {
        analysis_route(Some(&self.request.dialect))
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
        if self.cyclic || matches!(self.route(), AnalysisRoute::Native(_)) {
            return Ok(());
        }
        let mut candidates: Vec<(usize, Shapes, bool)> = Vec::new();
        for model in self.waves[wave].clone() {
            if self.cached(model) {
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
            let rows: Vec<Vec<DiagnosticRow>> = self.catalog.native.binding_results(prepared)?;
            for ((model, _, _), diagnostics) in validations.iter().zip(rows) {
                let analysis: &mut ModelAnalysis = &mut self.outcome_mut(*model)?.analysis;
                analysis.binding_diagnostics = diagnostics;
                analysis.binding_validated = true;
            }
        }
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

    /// Answer `models` from the cache where it can; returns the positions it cannot answer.
    fn read_cache(&mut self, models: &[usize], schemas: &[Shapes]) -> Result<Vec<usize>, String> {
        let Some(mut cache) = self.cache.take() else {
            return Ok((0..models.len()).collect());
        };
        if matches!(self.route(), AnalysisRoute::Legacy) {
            self.prepare_catalog(models, schemas);
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

    /// Load the relations and schemas a Polyglot analysis of `models` reads into its catalog.
    fn prepare_catalog(&mut self, models: &[usize], schemas: &[Shapes]) {
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
        if let AnalysisRoute::Native(dialect) = self.route() {
            for model in pending {
                let request: &ModelRequest = &self.request.models[model];
                let sql: String =
                    normalize_native_sql(&request.query_sql, dialect.name(), dialect.stub_prefix())
                        .map(|normalised| placeholder_defaults(&normalised, &request.placeholders))
                        .unwrap_or_default();
                self.outcome_mut(model)?.cleaned_sql = sql;
                self.pending_cleaned[model] = false;
            }
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
