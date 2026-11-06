//! Coarse, bounded native batch boundary for compact SQL query analysis.

use polyglot_sql::ExpressionWalk;
use polyglot_sql::{
    AnalyzeQueryOptions, DialectType, ProjectionNullability, QueryAnalysis, QueryShape,
    ReferenceConfidence, SchemaValidationOptions, TransformKind, ValidationResult,
    ValidationSchema, analyze_query, analyze_query_for_project_projections,
};
#[cfg(test)]
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::constants::{UNKNOWN_SQL_TYPE, VARCHAR_SQL_TYPE};
use sqlbuild_core::constants::SQL_WILDCARD;

const DEFAULT_WORKERS: usize = 4;
const MAX_WORKERS: usize = 4;
const ANALYSIS_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
const COMPACT_ANALYSIS_BATCH_SIZE: usize = 64;
const LARGE_COMPACT_SQL_BYTES: usize = 32 * 1024 * 1024;

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct AnalysisBatchRequest {
    requests: Vec<AnalysisRequest>,
    #[serde(default = "default_workers")]
    workers: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AnalysisRequest {
    sql: String,
    #[serde(default = "default_dialect")]
    dialect: String,
    #[serde(default)]
    schema: Option<ValidationSchema>,
    #[serde(default)]
    binding_schema: Option<ValidationSchema>,
    #[serde(default)]
    binding_options: Option<BindingOptions>,
    #[serde(default)]
    binding_references: Option<Vec<(String, bool)>>,
    #[serde(default)]
    binding_override: Option<usize>,
    /// Relation stubs in `sql` that rename the catalog relations of the binding schema.
    #[serde(default)]
    binding_aliases: Option<HashMap<String, String>>,
    #[serde(default)]
    analysis_references: Option<Vec<(String, String)>>,
    #[serde(default)]
    sqlbuild_diagnostics: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BindingOptions {
    #[serde(default)]
    check_types: bool,
    #[serde(default)]
    semantic: bool,
    #[serde(default)]
    known_functions: Vec<String>,
    #[serde(default)]
    known_types: Vec<String>,
}

impl AnalysisRequest {
    fn validation_options(&self) -> SchemaValidationOptions {
        SchemaValidationOptions {
            check_types: self
                .binding_options
                .as_ref()
                .is_some_and(|options| options.check_types),
            semantic: self
                .binding_options
                .as_ref()
                .is_some_and(|options| options.semantic),
            known_functions: self
                .binding_options
                .as_ref()
                .map_or_else(Vec::new, |options| options.known_functions.clone()),
            known_types: self
                .binding_options
                .as_ref()
                .map_or_else(Vec::new, |options| options.known_types.clone()),
            check_references: true,
            strict: Some(true),
            strict_syntax: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnalysisResponse {
    Success { analysis: QueryAnalysis },
    Failure { error: String },
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct ProjectAnalysisBatchRequest {
    requests: Vec<ProjectAnalysisRequest>,
    #[serde(default = "default_workers")]
    workers: usize,
}

#[derive(Debug, Deserialize)]
struct CompactProjectAnalysisBatchRequest {
    queries: Vec<AnalysisRequest>,
    templates: Vec<CompactProjectAnalysisTemplateRequest>,
    projections: Vec<CompactProjectAnalysisProjectionRequest>,
    #[serde(default = "default_workers")]
    workers: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactProjectAnalysisTemplateRequest {
    query_index: usize,
    #[serde(default)]
    references: HashMap<String, LineageResource>,
    #[serde(default)]
    function_return_types: HashMap<String, String>,
    #[serde(default)]
    declared_column_order: Vec<String>,
    #[serde(default)]
    recover_cte_facts: bool,
    #[serde(default = "default_true")]
    rich_type_inference: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactProjectAnalysisProjectionRequest {
    template_index: usize,
    #[serde(default)]
    resource_names: HashMap<String, String>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAnalysisRequest {
    #[serde(flatten)]
    query: AnalysisRequest,
    #[serde(default)]
    references: HashMap<String, LineageResource>,
    #[serde(default)]
    function_return_types: HashMap<String, String>,
    #[serde(default)]
    declared_column_order: Vec<String>,
    #[serde(default)]
    recover_cte_facts: bool,
    #[serde(default = "default_true")]
    rich_type_inference: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LineageResource {
    resource_type: String,
    resource_name: String,
}

#[cfg(test)]
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ProjectAnalysisResponse {
    Success { analysis: ProjectAnalysis },
    Failure { error: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAnalysis {
    columns: Vec<ProjectColumn>,
    lineage_columns: Vec<ProjectLineageColumn>,
    has_star: bool,
    #[serde(skip)]
    requires_legacy_fallback: bool,
    #[serde(skip)]
    preserve_fallback_lineage: bool,
}

struct ProjectAnalysisInputs<'a> {
    analysis: &'a QueryAnalysis,
    references: &'a HashMap<String, LineageResource>,
    function_return_types: &'a HashMap<String, String>,
    declared_column_order: &'a [String],
    recover_cte_facts: bool,
    rich_type_inference: bool,
}

struct ProjectAnalysisProjection {
    references: HashMap<String, LineageResource>,
    function_return_types: HashMap<String, String>,
    declared_column_order: Vec<String>,
    recover_cte_facts: bool,
    rich_type_inference: bool,
}

struct CompactQueryWork {
    query: AnalysisRequest,
    project_projections: bool,
    projections: Vec<(usize, ProjectAnalysisProjection)>,
}

struct CompiledQueryWorkResult {
    projections: Vec<(usize, Result<ProjectAnalysis, String>)>,
    validation: Option<Result<ValidationResult, String>>,
}

struct ProjectAnalysisProjectionResult {
    projection_index: usize,
    resource_names: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
struct ProjectColumn {
    name: String,
    #[serde(rename = "type")]
    data_type: Option<String>,
    nullability: ProjectNullability,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectNullability {
    NonNull,
    Nullable,
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectLineageColumn {
    output_column: String,
    upstream_columns: Vec<ProjectLineageSource>,
    transform_kind: ProjectTransformKind,
    confidence: ProjectConfidence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectLineageSource {
    resource_type: String,
    resource_name: String,
    column_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectTransformKind {
    Direct,
    Cast,
    Expression,
    Aggregation,
    Star,
    Constant,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectConfidence {
    High,
    Medium,
    Unknown,
}

fn default_workers() -> usize {
    DEFAULT_WORKERS
}

fn default_dialect() -> String {
    "generic".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
pub(crate) fn analyze_json(request_json: &str) -> Result<String, String> {
    let request: AnalysisBatchRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let workers = request.workers.clamp(1, MAX_WORKERS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.min(request.requests.len().max(1)))
        .stack_size(ANALYSIS_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let responses: Vec<AnalysisResponse> = pool.install(|| {
        request
            .requests
            .into_par_iter()
            .map(analyze_request)
            .collect()
    });
    serde_json::to_string(&responses).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn analyze_project_json(request_json: &str) -> Result<String, String> {
    let request: ProjectAnalysisBatchRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let workers = request.workers.clamp(1, MAX_WORKERS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.min(request.requests.len().max(1)))
        .stack_size(ANALYSIS_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let responses: Vec<ProjectAnalysisResponse> = pool.install(|| {
        request
            .requests
            .into_par_iter()
            .map(analyze_project_request)
            .collect()
    });
    serde_json::to_string(&responses).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn analyze_project_compact_json(request_json: &str) -> Result<String, String> {
    analyze_project_compact_with_catalog(request_json, None)
}

#[cfg(test)]
pub(crate) fn analyze_project_compact_with_catalog(
    request_json: &str,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> Result<String, String> {
    analyze_resolved_compact_batch(resolve_compact_batch(request_json, catalog)?, catalog)
}

/// Resolve catalog schemas now and return the analysis to run later without catalog tables.
pub(crate) fn prepare_project_compact_with_catalog(
    request_json: &str,
    catalog: &crate::semantic_validation::models::ProjectCatalog,
) -> Result<crate::semantic_validation::types::PreparedCompactAnalysis, String> {
    let request = resolve_compact_batch(request_json, Some(catalog))?;
    Ok(Box::new(move |view| {
        analyze_resolved_compact_batch(request, Some(view))
    }))
}

fn resolve_compact_batch(
    request_json: &str,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> Result<CompactProjectAnalysisBatchRequest, String> {
    let mut request: CompactProjectAnalysisBatchRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    for query in &mut request.queries {
        query.sqlbuild_diagnostics = catalog.is_some();
        if let Some(references) = &query.analysis_references {
            query.schema = catalog
                .ok_or("analysis references require a native project catalog")?
                .analysis_schema(references);
        }
        if let Some(references) = &query.binding_references {
            let catalog = catalog.ok_or("binding references require a native project catalog")?;
            let mut schema = catalog.reference_schema(references, query.binding_override)?;
            if let Some(aliases) = &query.binding_aliases {
                catalog.alias_reference_schema(&mut schema, references, aliases);
            }
            query.binding_schema = Some(schema);
            if !catalog.quoted_ignore_case || !query.sql.contains('"') {
                query.binding_options = Some(BindingOptions {
                    check_types: catalog.options.check_types,
                    semantic: catalog.options.semantic,
                    known_functions: catalog.options.known_functions.clone(),
                    known_types: catalog.options.known_types.clone(),
                });
            }
        }
    }
    Ok(request)
}

/// Analyse a resolved batch; a catalog supplies options, probes, and its compile-wide pool.
fn analyze_resolved_compact_batch(
    request: CompactProjectAnalysisBatchRequest,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> Result<String, String> {
    let workers = request.workers.clamp(1, MAX_WORKERS);
    if request
        .templates
        .iter()
        .any(|template| template.query_index >= request.queries.len())
    {
        return Err("compact project analysis received an invalid query index".to_string());
    }
    if request
        .projections
        .iter()
        .any(|projection| projection.template_index >= request.templates.len())
    {
        return Err("compact project analysis received an invalid template index".to_string());
    }
    let unique_query_count = request.queries.len();
    let unique_projection_count = request.templates.len();
    let mut project_projection_queries = vec![false; unique_query_count];
    for template in &request.templates {
        if template.recover_cte_facts {
            project_projection_queries[template.query_index] = true;
        }
    }
    let mut projections_by_query: Vec<Vec<(usize, ProjectAnalysisProjection)>> =
        (0..unique_query_count).map(|_| Vec::new()).collect();
    for (projection_index, template) in request.templates.into_iter().enumerate() {
        projections_by_query[template.query_index].push((
            projection_index,
            ProjectAnalysisProjection {
                references: template.references,
                function_return_types: template.function_return_types,
                declared_column_order: template.declared_column_order,
                recover_cte_facts: template.recover_cte_facts,
                rich_type_inference: template.rich_type_inference,
            },
        ));
    }
    let projection_results: Vec<ProjectAnalysisProjectionResult> = request
        .projections
        .into_iter()
        .map(|projection| {
            let mut resource_names: Vec<(String, String)> =
                projection.resource_names.into_iter().collect();
            resource_names.sort();
            ProjectAnalysisProjectionResult {
                projection_index: projection.template_index,
                resource_names,
            }
        })
        .collect();
    let sql_bytes: usize = request.queries.iter().map(|query| query.sql.len()).sum();
    let pool = match catalog {
        Some(catalog) => catalog.analysis_pool()?,
        None => std::sync::Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers.min(unique_query_count.max(1)))
                .stack_size(ANALYSIS_WORKER_STACK_BYTES)
                .build()
                .map_err(|error| error.to_string())?,
        ),
    };
    let query_work: Vec<CompactQueryWork> = request
        .queries
        .into_iter()
        .zip(project_projection_queries)
        .zip(projections_by_query)
        .map(
            |((query, project_projections), projections)| CompactQueryWork {
                query,
                project_projections,
                projections,
            },
        )
        .collect();
    let mut accumulator = CompactProjectAccumulator::new(unique_projection_count);
    let mut validations = Vec::with_capacity(unique_query_count);
    let batch_size = if sql_bytes >= LARGE_COMPACT_SQL_BYTES {
        COMPACT_ANALYSIS_BATCH_SIZE
    } else {
        unique_query_count.max(1)
    };
    let mut remaining = query_work.into_iter();
    loop {
        let batch: Vec<_> = remaining.by_ref().take(batch_size).collect();
        if batch.is_empty() {
            break;
        }
        for group in analyze_largest_first(&pool, batch, catalog) {
            validations.push(group.validation.transpose()?);
            for (projection_index, analysis) in group.projections {
                accumulator.compact_analysis(projection_index, analysis)?;
            }
        }
    }
    let mut response = accumulator.finish(
        projection_results,
        unique_query_count,
        unique_projection_count,
    )?;
    if validations.iter().any(Option::is_some) {
        response.validations = Some(validations);
    }
    serde_json::to_string(&response).map_err(|error| error.to_string())
}

/// Analyse largest SQL first on spawned drainers rather than a broadcast to every pool thread.
fn analyze_largest_first(
    pool: &rayon::ThreadPool,
    batch: Vec<CompactQueryWork>,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> Vec<CompiledQueryWorkResult> {
    let mut order: Vec<usize> = (0..batch.len()).collect();
    order.sort_by_key(|index| std::cmp::Reverse(batch[*index].query.sql.len()));
    let slots: Vec<Mutex<Option<CompactQueryWork>>> = batch
        .into_iter()
        .map(|work| Mutex::new(Some(work)))
        .collect();
    let next = AtomicUsize::new(0);
    let finished: Mutex<Vec<(usize, CompiledQueryWorkResult)>> = Mutex::new(Vec::new());
    let drainers = pool.current_num_threads().min(slots.len());
    pool.scope(|scope| {
        for _ in 0..drainers {
            scope.spawn(|_| {
                let mut drained: Vec<(usize, CompiledQueryWorkResult)> = Vec::new();
                while let Some(index) = order.get(next.fetch_add(1, Ordering::Relaxed)).copied() {
                    let work = slots[index]
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take();
                    if let Some(work) = work {
                        drained.push((index, analyze_compact_query_work(work, catalog)));
                    }
                }
                finished
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend(drained);
            });
        }
    });
    let mut results: Vec<Option<CompiledQueryWorkResult>> = slots.iter().map(|_| None).collect();
    for (index, result) in finished
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
    {
        results[index] = Some(result);
    }
    results.into_iter().flatten().collect()
}

fn analyze_compact_query_work(
    work: CompactQueryWork,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> CompiledQueryWorkResult {
    let diagnostic_sql = work
        .query
        .sqlbuild_diagnostics
        .then(|| work.query.sql.clone());
    let dialect = work.query.dialect.parse::<DialectType>();
    let mut result = analyze_compact_query_work_inner(work, catalog);
    if let (Some(sql), Ok(dialect)) = (diagnostic_sql, dialect)
        && let Some(validation) = result.validation.take()
    {
        let local_probes = crate::semantic_validation::models::FunctionProbes::default();
        let probes = catalog.map_or(&local_probes, |catalog| catalog.function_probes.as_ref());
        result.validation = Some(validation.and_then(|value| {
            crate::semantic_validation::main::map_diagnostics::map_diagnostics(
                &sql, dialect, value, probes,
            )
        }));
    }
    result
}

fn analyze_compact_query_work_inner(
    work: CompactQueryWork,
    catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> CompiledQueryWorkResult {
    let exact_catalog = catalog.filter(|catalog| {
        work.query
            .binding_schema
            .as_ref()
            .is_some_and(|schema| catalog.needs_identifier_encoding(&work.query.sql, schema))
    });
    let work = match try_borrowed_query(work, exact_catalog) {
        Ok(result) => return result,
        Err(work) => *work,
    };
    let exact_schema = exact_catalog.and_then(|_| work.query.binding_schema.clone());
    let sql = exact_catalog.map(|_| work.query.sql.clone());
    let (query_result, mut validation, expression) =
        compile_query(work.query, work.project_projections);
    if let (Some(catalog), Some(schema), Some(sql)) = (exact_catalog, exact_schema, sql) {
        validation = Some(match expression {
            Some(expression) => catalog.expression_validation(&sql, &schema, expression),
            None => catalog.validate(&sql, &schema),
        });
    }
    CompiledQueryWorkResult {
        projections: project_query_templates(&query_result, work.projections),
        validation,
    }
}

/// Fold fully bound lexical query graphs; unsupported shapes retain the existing resolver.
fn try_borrowed_query(
    mut work: CompactQueryWork,
    exact_catalog: Option<&crate::semantic_validation::models::ProjectCatalog>,
) -> Result<CompiledQueryWorkResult, Box<CompactQueryWork>> {
    if work.query.schema.is_none()
        || (work.query.binding_schema.is_none()
            && !work
                .query
                .schema
                .as_ref()
                .is_some_and(|schema| schema.strict == Some(true)))
        || work
            .projections
            .iter()
            .any(|(_, projection)| !projection.rich_type_inference && !projection.recover_cte_facts)
    {
        return Err(Box::new(work));
    }
    let Ok(dialect) = work.query.dialect.parse::<DialectType>() else {
        return Err(Box::new(work));
    };
    if !matches!(dialect, DialectType::Snowflake | DialectType::DuckDB) {
        return Err(Box::new(work));
    }
    let Ok(mut statements) = polyglot_sql::parse_with_options(
        &work.query.sql,
        dialect,
        &polyglot_sql::ParseOptions {
            complexity_guard: Some(polyglot_sql::ComplexityGuardOptions {
                max_function_call_depth: Some(128),
                ..Default::default()
            }),
        },
    ) else {
        return Err(Box::new(work));
    };
    if statements.len() != 1 {
        return Err(Box::new(work));
    }
    let Some(mut expression) = statements.pop() else {
        return Err(Box::new(work));
    };
    if dialect == DialectType::Snowflake && expression.dfs().any(has_case_sensitive_binding) {
        return Err(Box::new(work));
    }
    if matches!(&expression, polyglot_sql::Expression::Select(select)
        if select.with.is_none()
            && select.expressions.iter().any(|projection| super::borrowed_facts::projection_star(projection).is_some()))
    {
        if matches!(&expression, polyglot_sql::Expression::Select(select)
            if select.from.as_ref().is_some_and(|from| matches!(from.expressions.as_slice(), [polyglot_sql::Expression::Subquery(_)])))
        {
            work.project_projections = false;
        }
        return Err(Box::new(work));
    }
    if !matches!(
        expression,
        polyglot_sql::Expression::Select(_)
            | polyglot_sql::Expression::Union(_)
            | polyglot_sql::Expression::Intersect(_)
            | polyglot_sql::Expression::Except(_)
    ) {
        return Err(Box::new(work));
    }
    if expression.dfs().any(|node| {
        matches!(
            node,
            polyglot_sql::Expression::Pivot(_) | polyglot_sql::Expression::Unpivot(_)
                | polyglot_sql::Expression::Intersect(_) | polyglot_sql::Expression::Except(_)
        )
        || matches!(node, polyglot_sql::Expression::Table(table) if table.schema.is_some() || table.catalog.is_some())
        || matches!(node, polyglot_sql::Expression::Select(select) if select.with.as_ref().is_some_and(|with| with.recursive))
        || matches!(node, polyglot_sql::Expression::Union(union) if union.with.as_ref().is_some_and(|with| with.recursive))
        || matches!(node, polyglot_sql::Expression::Select(select) if !borrowed_sources_supported(select, dialect))
        || matches!(node, polyglot_sql::Expression::Select(select)
            if select.expressions.iter().any(|projection| projection.dfs().any(|child| matches!(child,
                polyglot_sql::Expression::Select(_) | polyglot_sql::Expression::Union(_)
                | polyglot_sql::Expression::Intersect(_) | polyglot_sql::Expression::Except(_)))))
    }) {
        return Err(Box::new(work));
    }
    let bound_facts = work
        .query
        .schema
        .as_ref()
        .filter(|schema| {
            plain_binding_schema(schema)
                && work.query.binding_schema.as_ref().is_none_or(|binding| {
                    plain_binding_schema(binding) && same_binding_columns(schema, binding)
                })
        })
        .and_then(|schema| super::borrowed_facts::infer_bound(&expression, Some(schema), dialect));
    let catalog_expression = exact_catalog.map(|_| expression.clone());
    let needs_validation = bound_facts.is_none() || work.query.binding_options.is_some();
    let mut validation_expression = None;
    let unannotated_outputs: Option<Vec<_>> = work
        .projections
        .iter()
        .map(|(_, projection)| {
            super::compatibility_types::infer_unannotated_outputs(
                &expression,
                work.query.schema.as_ref(),
                &projection.function_return_types,
                dialect,
            )
        })
        .collect();
    if unannotated_outputs.is_none() {
        if needs_validation {
            validation_expression = Some(expression.clone());
        }
        let schema = work.query.schema.as_ref().map(|schema| {
            polyglot_sql::validation::mapping_schema_from_validation_schema_with_dialect(
                schema, dialect,
            )
        });
        let schema = schema
            .as_ref()
            .map(|schema| schema as &dyn polyglot_sql::schema::Schema);
        polyglot_sql::optimizer::annotate_types::annotate_types(
            &mut expression,
            schema,
            Some(dialect),
        );
    }
    let facts = bound_facts.unwrap_or_else(|| {
        super::borrowed_facts::infer(&expression, work.query.schema.as_ref(), dialect)
    });
    if facts.is_empty()
        || facts
            .iter()
            .any(|fact| !fact.resolved || fact.name.is_empty() || fact.name == SQL_WILDCARD)
    {
        return Err(Box::new(work));
    }
    let mut projections: Vec<(usize, Result<ProjectAnalysis, String>)> = Vec::new();
    let mut facts_by_name: HashMap<&str, &super::borrowed_facts::OutputFact> = HashMap::new();
    for fact in &facts {
        if facts_by_name.insert(&fact.name, fact).is_some() {
            return Err(Box::new(work));
        }
    }
    let mut unannotated_outputs = unannotated_outputs.unwrap_or_default().into_iter();
    for (index, projection) in &work.projections {
        let mut outputs = unannotated_outputs.next().unwrap_or_else(|| {
            super::compatibility_types::infer_outputs(
                &expression,
                work.query.schema.as_ref(),
                &projection.function_return_types,
                dialect,
            )
        });
        {
            let types: HashMap<_, _> = outputs.into_iter().collect();
            outputs = facts
                .iter()
                .map(|fact| (fact.name.clone(), types.get(&fact.name).cloned().flatten()))
                .collect();
        }
        let columns: Vec<_> = outputs
            .into_iter()
            .map(|(name, data_type)| ProjectColumn {
                nullability: facts_by_name
                    .get(name.as_str())
                    .map_or(ProjectNullability::Unknown, |fact| {
                        project_nullability(fact.nullability)
                    }),
                name,
                data_type: known_compatibility_type(data_type, projection.rich_type_inference),
            })
            .collect();
        let mut lineage_columns: Vec<ProjectLineageColumn> = Vec::new();
        for column in &columns {
            let fact = facts_by_name.get(column.name.as_str());
            let mut upstream_columns: Vec<ProjectLineageSource> = Vec::new();
            if let Some(fact) = fact {
                for (table, source_column) in fact.upstream.iter() {
                    if let Some(resource) = projection.references.get(table) {
                        upstream_columns.push(ProjectLineageSource {
                            resource_type: resource.resource_type.clone(),
                            resource_name: resource.resource_name.clone(),
                            column_name: source_column.clone(),
                        });
                    }
                }
            }
            let upstream_columns = order_project_sources(upstream_columns);
            lineage_columns.push(ProjectLineageColumn {
                output_column: column.name.clone(),
                transform_kind: fact.map_or(ProjectTransformKind::Direct, |fact| {
                    project_transform_kind(fact.transform, !upstream_columns.is_empty())
                }),
                confidence: fact.map_or(ProjectConfidence::Unknown, |fact| {
                    if fact.resolved {
                        ProjectConfidence::High
                    } else {
                        ProjectConfidence::Unknown
                    }
                }),
                upstream_columns,
            });
        }
        let requires_legacy_fallback = columns.iter().any(|column| column.data_type.is_none());
        projections.push((
            *index,
            Ok(ProjectAnalysis {
                columns,
                lineage_columns,
                has_star: matches!(&expression, polyglot_sql::Expression::Select(select)
                    if select.expressions.iter().any(|projection| super::borrowed_facts::projection_star(projection).is_some())),
                requires_legacy_fallback,
                preserve_fallback_lineage: true,
            }),
        ));
    }
    let mut validation = work
        .query
        .binding_schema
        .as_ref()
        .or(work.query.schema.as_ref())
        .map(|schema| {
            if !needs_validation {
                return Ok(ValidationResult::with_errors(Vec::new()));
            }
            let retain_expression =
                crate::semantic_validation::main::may_have_extra_clause_checks(&work.query.sql);
            let validation_expression = validation_expression.take().unwrap_or_else(|| {
                if retain_expression {
                    expression.clone()
                } else {
                    std::mem::replace(
                        &mut expression,
                        polyglot_sql::Expression::Null(polyglot_sql::expressions::Null),
                    )
                }
            });
            let result = polyglot_sql::validation::validate_parsed_with_schema(
                vec![validation_expression],
                dialect,
                schema,
                &work.query.validation_options(),
            );
            crate::semantic_validation::main::complete_parsed_validation(
                crate::semantic_validation::main::ParsedValidationRequest {
                    sql: &work.query.sql,
                    dialect,
                    schema,
                    result,
                    expression: retain_expression.then_some(&expression),
                },
            )
        });
    if !matches!(validation, Some(Ok(ref result)) if result.valid) {
        return Err(Box::new(work));
    }
    if let (Some(catalog), Some(expression), Some(schema)) = (
        exact_catalog,
        catalog_expression,
        work.query.binding_schema.as_ref(),
    ) {
        validation = Some(catalog.expression_validation(&work.query.sql, schema, expression));
    }
    Ok(CompiledQueryWorkResult {
        projections,
        validation: if work.query.binding_schema.is_some() {
            validation
        } else {
            None
        },
    })
}

fn same_binding_columns(left: &ValidationSchema, right: &ValidationSchema) -> bool {
    if left.tables.len() != right.tables.len() {
        return false;
    }
    for table in &left.tables {
        let Some(other) = right
            .tables
            .iter()
            .find(|other| other.name.eq_ignore_ascii_case(&table.name))
        else {
            return false;
        };
        if table.columns.len() != other.columns.len()
            || !table
                .columns
                .iter()
                .zip(&other.columns)
                .all(|(left, right)| left.name.eq_ignore_ascii_case(&right.name))
        {
            return false;
        }
    }
    true
}

fn plain_binding_schema(schema: &ValidationSchema) -> bool {
    for table in &schema.tables {
        if table.schema.is_some()
            || !table.aliases.is_empty()
            || !table.foreign_keys.is_empty()
            || !table.primary_key.is_empty()
            || !table.unique_keys.is_empty()
            || table
                .columns
                .iter()
                .any(|column| column.references.is_some() || column.primary_key || column.unique)
        {
            return false;
        }
    }
    true
}

fn has_case_sensitive_binding(expression: &polyglot_sql::Expression) -> bool {
    use polyglot_sql::Expression;
    match expression {
        Expression::Column(column) => {
            case_sensitive_identifier(&column.name)
                || column.table.as_ref().is_some_and(case_sensitive_identifier)
        }
        Expression::Identifier(identifier) => case_sensitive_identifier(identifier),
        Expression::Alias(alias) => {
            case_sensitive_identifier(&alias.alias)
                || alias.column_aliases.iter().any(case_sensitive_identifier)
        }
        Expression::Table(table) => {
            case_sensitive_identifier(&table.name)
                || table.alias.as_ref().is_some_and(case_sensitive_identifier)
                || table.column_aliases.iter().any(case_sensitive_identifier)
        }
        Expression::Cte(cte) => {
            case_sensitive_identifier(&cte.alias)
                || cte.columns.iter().any(case_sensitive_identifier)
        }
        Expression::Subquery(query) => {
            query.alias.as_ref().is_some_and(case_sensitive_identifier)
                || query.column_aliases.iter().any(case_sensitive_identifier)
        }
        Expression::Select(select) => select
            .with
            .as_ref()
            .is_some_and(case_sensitive_cte_bindings),
        Expression::Union(union) => union.with.as_ref().is_some_and(case_sensitive_cte_bindings),
        _ => false,
    }
}

fn case_sensitive_identifier(identifier: &polyglot_sql::expressions::Identifier) -> bool {
    identifier.quoted && identifier.name != identifier.name.to_uppercase()
}

fn case_sensitive_cte_bindings(with: &polyglot_sql::expressions::With) -> bool {
    with.ctes.iter().any(|cte| {
        case_sensitive_identifier(&cte.alias) || cte.columns.iter().any(case_sensitive_identifier)
    })
}

fn borrowed_sources_supported(
    select: &polyglot_sql::expressions::Select,
    dialect: DialectType,
) -> bool {
    use polyglot_sql::expressions::JoinKind;
    if select.joins.iter().any(|join| !join.using.is_empty())
        && select
            .expressions
            .iter()
            .any(|projection| super::borrowed_facts::projection_star(projection).is_some())
    {
        return false;
    }
    select
        .from
        .iter()
        .flat_map(|from| &from.expressions)
        .all(|source| borrowed_source_supported(source, dialect))
        && select.joins.iter().all(|join| {
            matches!(
                join.kind,
                JoinKind::Inner
                    | JoinKind::Left
                    | JoinKind::Right
                    | JoinKind::Full
                    | JoinKind::Cross
                    | JoinKind::Implicit
                    | JoinKind::AsOf
                    | JoinKind::AsOfLeft
                    | JoinKind::AsOfRight
                    | JoinKind::Lateral
                    | JoinKind::LeftLateral
            ) && borrowed_source_supported(&join.this, dialect)
        })
}

fn borrowed_source_supported(expression: &polyglot_sql::Expression, dialect: DialectType) -> bool {
    use polyglot_sql::Expression;
    match expression {
        Expression::Table(_) => true,
        Expression::Values(values) => {
            dialect == DialectType::Snowflake || !values.column_aliases.is_empty()
        }
        Expression::Subquery(query) => query.alias.is_some(),
        Expression::Lateral(lateral) => {
            lateral.alias.is_some()
                && matches!(lateral.this.as_ref(), Expression::Function(function) if !function.quoted && function.name.eq_ignore_ascii_case("FLATTEN"))
        }
        Expression::Paren(paren) => borrowed_source_supported(&paren.this, dialect),
        Expression::Annotated(annotation) => borrowed_source_supported(&annotation.this, dialect),
        _ => false,
    }
}

fn known_compatibility_type(data_type: Option<String>, normalize: bool) -> Option<String> {
    data_type
        .filter(|value| {
            !value.is_empty()
                && value != UNKNOWN_SQL_TYPE
                && value != super::compatibility_types::NULL_TYPE
        })
        .map(|value| {
            if normalize {
                normalize_project_type(&value)
            } else {
                value
            }
        })
}

type CompiledQueryResult = (
    Result<QueryAnalysis, String>,
    Option<Result<ValidationResult, String>>,
    Option<polyglot_sql::Expression>,
);

fn compile_query(mut request: AnalysisRequest, project_projections: bool) -> CompiledQueryResult {
    let validation_options = request.validation_options();
    let Some(schema) = request.binding_schema.take() else {
        return (query_analysis(request, project_projections), None, None);
    };
    let dialect: DialectType = match request.dialect.parse() {
        Ok(dialect) => dialect,
        Err(_) => return (query_analysis(request, project_projections), None, None),
    };
    let compiled = polyglot_sql::compile_required_query_analysis(
        &request.sql,
        AnalyzeQueryOptions {
            complexity_guard: None,
            dialect,
            schema: request.schema,
        },
        &schema,
        &validation_options,
        project_projections,
    );
    let validation = crate::semantic_validation::main::complete_parsed_validation(
        crate::semantic_validation::main::ParsedValidationRequest {
            sql: &request.sql,
            dialect,
            schema: &schema,
            result: compiled.validation,
            expression: compiled.expression.as_ref(),
        },
    );
    (
        compiled.analysis.map_err(|error| error.to_string()),
        Some(validation),
        compiled.expression,
    )
}

fn project_query_templates(
    query_result: &Result<QueryAnalysis, String>,
    projections: Vec<(usize, ProjectAnalysisProjection)>,
) -> Vec<(usize, Result<ProjectAnalysis, String>)> {
    let mut results: Vec<(usize, Result<ProjectAnalysis, String>)> =
        Vec::with_capacity(projections.len());
    for (projection_index, projection) in projections {
        let projected = match query_result {
            Ok(analysis) => Ok(project_analysis(ProjectAnalysisInputs {
                analysis,
                references: &projection.references,
                function_return_types: &projection.function_return_types,
                declared_column_order: &projection.declared_column_order,
                recover_cte_facts: projection.recover_cte_facts,
                rich_type_inference: projection.rich_type_inference,
            })),
            Err(error) => Err(error.clone()),
        };
        results.push((projection_index, projected));
    }
    results
}

#[cfg(test)]
fn analyze_request(request: AnalysisRequest) -> AnalysisResponse {
    match query_analysis(request, false) {
        Ok(analysis) => AnalysisResponse::Success { analysis },
        Err(error) => AnalysisResponse::Failure { error },
    }
}

fn query_analysis(
    request: AnalysisRequest,
    project_projections: bool,
) -> Result<QueryAnalysis, String> {
    let dialect: DialectType = match request.dialect.parse() {
        Ok(dialect) => dialect,
        Err(error) => {
            return Err(format!(
                "unsupported Polyglot dialect '{}': {error}",
                request.dialect
            ));
        }
    };
    let options = AnalyzeQueryOptions {
        complexity_guard: None,
        dialect,
        schema: request.schema,
    };
    if project_projections {
        analyze_query_for_project_projections(&request.sql, options)
    } else {
        analyze_query(&request.sql, options)
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
fn analyze_project_request(request: ProjectAnalysisRequest) -> ProjectAnalysisResponse {
    match analyze_project_result(request) {
        Ok(analysis) => ProjectAnalysisResponse::Success { analysis },
        Err(error) => ProjectAnalysisResponse::Failure { error },
    }
}

#[cfg(test)]
fn analyze_project_result(request: ProjectAnalysisRequest) -> Result<ProjectAnalysis, String> {
    let analysis = query_analysis(request.query, true)?;
    Ok(project_analysis(ProjectAnalysisInputs {
        analysis: &analysis,
        references: &request.references,
        function_return_types: &request.function_return_types,
        declared_column_order: &request.declared_column_order,
        recover_cte_facts: request.recover_cte_facts,
        rich_type_inference: request.rich_type_inference,
    }))
}

/// One interned column fact; it serializes as the positional array the batch reader expects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompactProjectColumn {
    name: usize,
    data_type: Option<usize>,
    nullability: u8,
    transform: u8,
    confidence: u8,
    upstream: Vec<(usize, usize, usize)>,
}

impl Serialize for CompactProjectColumn {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut fields = serializer.serialize_tuple(6)?;
        fields.serialize_element(&self.name)?;
        fields.serialize_element(&self.data_type)?;
        fields.serialize_element(&self.nullability)?;
        fields.serialize_element(&self.transform)?;
        fields.serialize_element(&self.confidence)?;
        fields.serialize_element(&self.upstream)?;
        fields.end()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompactProjectBatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    validations: Option<Vec<Option<ValidationResult>>>,
    strings: Vec<String>,
    facts: Vec<CompactProjectColumn>,
    templates: Vec<CompactProjectResponse>,
    analyses: Vec<CompactProjectProjection>,
    unique_query_count: usize,
    unique_projection_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CompactProjectResponse {
    Success((Vec<usize>, bool, bool)),
    LegacyTypeRecovery((Vec<usize>, bool, &'static str)),
    Failure(String),
}

type CompactProjectProjection = (usize, Vec<(usize, usize)>);

#[derive(Default)]
struct StringInterner {
    strings: Vec<String>,
    indexes: HashMap<String, usize>,
}

struct CompactProjectAccumulator {
    interner: StringInterner,
    facts: Vec<CompactProjectColumn>,
    fact_indexes: HashMap<CompactProjectColumn, usize>,
    templates: Vec<Option<CompactProjectResponse>>,
}

impl StringInterner {
    fn intern(&mut self, value: String) -> usize {
        if let Some(index) = self.indexes.get(&value) {
            return *index;
        }
        let index = self.strings.len();
        self.indexes.insert(value.clone(), index);
        self.strings.push(value);
        index
    }
}

impl CompactProjectAccumulator {
    fn new(template_count: usize) -> Self {
        Self {
            interner: StringInterner::default(),
            facts: Vec::new(),
            fact_indexes: HashMap::new(),
            templates: (0..template_count).map(|_| None).collect(),
        }
    }

    fn compact_analysis(
        &mut self,
        projection_index: usize,
        result: Result<ProjectAnalysis, String>,
    ) -> Result<(), String> {
        let analysis = match result {
            Ok(analysis) => analysis,
            Err(error) => {
                self.set_template(projection_index, CompactProjectResponse::Failure(error))?;
                return Ok(());
            }
        };
        if analysis.requires_legacy_fallback && !analysis.preserve_fallback_lineage {
            self.set_template(
                projection_index,
                CompactProjectResponse::Failure(
                    "native project type recovery requires legacy fallback".to_string(),
                ),
            )?;
            return Ok(());
        }
        let mut columns = Vec::with_capacity(analysis.columns.len());
        for (column, lineage) in analysis.columns.into_iter().zip(analysis.lineage_columns) {
            let upstream: Vec<(usize, usize, usize)> = lineage
                .upstream_columns
                .into_iter()
                .map(|source| {
                    (
                        self.interner.intern(source.resource_type),
                        self.interner.intern(source.resource_name),
                        self.interner.intern(source.column_name),
                    )
                })
                .collect();
            let fact = CompactProjectColumn {
                name: self.interner.intern(column.name),
                data_type: column.data_type.map(|value| self.interner.intern(value)),
                nullability: nullability_code(column.nullability),
                transform: transform_code(lineage.transform_kind),
                confidence: confidence_code(lineage.confidence),
                upstream,
            };
            let next_index = self.facts.len();
            let fact_index = *self.fact_indexes.entry(fact.clone()).or_insert_with(|| {
                self.facts.push(fact);
                next_index
            });
            columns.push(fact_index);
        }
        self.set_template(
            projection_index,
            if analysis.requires_legacy_fallback {
                CompactProjectResponse::LegacyTypeRecovery((
                    columns,
                    analysis.has_star,
                    "native project type recovery requires legacy fallback",
                ))
            } else {
                CompactProjectResponse::Success((
                    columns,
                    analysis.has_star,
                    analysis.preserve_fallback_lineage,
                ))
            },
        )
    }

    fn set_template(
        &mut self,
        projection_index: usize,
        response: CompactProjectResponse,
    ) -> Result<(), String> {
        let Some(slot) = self.templates.get_mut(projection_index) else {
            return Err("compact project analysis received an invalid projection index".to_owned());
        };
        if slot.is_some() {
            return Err(
                "compact project analysis received a duplicate projection index".to_owned(),
            );
        }
        *slot = Some(response);
        Ok(())
    }

    fn finish(
        mut self,
        projections: Vec<ProjectAnalysisProjectionResult>,
        unique_query_count: usize,
        unique_projection_count: usize,
    ) -> Result<CompactProjectBatch, String> {
        let mut compact_projections: Vec<CompactProjectProjection> =
            Vec::with_capacity(projections.len());
        for projection in projections {
            let mut resource_names: Vec<(usize, usize)> =
                Vec::with_capacity(projection.resource_names.len());
            for (canonical_name, resource_name) in projection.resource_names {
                resource_names.push((
                    self.interner.intern(canonical_name),
                    self.interner.intern(resource_name),
                ));
            }
            compact_projections.push((projection.projection_index, resource_names));
        }
        let templates: Option<Vec<CompactProjectResponse>> = self.templates.into_iter().collect();
        let Some(templates) = templates else {
            return Err("compact project analysis omitted a projection".to_owned());
        };
        Ok(CompactProjectBatch {
            validations: None,
            strings: self.interner.strings,
            facts: self.facts,
            templates,
            analyses: compact_projections,
            unique_query_count,
            unique_projection_count,
        })
    }
}

fn nullability_code(value: ProjectNullability) -> u8 {
    match value {
        ProjectNullability::Unknown => 0,
        ProjectNullability::NonNull => 1,
        ProjectNullability::Nullable => 2,
    }
}

fn transform_code(value: ProjectTransformKind) -> u8 {
    match value {
        ProjectTransformKind::Direct => 0,
        ProjectTransformKind::Cast => 1,
        ProjectTransformKind::Expression => 2,
        ProjectTransformKind::Aggregation => 3,
        ProjectTransformKind::Star => 4,
        ProjectTransformKind::Constant => 5,
    }
}

fn confidence_code(value: ProjectConfidence) -> u8 {
    match value {
        ProjectConfidence::Unknown => 0,
        ProjectConfidence::High => 1,
        ProjectConfidence::Medium => 2,
    }
}

fn project_analysis(inputs: ProjectAnalysisInputs<'_>) -> ProjectAnalysis {
    let cte_facts: CteColumnFacts = if inputs.recover_cte_facts {
        recovered_cte_facts(
            inputs.analysis,
            inputs.function_return_types,
            inputs.references,
            inputs.rich_type_inference,
        )
    } else {
        HashMap::new()
    };
    let infer_nullability = inputs.analysis.shape != QueryShape::SetOperation;
    let mut columns: Vec<ProjectColumn> = Vec::new();
    let mut lineage_columns: Vec<ProjectLineageColumn> = Vec::new();
    let mut requires_legacy_fallback = false;
    for projection in inputs
        .analysis
        .projections
        .iter()
        .filter(|projection| !projection.is_star)
    {
        let Some(output_column) = projection
            .name
            .as_ref()
            .filter(|name| !name.is_empty() && name.as_str() != SQL_WILDCARD)
        else {
            continue;
        };
        let (upstream_columns, confidence) = if inputs.analysis.shape == QueryShape::SetOperation {
            project_upstream_columns(projection, inputs.references)
        } else {
            recovered_projection_lineage(projection, inputs.references, &cte_facts)
        };
        let has_upstream = !upstream_columns.is_empty();
        let transform_kind = project_transform_kind(projection.transform_kind, has_upstream);
        requires_legacy_fallback |=
            direct_projection_requires_legacy_fallback(projection, &inputs, &cte_facts);
        columns.push(ProjectColumn {
            name: output_column.clone(),
            data_type: recovered_projection_type(
                projection,
                inputs.function_return_types,
                &cte_facts,
                ProjectionTypeOptions {
                    allow_direct_type_hint: inputs.rich_type_inference || inputs.recover_cte_facts,
                    rich_type_inference: inputs.rich_type_inference,
                },
            ),
            nullability: if infer_nullability {
                recovered_projection_nullability(projection, &cte_facts)
            } else {
                ProjectNullability::Unknown
            },
        });
        lineage_columns.push(ProjectLineageColumn {
            output_column: output_column.clone(),
            upstream_columns,
            transform_kind,
            confidence: if matches!(transform_kind, ProjectTransformKind::Constant) || has_upstream
            {
                confidence
            } else {
                ProjectConfidence::Unknown
            },
        });
    }
    if !inputs.analysis.star_projections.is_empty() && inputs.references.len() == 1 {
        let order: HashMap<&str, usize> = inputs
            .declared_column_order
            .iter()
            .enumerate()
            .map(|(index, name)| (name.as_str(), index))
            .collect();
        let fallback = order.len();
        columns.sort_by_key(|column| order.get(column.name.as_str()).copied().unwrap_or(fallback));
        lineage_columns.sort_by_key(|column| {
            order
                .get(column.output_column.as_str())
                .copied()
                .unwrap_or(fallback)
        });
    }
    ProjectAnalysis {
        columns,
        lineage_columns,
        has_star: inputs.analysis.has_root_star,
        requires_legacy_fallback,
        preserve_fallback_lineage: false,
    }
}

#[derive(Clone, Debug)]
struct CteColumnFact {
    data_type: Option<String>,
    authoritative_type: bool,
    nullability: ProjectNullability,
    upstream_columns: Vec<ProjectLineageSource>,
    confidence: ProjectConfidence,
}

#[derive(Clone, Copy)]
struct ProjectionTypeOptions {
    allow_direct_type_hint: bool,
    rich_type_inference: bool,
}

type CteColumnFacts = HashMap<(String, String), CteColumnFact>;

fn recovered_cte_facts(
    analysis: &QueryAnalysis,
    function_return_types: &HashMap<String, String>,
    references: &HashMap<String, LineageResource>,
    rich_type_inference: bool,
) -> CteColumnFacts {
    let mut facts: CteColumnFacts = HashMap::new();
    for cte in &analysis.cte_facts {
        for projection in &cte.projections {
            let Some(name) = projection.name.as_ref() else {
                continue;
            };
            let data_type = recovered_projection_type(
                projection,
                function_return_types,
                &facts,
                ProjectionTypeOptions {
                    allow_direct_type_hint: true,
                    rich_type_inference,
                },
            );
            let authoritative_type =
                projection_type_is_authoritative(projection, function_return_types, &facts);
            let nullability = if cte.shape == Some(QueryShape::SetOperation) {
                project_nullability(projection.nullability)
            } else {
                recovered_projection_nullability(projection, &facts)
            };
            let (upstream_columns, confidence) = if cte.shape == Some(QueryShape::SetOperation) {
                project_upstream_columns(projection, references)
            } else {
                recovered_projection_lineage(projection, references, &facts)
            };
            facts.insert(
                (cte.name.to_lowercase(), name.to_lowercase()),
                CteColumnFact {
                    data_type,
                    authoritative_type,
                    nullability,
                    upstream_columns,
                    confidence,
                },
            );
        }
    }
    facts
}

fn recovered_projection_type(
    projection: &polyglot_sql::ProjectionFact,
    function_return_types: &HashMap<String, String>,
    cte_facts: &CteColumnFacts,
    options: ProjectionTypeOptions,
) -> Option<String> {
    if projection.transform_kind == TransformKind::Cast
        && let Some(data_type) = projection
            .cast_type
            .as_deref()
            .filter(|value| !value.is_empty() && *value != UNKNOWN_SQL_TYPE)
    {
        return Some(normalize_project_type(data_type));
    }
    if let Some(data_type) = projection.transform_function.as_ref().and_then(|function| {
        function_return_types
            .get(&function.name.to_uppercase())
            .cloned()
    }) {
        return Some(data_type);
    }
    if let Some(data_type) = projection_cte_source_type(projection, cte_facts) {
        return Some(data_type);
    }
    if projection.transform_kind == TransformKind::Direct {
        let cte_fact = cte_column_fact(projection, cte_facts);
        if cte_fact.is_some_and(|fact| fact.authoritative_type)
            && let Some(data_type) = cte_fact.and_then(|fact| fact.data_type.clone())
        {
            return Some(data_type);
        }
        if options.allow_direct_type_hint
            && let Some(data_type) = project_type(projection, function_return_types, true)
        {
            return Some(data_type);
        }
        return cte_fact.and_then(|fact| fact.data_type.clone());
    }
    if options.rich_type_inference {
        return project_type(projection, function_return_types, true);
    }
    legacy_expression_type(projection)
}

fn projection_cte_source_type(
    projection: &polyglot_sql::ProjectionFact,
    cte_facts: &CteColumnFacts,
) -> Option<String> {
    if projection.transform_kind != TransformKind::Direct {
        return None;
    }
    let mut data_types: Vec<String> = projection
        .type_column_args
        .iter()
        .filter_map(|column| {
            let source = column.source_name.as_ref().or(column.table.as_ref())?;
            cte_facts
                .get(&(source.to_lowercase(), column.column.to_lowercase()))
                .and_then(|fact| fact.data_type.clone())
        })
        .collect();
    data_types.sort();
    data_types.dedup();
    let [data_type] = data_types.as_slice() else {
        return None;
    };
    Some(data_type.clone())
}

fn direct_projection_requires_legacy_fallback(
    projection: &polyglot_sql::ProjectionFact,
    inputs: &ProjectAnalysisInputs<'_>,
    cte_facts: &CteColumnFacts,
) -> bool {
    if projection.transform_kind == TransformKind::Aggregation
        && !inputs.analysis.cte_facts.is_empty()
    {
        return true;
    }
    if inputs.rich_type_inference
        || projection.transform_kind != TransformKind::Direct
        || projection.passthrough_source.is_none()
    {
        return false;
    }
    let Some(fact) = cte_column_fact(projection, cte_facts) else {
        return true;
    };
    let Some(fact_type) = fact.data_type.as_deref() else {
        return true;
    };
    if !fact.authoritative_type {
        return true;
    }
    project_type(projection, inputs.function_return_types, true).is_some_and(|root_type| {
        canonical_project_type(&root_type) != canonical_project_type(fact_type)
    })
}

fn canonical_project_type(value: &str) -> String {
    let compact: String = normalize_project_type(value)
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '_')
        .flat_map(char::to_uppercase)
        .collect();
    match compact.as_str() {
        "INTEGER" => "INT".to_string(),
        "VARCHAR" => "TEXT".to_string(),
        _ => compact,
    }
}

fn projection_type_is_authoritative(
    projection: &polyglot_sql::ProjectionFact,
    function_return_types: &HashMap<String, String>,
    cte_facts: &CteColumnFacts,
) -> bool {
    if projection
        .cast_type
        .as_deref()
        .is_some_and(|value| !value.is_empty() && value != UNKNOWN_SQL_TYPE)
    {
        return true;
    }
    if projection
        .transform_function
        .as_ref()
        .is_some_and(|function| function_return_types.contains_key(&function.name.to_uppercase()))
    {
        return true;
    }
    let source_facts: Vec<&CteColumnFact> = projection
        .type_column_args
        .iter()
        .filter_map(|column| {
            let source = column.source_name.as_ref().or(column.table.as_ref())?;
            cte_facts.get(&(source.to_lowercase(), column.column.to_lowercase()))
        })
        .collect();
    if projection.transform_kind == TransformKind::Direct
        && !source_facts.is_empty()
        && source_facts.iter().all(|fact| fact.authoritative_type)
    {
        return true;
    }
    if projection.transform_kind == TransformKind::Direct
        && projection.passthrough_source.is_none()
        && projection
            .type_hint
            .as_deref()
            .is_some_and(|value| !value.is_empty() && value != UNKNOWN_SQL_TYPE)
    {
        return true;
    }
    projection.transform_kind == TransformKind::Direct
        && cte_column_fact(projection, cte_facts).is_some_and(|fact| fact.authoritative_type)
}

fn legacy_expression_type(projection: &polyglot_sql::ProjectionFact) -> Option<String> {
    if projection.transform_kind == TransformKind::Cast {
        return projection.cast_type.as_deref().map(normalize_project_type);
    }
    if projection.transform_kind == TransformKind::Expression
        && projection
            .type_hint
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("BOOLEAN"))
    {
        return Some("BOOLEAN".to_string());
    }
    None
}

fn recovered_projection_nullability(
    projection: &polyglot_sql::ProjectionFact,
    cte_facts: &CteColumnFacts,
) -> ProjectNullability {
    let direct = project_nullability(projection.nullability);
    if !matches!(direct, ProjectNullability::Unknown) {
        return direct;
    }
    if projection.transform_kind == TransformKind::Direct
        && let Some(fact) = cte_column_fact(projection, cte_facts)
    {
        return fact.nullability;
    }
    ProjectNullability::Unknown
}

fn cte_column_fact<'a>(
    projection: &polyglot_sql::ProjectionFact,
    cte_facts: &'a CteColumnFacts,
) -> Option<&'a CteColumnFact> {
    let source = projection.passthrough_source.as_ref()?;
    let column = projection.passthrough_column.as_ref()?;
    cte_facts.get(&(source.to_lowercase(), column.to_lowercase()))
}

fn recovered_projection_lineage(
    projection: &polyglot_sql::ProjectionFact,
    references: &HashMap<String, LineageResource>,
    cte_facts: &CteColumnFacts,
) -> (Vec<ProjectLineageSource>, ProjectConfidence) {
    if matches!(
        projection.transform_kind,
        TransformKind::Direct | TransformKind::Cast
    ) && let Some(fact) = cte_column_fact(projection, cte_facts)
    {
        return (fact.upstream_columns.clone(), fact.confidence);
    }
    project_upstream_columns(projection, references)
}

fn project_nullability(value: ProjectionNullability) -> ProjectNullability {
    match value {
        ProjectionNullability::NonNull => ProjectNullability::NonNull,
        ProjectionNullability::Nullable => ProjectNullability::Nullable,
        ProjectionNullability::Unknown => ProjectNullability::Unknown,
    }
}

fn project_type(
    projection: &polyglot_sql::ProjectionFact,
    function_return_types: &HashMap<String, String>,
    normalize_expression_type: bool,
) -> Option<String> {
    projection
        .cast_type
        .as_ref()
        .filter(|value| !value.is_empty() && value.as_str() != UNKNOWN_SQL_TYPE)
        .or_else(|| {
            projection
                .type_hint
                .as_ref()
                .filter(|value| !value.is_empty() && value.as_str() != UNKNOWN_SQL_TYPE)
        })
        .map(|value| {
            if normalize_expression_type {
                normalize_project_type(value)
            } else {
                value.to_string()
            }
        })
        .or_else(|| {
            projection.transform_function.as_ref().and_then(|function| {
                function_return_types
                    .get(&function.name.to_uppercase())
                    .cloned()
            })
        })
}

fn normalize_project_type(value: &str) -> String {
    let upper = value.to_uppercase();
    if upper == VARCHAR_SQL_TYPE {
        return "TEXT".to_string();
    }
    if let Some(length) = upper
        .strip_prefix("TEXT(")
        .and_then(|suffix| suffix.strip_suffix(')'))
    {
        return format!("VARCHAR({length})");
    }
    value.to_string()
}

fn project_upstream_columns(
    projection: &polyglot_sql::ProjectionFact,
    references: &HashMap<String, LineageResource>,
) -> (Vec<ProjectLineageSource>, ProjectConfidence) {
    let mut columns: Vec<ProjectLineageSource> = Vec::new();
    let mut seen: HashSet<(&str, &str, &str)> = HashSet::new();
    let mut confidence = ProjectConfidence::High;
    for upstream in &projection.upstream {
        if upstream.column.is_empty() || upstream.column == SQL_WILDCARD {
            continue;
        }
        let Some(source_name) = upstream
            .source_name
            .as_deref()
            .or(upstream.table.as_deref())
        else {
            confidence = ProjectConfidence::Unknown;
            continue;
        };
        let Some(resource) = references.get(source_name) else {
            continue;
        };
        if upstream.confidence != ReferenceConfidence::Resolved && upstream.source_alias.is_none() {
            confidence = ProjectConfidence::Medium;
        }
        let key = (
            resource.resource_type.as_str(),
            resource.resource_name.as_str(),
            upstream.column.as_str(),
        );
        if !seen.insert(key) {
            continue;
        }
        columns.push(ProjectLineageSource {
            resource_type: resource.resource_type.clone(),
            resource_name: resource.resource_name.clone(),
            column_name: upstream.column.clone(),
        });
    }
    let columns = order_project_sources(columns);
    (columns, confidence)
}

fn order_project_sources(mut columns: Vec<ProjectLineageSource>) -> Vec<ProjectLineageSource> {
    columns.sort_by(|left, right| {
        (
            left.resource_type.as_str(),
            left.resource_name.as_str(),
            left.column_name.as_str(),
        )
            .cmp(&(
                right.resource_type.as_str(),
                right.resource_name.as_str(),
                right.column_name.as_str(),
            ))
    });
    columns.dedup_by(|left, right| {
        left.resource_type == right.resource_type
            && left.resource_name == right.resource_name
            && left.column_name == right.column_name
    });
    columns
}

fn project_transform_kind(kind: TransformKind, has_upstream: bool) -> ProjectTransformKind {
    match kind {
        TransformKind::Star => ProjectTransformKind::Star,
        TransformKind::Cast => ProjectTransformKind::Cast,
        TransformKind::Aggregation => ProjectTransformKind::Aggregation,
        TransformKind::Constant => ProjectTransformKind::Constant,
        TransformKind::Direct => ProjectTransformKind::Direct,
        TransformKind::Expression if !has_upstream => ProjectTransformKind::Constant,
        TransformKind::Expression => ProjectTransformKind::Expression,
    }
}
