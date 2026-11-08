//! Coarse project-level planning for SQL-native test artifacts.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::Infallible;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use sqlquack_syntax::{Expression, Parsed};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use regex::Regex;
#[cfg(test)]
use serde::Serialize;
use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::graph_walk::main::depth_first_postorder::depth_first_postorder;

use crate::analysis_route::types::NativeDialect;
use crate::compiler::_helpers::sql_tests::cte_rename::defined_cte_keys;
use crate::compiler::_helpers::sql_tests::cte_slices::{SliceDialect, strip_statement_terminators};
use crate::compiler::_helpers::sql_tests::cte_sql::{
    cte_definition_sql, leading_with_prefix_end, with_leading_ctes, with_unique_ctes,
};
use crate::compiler::_helpers::sql_tests::expected_columns::expected_columns;
use crate::compiler::_helpers::sql_tests::helper_names::HELPER_PREFIX;
use crate::compiler::_helpers::sql_tests::helper_scope::{
    ResolvedHelper, ScopeGraph, ScopeRequest, helper_scope_ctes, merged_scoped_ctes,
};
use crate::compiler::_helpers::sql_tests::markers::{
    ProtectedRanges, marker_names, marker_names_in, replace_callable_markers,
    replace_dbt_ref_markers, replace_named_markers,
};
use crate::compiler::_helpers::sql_tests::reader_references::{
    CompilerReads, assertion_ref_targets, calls_reference, unresolvable_call,
    unresolved_reference_error,
};
use crate::compiler::_helpers::sql_tests::real_model_limit::{
    RealModelChain, real_model_limit_message,
};
use crate::compiler::_helpers::sql_tests::relation_markers::relation_marker_calls;
use crate::compiler::_helpers::sql_tests::rendering::{
    RenderRequest, render_comparison_sql, rendered_chain_steps, sql_test_parser,
};
use crate::compiler::models::{
    SqlTestAssertionStep, SqlTestChainBatch, SqlTestChainStep, SqlTestCte, SqlTestPlan,
    SqlTestPlanBatch, SqlTestPlanBatchOutcome, SqlTestPlanFunction, SqlTestPlanPayload,
    SqlTestPlanTest, SqlTestPlanWarning,
};
use crate::constants::{
    SQL_TEST_ACTUAL_CTE, SQL_TEST_ACTUAL_CTE_PREFIX, SQL_TEST_EXPECTED_CTE,
    TABLE_FUNCTION_TEST_MODE, UDF_TEST_MODE,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

const MAX_WORKERS: usize = 4;
const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const REF_PREFIX: &str = "__ref__";
pub(crate) const SOURCE_PREFIX: &str = "__source__";
pub(crate) const SEED_PREFIX: &str = "__seed__";
pub(crate) const DBT_REF_PREFIX: &str = "__dbt_ref__";
pub(crate) const TABLE_FUNCTION_PREFIX: &str = "__table_fn__";
pub(crate) const MOCK_CTE_PREFIXES: [&str; 5] = [
    REF_PREFIX,
    SOURCE_PREFIX,
    SEED_PREFIX,
    DBT_REF_PREFIX,
    TABLE_FUNCTION_PREFIX,
];
pub(crate) const EXPECTED_PREFIX: &str = "__expected__";
pub(crate) const ASSERT_PREFIX: &str = "__assert__";
const REF_FUNCTION: &str = "__ref";
const SOURCE_FUNCTION: &str = "__source";
const SEED_FUNCTION: &str = "__seed";
pub(crate) const DBT_REF_FUNCTION: &str = "__dbt_ref";
const UDF_FUNCTION: &str = "__udf";
const TABLE_FUNCTION: &str = "__table_fn";

#[cfg(test)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChainBatchResponse {
    chains: Vec<Vec<String>>,
}

struct PlannedResponse {
    request: RenderRequest,
    model_names: Vec<String>,
    warnings: Vec<SqlTestPlanWarning>,
}

#[derive(Clone)]
struct ProjectContext {
    models: HashMap<String, ModelInputOwned>,
    functions: HashMap<String, SqlTestPlanFunction>,
    sql_analysis_enabled: bool,
    dialect: String,
    set_difference_operator: String,
    requires_derived_table_aliases: bool,
    analysis_templates: Arc<AnalysisTemplateCache>,
    patterns: SqlTestPatterns,
    parser: &'static dyn NativeDialect,
    rejects_nested_with: bool,
    max_real_models: Option<usize>,
    /// Each model query's reserved CTE name, since every test reading the model checks it.
    reserved_ctes: Arc<Mutex<HashMap<String, Option<String>>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SqlTestPatterns {
    pub(crate) reference: Regex,
    pub(crate) source: Regex,
    pub(crate) seed: Regex,
    pub(crate) udf: Regex,
    table_function: Regex,
    pub(crate) dbt_reference: Regex,
    pub(crate) test_reference: Regex,
    pub(crate) lexical: LexicalSyntax,
    pub(crate) identifier: Regex,
}

impl SqlTestPatterns {
    pub(crate) fn new(lexical: LexicalSyntax) -> Result<Self, String> {
        Ok(Self {
            reference: compile_pattern(r#"(?i)__ref\(\"([^\"]+)\"\)"#)?,
            source: compile_pattern(r#"(?i)__source\(\"([^\"]+)\"\)"#)?,
            seed: compile_pattern(r#"(?i)__seed\(\"([^\"]+)\"\)"#)?,
            udf: compile_pattern(r#"(?i)__udf\(\"([^\"]+)\"\)"#)?,
            table_function: compile_pattern(r#"(?i)__table_fn\(\"([^\"]+)\"\)"#)?,
            dbt_reference: compile_pattern(
                r#"(?i)__dbt_ref\(\s*\"([^\"]+)\"\s*(?:,\s*\"([^\"]+)\"\s*)?\)"#,
            )?,
            test_reference: compile_pattern(r#"(?i)__(?:ref|source|seed|dbt_ref|table_fn)\("#)?,
            lexical,
            identifier: compile_pattern(r"[A-Za-z_][A-Za-z0-9_$]*")?,
        })
    }
}

#[derive(Clone)]
struct ModelInputOwned {
    query_sql: String,
    model_dependencies: Vec<String>,
}

#[derive(Clone)]
struct AnalysisResolvedSql {
    resolved_sql: String,
    cte_body_sql: String,
    generated_ctes: Vec<(String, String)>,
    reachable_mock_names: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnalysisTemplate {
    pub(crate) existing_cte_names: HashSet<String>,
    pub(crate) marker_calls: Arc<[(String, String)]>,
}

type AnalysisTemplateCell = Arc<OnceLock<Option<AnalysisTemplate>>>;
type AnalysisTemplateCache = Mutex<HashMap<String, AnalysisTemplateCell>>;

pub(crate) struct TestFixtures {
    pub(crate) mock_refs: BTreeMap<String, String>,
    pub(crate) mock_sources: BTreeMap<String, String>,
    pub(crate) mock_seeds: BTreeMap<String, String>,
    pub(crate) mock_dbt_refs: BTreeMap<String, String>,
    pub(crate) mock_table_functions: BTreeMap<String, String>,
    pub(crate) helpers: Vec<SqlTestCte>,
    pub(crate) scope: ScopeGraph,
    pub(crate) expected: BTreeMap<String, String>,
    pub(crate) assertions: Vec<(String, String)>,
}

struct DirectTestPlan {
    name: String,
    mode: String,
    actual_cte: SqlTestCte,
    expected_cte: SqlTestCte,
    helpers: Vec<SqlTestCte>,
}

struct ModelTestPlan {
    test_name: String,
    file_label: String,
    authored_ctes: Vec<SqlTestCte>,
    model_query_overrides: BTreeMap<String, String>,
    expected_ctes: Vec<SqlTestCte>,
    expected_model_names: Vec<String>,
    assertion_ctes: Vec<SqlTestCte>,
    reads: CompilerReads,
}

struct TextualChainRequest<'a> {
    ordered_names: &'a [String],
    overrides: &'a BTreeMap<String, String>,
    fixtures: &'a TestFixtures,
    context: &'a ProjectContext,
    chain: &'a mut TextualChain,
}

/// Chain models resolved once each as named CTE bodies, so work scales with models not paths.
#[derive(Default)]
struct TextualChain {
    cte_names: HashMap<String, String>,
    bodies: HashMap<String, String>,
    dependencies: HashMap<String, Vec<String>>,
    mock_ctes: HashMap<String, Vec<(String, String)>>,
    order: Vec<String>,
}

struct TextualChainInputs<'a> {
    ordered_names: &'a [String],
    overrides: &'a BTreeMap<String, String>,
    fixtures: &'a TestFixtures,
    context: &'a ProjectContext,
}

struct TextualStep {
    resolved_sql: String,
    lifted_ctes: Vec<(String, String)>,
    body_sql: String,
}

impl TextualChain {
    fn extend(&mut self, inputs: &TextualChainInputs<'_>) -> Result<HashSet<String>, String> {
        let mut reachable: HashSet<String> = HashSet::new();
        for model_name in inputs.ordered_names {
            if self.bodies.contains_key(model_name) {
                continue;
            }
            let Some(model) = inputs.context.models.get(model_name) else {
                continue;
            };
            let query_sql = inputs.overrides.get(model_name).unwrap_or(&model.query_sql);
            let mut chain_references: Vec<String> = Vec::new();
            let resolution = resolve_textual_sql(TextualResolutionRequest {
                query_sql,
                fixtures: inputs.fixtures,
                resolved_chain: &self.cte_names,
                chain_references: Some(&mut chain_references),
                functions: &inputs.context.functions,
                patterns: &inputs.context.patterns,
            })?;
            reachable.extend(resolution.reached);
            self.mock_ctes
                .insert(model_name.to_string(), resolution.mock_ctes);
            let body = strip_statement_terminators(
                &resolution.sql,
                SliceDialect::new(Some(&inputs.context.dialect)),
            )
            .to_string();
            self.insert(model_name, body, chain_references);
        }
        Ok(reachable)
    }

    fn insert(&mut self, model_name: &str, body: String, dependencies: Vec<String>) {
        self.cte_names
            .insert(model_name.to_string(), format!("{REF_PREFIX}{model_name}"));
        self.bodies.insert(model_name.to_string(), body);
        self.dependencies
            .insert(model_name.to_string(), dependencies);
        self.order.push(model_name.to_string());
    }

    /// Return the generated CTEs needed by `roots`, dependencies first, each exactly once.
    fn closure_ctes(&self, roots: &[String], include_roots: bool) -> Vec<(String, String)> {
        let mut needed: HashSet<String> = HashSet::new();
        if include_roots {
            needed.extend(roots.iter().cloned());
        }
        let Ok(()) = breadth_first_reach(roots.iter().cloned(), None, &mut needed, |name| {
            Ok::<_, Infallible>(self.dependencies.get(name).into_iter().flatten().cloned())
        });
        let mut ctes: Vec<(String, String)> = with_unique_ctes(
            Vec::new(),
            self.order
                .iter()
                .filter(|name| needed.contains(*name) || roots.contains(name))
                .filter_map(|name| self.mock_ctes.get(name))
                .flatten()
                .cloned(),
        );
        ctes.extend(
            self.order
                .iter()
                .filter(|name| needed.contains(*name))
                .filter_map(|name| {
                    Some((
                        self.cte_names.get(name)?.clone(),
                        self.bodies.get(name)?.clone(),
                    ))
                }),
        );
        ctes
    }

    fn step(&self, model_name: &str) -> Option<TextualStep> {
        let body_sql = self.bodies.get(model_name)?.clone();
        let lifted_ctes = self.closure_ctes(&[model_name.to_string()], false);
        Some(TextualStep {
            resolved_sql: with_leading_ctes(&lifted_ctes, &body_sql),
            lifted_ctes,
            body_sql,
        })
    }
}

struct AssertionResolutionRequest<'a> {
    assertion_sql: &'a str,
    fixtures: &'a TestFixtures,
    chain: &'a TextualChain,
    functions: &'a HashMap<String, SqlTestPlanFunction>,
    requires_flat_ctes: bool,
    patterns: &'a SqlTestPatterns,
}

struct AnalysisResolutionRequest<'a> {
    query_sql: &'a str,
    fixture_ctes: &'a [(String, String)],
    fixtures: &'a TestFixtures,
    resolved_chain: &'a HashMap<String, AnalysisResolvedSql>,
    functions: &'a HashMap<String, SqlTestPlanFunction>,
    file_label: &'a str,
    parser: &'a dyn NativeDialect,
    templates: &'a AnalysisTemplateCache,
    patterns: &'a SqlTestPatterns,
}

struct TextualResolutionRequest<'a> {
    query_sql: &'a str,
    fixtures: &'a TestFixtures,
    resolved_chain: &'a HashMap<String, String>,
    chain_references: Option<&'a mut Vec<String>>,
    functions: &'a HashMap<String, SqlTestPlanFunction>,
    patterns: &'a SqlTestPatterns,
}

struct TopoSortRequest<'a> {
    expected_names: &'a [String],
    models: &'a HashMap<String, ModelInputOwned>,
    overrides: &'a BTreeMap<String, String>,
    mock_refs: &'a BTreeMap<String, String>,
    patterns: &'a SqlTestPatterns,
}

struct MockTargetRequest<'a> {
    prefix: &'a str,
    referenced_name: &'a str,
    mocks: &'a BTreeMap<String, String>,
    fixtures: &'a TestFixtures,
    file_label: &'a str,
}

#[derive(Default)]
struct GeneratedCteState {
    generated: Vec<(String, String)>,
    names: HashSet<String>,
    reachable: HashSet<String>,
}

impl GeneratedCteState {
    fn insert(&mut self, name: &str, sql: &str, file_label: &str) -> Result<(), String> {
        if let Some((_, existing)) = self
            .generated
            .iter()
            .find(|(existing_name, _)| existing_name == name)
            && existing == sql
        {
            return Ok(());
        }
        if self.names.contains(name) && !self.generated.iter().any(|(existing, _)| existing == name)
        {
            return Err(compile_error(&format!(
                "SQL test '{file_label}' defines CTE '{name}', which conflicts with the generated CTE"
            )));
        }
        if self.names.insert(name.to_string()) {
            self.generated.push((name.to_string(), sql.to_string()));
        }
        Ok(())
    }

    /// Add a test helper read by a mock; model CTEs of the same name stay nested apart from it.
    fn insert_helper(&mut self, name: &str, sql: &str) {
        if !self.generated.iter().any(|(existing, _)| existing == name) {
            self.generated.push((name.to_string(), sql.to_string()));
        }
    }

    fn mock_target(&mut self, request: MockTargetRequest<'_>) -> Result<Option<String>, String> {
        if !request.mocks.contains_key(request.referenced_name) {
            return Ok(None);
        }
        self.reachable.insert(request.referenced_name.to_string());
        let generated_name = format!("{}{}", request.prefix, request.referenced_name);
        if self.names.contains(&generated_name)
            && !self
                .generated
                .iter()
                .any(|(existing_name, _)| existing_name == &generated_name)
        {
            let referenced_kind = request.prefix.trim_matches('_');
            return Err(compile_error(&format!(
                "SQL test '{}' defines CTE '{generated_name}', which conflicts with the generated {referenced_kind} CTE for '{}'",
                request.file_label, request.referenced_name
            )));
        }
        for dependency in request.fixtures.scope.mock_dependencies(&generated_name) {
            let Some(mock_name) = dependency.mock_name else {
                self.insert_helper(dependency.generated_name, dependency.sql);
                continue;
            };
            self.reachable.insert(mock_name.to_string());
            self.insert(
                dependency.generated_name,
                dependency.sql,
                request.file_label,
            )?;
        }
        let sql = request
            .fixtures
            .scope
            .mock_sql(&generated_name)
            .ok_or_else(|| planner_error("SQL-test mock scope is incomplete"))?
            .to_string();
        self.insert(&generated_name, &sql, request.file_label)?;
        Ok(Some(generated_name))
    }
}

/// Return each test's topologically ordered unmocked model chain without planning SQL.
#[cfg(test)]
pub(crate) fn resolve_chains_json(request_json: &str) -> Result<String, String> {
    let request: SqlTestChainBatch =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let chains: Vec<Vec<String>> = resolve_chains(request)?;
    serde_json::to_string(&ChainBatchResponse { chains }).map_err(|error| error.to_string())
}

/// Each test's topologically ordered unmocked model chain, in request order.
pub(crate) fn resolve_chains(request: SqlTestChainBatch) -> Result<Vec<Vec<String>>, String> {
    let patterns = SqlTestPatterns::new(request.lexical_syntax)?;
    let models: HashMap<String, ModelInputOwned> = request
        .models
        .into_iter()
        .map(|model| {
            (
                model.name,
                ModelInputOwned {
                    query_sql: String::new(),
                    model_dependencies: model.model_dependencies,
                },
            )
        })
        .collect();
    let chains: Vec<Vec<String>> = request
        .tests
        .into_iter()
        .map(|test| match test.payload {
            SqlTestPlanPayload::Direct { .. } => Ok(Vec::new()),
            SqlTestPlanPayload::Model {
                authored_ctes,
                model_query_overrides,
                expected_ctes,
                expected_model_names,
                assertion_ctes,
                read_helper_names,
                reference_target_model_names,
            } => {
                let reads = CompilerReads::required(
                    &test.file_label,
                    read_helper_names,
                    reference_target_model_names,
                )?;
                let fixtures = classify_fixtures(authored_ctes, expected_ctes, assertion_ctes);
                let expected_names =
                    chain_root_names(expected_model_names, &reads, &fixtures, &patterns);
                Ok(topo_sort_model_chain(&TopoSortRequest {
                    expected_names: &expected_names,
                    models: &models,
                    overrides: &model_query_overrides,
                    mock_refs: &fixtures.mock_refs,
                    patterns: &patterns,
                }))
            }
        })
        .collect::<Result<_, String>>()?;
    Ok(chains)
}

/// Models the test runs: expected models, assertion targets and the compiler's reference targets.
fn chain_root_names(
    mut expected_names: Vec<String>,
    reads: &CompilerReads,
    fixtures: &TestFixtures,
    patterns: &SqlTestPatterns,
) -> Vec<String> {
    expected_names.extend(assertion_ref_targets(fixtures, patterns));
    expected_names.extend(reads.reference_target_model_names.iter().cloned());
    dedupe(expected_names)
}

#[cfg(test)]
pub(crate) fn plan_and_render_json(request_json: &str) -> Result<String, String> {
    let request: SqlTestPlanBatch =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    serde_json::to_string(&plan_batch(request)?).map_err(|error| error.to_string())
}

/// Plan every test of `request` on a bounded pool, keeping request order.
pub(crate) fn plan_batch(request: SqlTestPlanBatch) -> Result<SqlTestPlanBatchOutcome, String> {
    let render_sql = request.render_sql;
    let include_plan = request.include_plan;
    let Some(parser) = sql_test_parser(request.sql_analysis_dialect.as_deref()) else {
        if request.tests.is_empty() {
            return Ok(SqlTestPlanBatchOutcome {
                plans: Vec::new(),
                planning_ns: 0,
                rendering_ns: 0,
            });
        }
        return Err(unavailable_sql_tests(request.sql_analysis_dialect.as_deref()));
    };
    let rejects_nested_with =
        SliceDialect::new(request.sql_analysis_dialect.as_deref()).rejects_nested_with();
    let context = ProjectContext {
        models: request
            .models
            .into_iter()
            .map(|model| {
                (
                    model.name,
                    ModelInputOwned {
                        query_sql: model.query_sql,
                        model_dependencies: model.model_dependencies,
                    },
                )
            })
            .collect(),
        functions: request
            .functions
            .into_iter()
            .map(|function| (function.name.clone(), function))
            .collect(),
        sql_analysis_enabled: request.sql_analysis_enabled,
        dialect: request
            .sql_analysis_dialect
            .unwrap_or_else(|| "generic".to_string()),
        set_difference_operator: request.set_difference_operator,
        requires_derived_table_aliases: request.requires_derived_table_aliases,
        analysis_templates: Arc::new(Mutex::new(HashMap::new())),
        patterns: SqlTestPatterns::new(request.lexical_syntax)?,
        parser,
        rejects_nested_with,
        max_real_models: request.max_unit_test_real_models,
        reserved_ctes: Arc::new(Mutex::new(HashMap::new())),
    };
    let workers = request.workers.clamp(1, MAX_WORKERS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.min(request.tests.len().max(1)))
        .stack_size(WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let batch_start = Instant::now();
    let responses: Vec<Result<(SqlTestPlan, u128, u128), String>> = pool.install(|| {
        request
            .tests
            .into_par_iter()
            .map(|test| {
                let planning_start = Instant::now();
                let test_name = test.name.clone();
                let mut planned = plan_test(test, &context)?;
                let planning_ns = planning_start.elapsed().as_nanos();
                let rendering_start = Instant::now();
                let rendered = (render_sql || context.rejects_nested_with)
                    .then(|| render_comparison_sql(&planned.request));
                let sql = match rendered {
                    Some(Ok(sql)) => render_sql.then_some(sql),
                    Some(Err(message)) => {
                        planned.warnings.push(SqlTestPlanWarning {
                            model_name: None,
                            severity: "error",
                            message: format!("test '{test_name}' cannot be rendered: {message}"),
                        });
                        render_sql.then(String::new)
                    }
                    None => None,
                };
                if !include_plan {
                    planned.request.chain.clear();
                    planned.request.assertions.clear();
                }
                let response = SqlTestPlan {
                    sql,
                    chain: planned.request.chain,
                    assertions: planned.request.assertions,
                    model_names: planned.model_names,
                    warnings: planned.warnings,
                };
                Ok((response, planning_ns, rendering_start.elapsed().as_nanos()))
            })
            .collect()
    });
    let responses: Vec<(SqlTestPlan, u128, u128)> =
        responses.into_iter().collect::<Result<_, _>>()?;
    let batch_ns = batch_start.elapsed().as_nanos();
    let planning_cpu_ns: u128 = responses.iter().map(|(_, value, _)| *value).sum();
    let rendering_cpu_ns: u128 = responses.iter().map(|(_, _, value)| *value).sum();
    let measured_cpu_ns = planning_cpu_ns + rendering_cpu_ns;
    let planning_ns = batch_ns
        .saturating_mul(planning_cpu_ns)
        .checked_div(measured_cpu_ns)
        .unwrap_or(batch_ns);
    let rendering_ns = batch_ns.saturating_sub(planning_ns);
    let artifacts: Vec<SqlTestPlan> = responses
        .into_iter()
        .map(|(response, _, _)| response)
        .collect();
    Ok(SqlTestPlanBatchOutcome {
        plans: artifacts,
        planning_ns: planning_ns.min(u128::from(u64::MAX)) as u64,
        rendering_ns: rendering_ns.min(u128::from(u64::MAX)) as u64,
    })
}

fn plan_test(test: SqlTestPlanTest, context: &ProjectContext) -> Result<PlannedResponse, String> {
    match test.payload {
        SqlTestPlanPayload::Direct {
            mode,
            actual_cte,
            expected_cte,
            helper_ctes,
        } => plan_direct_test(
            DirectTestPlan {
                name: test.name,
                mode,
                actual_cte,
                expected_cte,
                helpers: helper_ctes,
            },
            context,
        ),
        SqlTestPlanPayload::Model {
            authored_ctes,
            model_query_overrides,
            expected_ctes,
            expected_model_names,
            assertion_ctes,
            read_helper_names,
            reference_target_model_names,
        } => {
            let reads = CompilerReads::required(
                &test.file_label,
                read_helper_names,
                reference_target_model_names,
            )?;
            plan_model_test(
                ModelTestPlan {
                    test_name: test.name,
                    file_label: test.file_label,
                    authored_ctes,
                    model_query_overrides,
                    expected_ctes,
                    expected_model_names,
                    assertion_ctes,
                    reads,
                },
                context,
            )
        }
    }
}

fn plan_direct_test(
    plan: DirectTestPlan,
    context: &ProjectContext,
) -> Result<PlannedResponse, String> {
    let mut actual_sql = plan.actual_cte.sql_body;
    if plan.mode == UDF_TEST_MODE {
        actual_sql =
            resolve_function_calls(&actual_sql, &context.functions, false, &context.patterns)?;
    } else if plan.mode == TABLE_FUNCTION_TEST_MODE {
        actual_sql =
            resolve_function_calls(&actual_sql, &context.functions, true, &context.patterns)?;
    }
    let model_name = format!("{} {}", plan.mode, plan.name);
    let request = RenderRequest {
        chain: vec![SqlTestChainStep {
            model_name: model_name.clone(),
            resolved_sql: with_helper_ctes(&actual_sql, &plan.helpers),
            expected_cte_sql: Some(with_helper_ctes(&plan.expected_cte.sql_body, &plan.helpers)),
            expected_lifted_ctes: Vec::new(),
            lifted_ctes: Vec::new(),
            comparison_body_sql: None,
            expected_columns: None,
        }],
        assertions: Vec::new(),
        sql_analysis_enabled: context.sql_analysis_enabled,
        set_difference_operator: context.set_difference_operator.clone(),
        sql_analysis_dialect: Some(context.dialect.clone()),
        probe_step_index: None,
        actual_relations: HashMap::new(),
        output_step_index: None,
    };
    Ok(PlannedResponse {
        request,
        model_names: vec![model_name],
        warnings: Vec::new(),
    })
}

impl ProjectContext {
    fn reserved_cte_name(&self, query_sql: &str) -> Option<String> {
        let cached: Option<Option<String>> = self
            .reserved_ctes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(query_sql)
            .cloned();
        if let Some(name) = cached {
            return name;
        }
        let name: Option<String> =
            reserved_cte_name(query_sql, SliceDialect::new(Some(&self.dialect)));
        self.reserved_ctes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(query_sql.to_owned(), name.clone());
        name
    }
}

/// The first CTE a model defines under a name reserved for generated test-query CTEs.
fn reserved_cte_name(query_sql: &str, dialect: SliceDialect) -> Option<String> {
    let reserved = MOCK_CTE_PREFIXES.iter().copied().chain([
        HELPER_PREFIX,
        EXPECTED_PREFIX,
        ASSERT_PREFIX,
        SQL_TEST_ACTUAL_CTE_PREFIX,
    ]);
    let prefixes: Vec<&str> = reserved.collect();
    defined_cte_keys(query_sql, dialect)
        .into_iter()
        .find(|key| is_reserved_cte_key(key, &prefixes))
}

fn is_reserved_cte_key(key: &str, prefixes: &[&str]) -> bool {
    if key == SQL_TEST_ACTUAL_CTE || key == SQL_TEST_EXPECTED_CTE {
        return true;
    }
    prefixes.iter().any(|prefix| key.starts_with(prefix))
}

fn plan_model_test(
    plan: ModelTestPlan,
    context: &ProjectContext,
) -> Result<PlannedResponse, String> {
    let mut fixtures =
        classify_fixtures(plan.authored_ctes, plan.expected_ctes, plan.assertion_ctes);
    fixtures.scope = ScopeGraph::new(&ScopeRequest {
        fixtures: &fixtures,
        patterns: &context.patterns,
        file_label: &plan.file_label,
        rename_dialect: (context.sql_analysis_enabled || context.rejects_nested_with)
            .then_some(context.parser),
        flat_dialect: context.rejects_nested_with,
        slice_dialect: SliceDialect::new(Some(&context.dialect)),
        read_helper_names: &plan.reads.read_helper_names,
    })?;
    let expected_names = chain_root_names(
        plan.expected_model_names,
        &plan.reads,
        &fixtures,
        &context.patterns,
    );
    let topo_request = TopoSortRequest {
        expected_names: &expected_names,
        models: &context.models,
        overrides: &plan.model_query_overrides,
        mock_refs: &fixtures.mock_refs,
        patterns: &context.patterns,
    };
    let ordered_names = topo_sort_model_chain(&topo_request);
    if let Some(limit) = context.max_real_models {
        let real_models: Vec<String> = ordered_names
            .iter()
            .filter(|name| context.models.contains_key(*name))
            .cloned()
            .collect();
        let chain = RealModelChain {
            test_name: &plan.test_name,
            file_label: &plan.file_label,
            roots: &expected_names,
            real_models: &real_models,
            limit,
        };
        if let Some(message) =
            real_model_limit_message(&chain, |name| real_model_inputs(&topo_request, name))
        {
            return Ok(PlannedResponse {
                request: RenderRequest {
                    chain: Vec::new(),
                    assertions: Vec::new(),
                    sql_analysis_enabled: context.sql_analysis_enabled,
                    set_difference_operator: context.set_difference_operator.clone(),
                    sql_analysis_dialect: Some(context.dialect.clone()),
                    probe_step_index: None,
                    actual_relations: HashMap::new(),
                    output_step_index: None,
                },
                model_names: real_models,
                warnings: vec![SqlTestPlanWarning {
                    model_name: None,
                    severity: "error",
                    message,
                }],
            });
        }
    }
    let mut warnings: Vec<SqlTestPlanWarning> = Vec::new();
    let mut reported_missing_mocks: HashSet<(&'static str, String)> = HashSet::new();
    let mut reachable_mocks: HashSet<String> = HashSet::new();
    let mut analysis_resolved: HashMap<String, AnalysisResolvedSql> = HashMap::new();
    let mut textual_chain: TextualChain = TextualChain::default();
    let mut chain: Vec<SqlTestChainStep> = Vec::new();

    for (model_index, model_name) in ordered_names.iter().enumerate() {
        let Some(model) = context.models.get(model_name) else {
            warnings.push(SqlTestPlanWarning {
                model_name: None,
                severity: "error",
                message: format!(
                    "test '{}' expects model '{model_name}' which does not exist",
                    plan.test_name
                ),
            });
            continue;
        };
        let query_sql = plan
            .model_query_overrides
            .get(model_name)
            .unwrap_or(&model.query_sql);
        if let Some(name) = context.reserved_cte_name(query_sql) {
            return Err(compile_error(&format!(
                "SQL test '{}' reads model '{model_name}', which defines CTE '{name}'; names \
                 starting with a SQLBuild test prefix such as __helper__ or __ref__ are reserved \
                 for the test query, so rename the CTE in the model",
                plan.file_label
            )));
        }
        let table_functions =
            resolve_table_function_fixtures(query_sql, &fixtures, &context.patterns)?;
        reachable_mocks.extend(table_functions.reached.iter().cloned());
        let analyzed = if context.sql_analysis_enabled && analysis_resolved.len() == model_index {
            analyze_and_resolve_sql(AnalysisResolutionRequest {
                query_sql: &table_functions.sql,
                fixture_ctes: &table_functions.ctes,
                fixtures: &fixtures,
                resolved_chain: &analysis_resolved,
                functions: &context.functions,
                file_label: &plan.file_label,
                parser: context.parser,
                templates: &context.analysis_templates,
                patterns: &context.patterns,
            })?
        } else {
            None
        };
        let (resolved_sql, lifted_ctes, comparison_body_sql) = if let Some(analyzed) = analyzed {
            reachable_mocks.extend(analyzed.reachable_mock_names.iter().cloned());
            let result = (
                analyzed.resolved_sql.clone(),
                analyzed.generated_ctes.clone(),
                Some(analyzed.cte_body_sql.clone()),
            );
            analysis_resolved.insert(model_name.clone(), analyzed);
            result
        } else {
            let (step, reached) = ensure_textual_chain_through(TextualChainRequest {
                ordered_names: &ordered_names[..=model_index],
                overrides: &plan.model_query_overrides,
                fixtures: &fixtures,
                context,
                chain: &mut textual_chain,
            })?;
            reachable_mocks.extend(reached);
            (step.resolved_sql, step.lifted_ctes, Some(step.body_sql))
        };
        warnings.extend(unresolved_reference_warnings(UnresolvedReferenceRequest {
            sql: &resolved_sql,
            test_name: &plan.test_name,
            model_name,
            patterns: &context.patterns,
            reported: &mut reported_missing_mocks,
        }));
        let authored_expected = fixtures.expected.get(model_name);
        let expected_lifted_ctes = match authored_expected {
            Some(sql) => {
                let scope = helper_scope_ctes(sql, &fixtures, &context.patterns, &plan.file_label)?;
                reachable_mocks.extend(scope.reached_mocks);
                scope.ctes
            }
            None => Vec::new(),
        };
        chain.push(SqlTestChainStep {
            model_name: model_name.clone(),
            resolved_sql,
            expected_columns: authored_expected
                .and_then(|sql| expected_columns(sql, context.parser)),
            expected_cte_sql: authored_expected
                .map(|sql| fixtures.scope.reader_sql(sql, &context.patterns)),
            expected_lifted_ctes,
            lifted_ctes,
            comparison_body_sql,
        });
    }

    let mut readers = ReaderContext {
        context,
        ordered_names: &ordered_names,
        overrides: &plan.model_query_overrides,
        test_name: &plan.test_name,
        file_label: &plan.file_label,
        analysis_resolved: &analysis_resolved,
        textual_chain: None,
    };
    resolve_referencing_helpers(HelperResolution {
        readers: &mut readers,
        fixtures: &mut fixtures,
        read_helper_names: &plan.reads.read_helper_names,
        chain: &mut chain,
        reachable_mocks: &mut reachable_mocks,
    })?;

    let mut assertions: Vec<SqlTestAssertionStep> = Vec::new();
    for (assertion_name, assertion_sql) in &fixtures.assertions {
        let placed_sql = fixtures.scope.reader_sql(assertion_sql, &context.patterns);
        let resolved = readers.resolve(
            &placed_sql,
            &fixtures,
            &AuthoredReader {
                cte_name: &format!("{ASSERT_PREFIX}{assertion_name}"),
                sql: assertion_sql,
            },
        )?;
        reachable_mocks.extend(resolved.chain_reached);
        reachable_mocks.extend(resolved.reached);
        let mut resolved_sql = resolved.resolved_sql;
        let mut lifted_ctes = resolved.lifted_ctes;
        let comparison_body_sql = Some(resolved.body_sql);
        let scope = helper_scope_ctes(
            assertion_sql,
            &fixtures,
            &context.patterns,
            &plan.file_label,
        )?;
        if !scope.ctes.is_empty() {
            reachable_mocks.extend(scope.reached_mocks.iter().cloned());
            lifted_ctes = merged_scoped_ctes(lifted_ctes, scope, &plan.file_label)?;
            resolved_sql = with_leading_ctes(
                &lifted_ctes,
                comparison_body_sql.as_deref().unwrap_or(&resolved_sql),
            );
        }
        assertions.push(SqlTestAssertionStep {
            name: assertion_name.clone(),
            resolved_sql,
            lifted_ctes,
            comparison_body_sql,
        });
    }
    warnings.extend(unreachable_mock_warnings(
        &plan.test_name,
        &reachable_mocks,
        &fixtures,
    ));
    let chain = omit_unrendered_step_sql(chain, &assertions);
    let model_names: Vec<String> = chain.iter().map(|step| step.model_name.clone()).collect();
    let request = RenderRequest {
        chain,
        assertions,
        sql_analysis_enabled: context.sql_analysis_enabled,
        set_difference_operator: context.set_difference_operator.clone(),
        sql_analysis_dialect: Some(context.dialect.clone()),
        probe_step_index: None,
        actual_relations: HashMap::new(),
        output_step_index: None,
    };
    Ok(PlannedResponse {
        request,
        model_names,
        warnings,
    })
}

/// The planned chain and fixtures whose helpers are resolved once every chain model is known.
struct HelperResolution<'r, 'a> {
    readers: &'r mut ReaderContext<'a>,
    fixtures: &'r mut TestFixtures,
    read_helper_names: &'r [String],
    chain: &'r mut [SqlTestChainStep],
    reachable_mocks: &'r mut HashSet<String>,
}

/// Resolve the references helpers call, then give expected rows the CTEs their helpers now read.
fn resolve_referencing_helpers(request: HelperResolution<'_, '_>) -> Result<(), String> {
    let HelperResolution {
        readers,
        fixtures,
        read_helper_names,
        chain,
        reachable_mocks,
    } = request;
    let patterns = &readers.context.patterns;
    let read_helpers = fixtures.scope.read_helper_indexes(read_helper_names);
    let mut resolved_helpers: Vec<(usize, ResolvedHelper)> = Vec::new();
    for (index, helper_sql) in fixtures.scope.referencing_helpers(&read_helpers, patterns) {
        let label = fixtures.scope.helper_name(index).to_string();
        let authored = AuthoredReader {
            cte_name: &label,
            sql: &helper_sql,
        };
        let resolved = readers.resolve(&helper_sql, fixtures, &authored)?;
        reachable_mocks.extend(resolved.chain_reached);
        resolved_helpers.push((
            index,
            ResolvedHelper {
                sql: resolved.body_sql,
                lifted_ctes: resolved.lifted_ctes,
                reached_mocks: resolved.reached,
            },
        ));
    }
    let helpers_resolved = !resolved_helpers.is_empty();
    for (index, resolved) in resolved_helpers {
        fixtures.scope.resolve_helper(index, resolved);
    }
    for step in chain {
        let Some(sql) = fixtures.expected.get(&step.model_name) else {
            continue;
        };
        let placed_sql = fixtures.scope.reader_sql(sql, patterns);
        let references = calls_reference(sql, patterns);
        if !helpers_resolved && !references {
            continue;
        }
        let scope = helper_scope_ctes(sql, fixtures, patterns, readers.file_label)?;
        reachable_mocks.extend(scope.reached_mocks.iter().cloned());
        if references {
            let label = format!("{EXPECTED_PREFIX}{}", step.model_name);
            let authored = AuthoredReader {
                cte_name: &label,
                sql,
            };
            let resolved = readers.resolve(&placed_sql, fixtures, &authored)?;
            reachable_mocks.extend(resolved.chain_reached);
            reachable_mocks.extend(resolved.reached);
            step.expected_lifted_ctes =
                merged_scoped_ctes(resolved.lifted_ctes, scope, readers.file_label)?;
            step.expected_cte_sql = Some(resolved.body_sql);
        } else {
            step.expected_lifted_ctes = scope.ctes;
            step.expected_cte_sql = Some(placed_sql);
        }
    }
    Ok(())
}

/// What resolving one test-authored query's relation references needs once the chain is planned.
struct ReaderContext<'a> {
    context: &'a ProjectContext,
    ordered_names: &'a [String],
    overrides: &'a BTreeMap<String, String>,
    test_name: &'a str,
    file_label: &'a str,
    analysis_resolved: &'a HashMap<String, AnalysisResolvedSql>,
    textual_chain: Option<TextualChain>,
}

/// One test CTE as authored, whose own reference calls must all resolve.
struct AuthoredReader<'s> {
    cte_name: &'s str,
    sql: &'s str,
}

/// One assertion or helper query with its references resolved to the CTEs standing in for them.
struct ResolvedReader {
    resolved_sql: String,
    lifted_ctes: Vec<(String, String)>,
    body_sql: String,
    reached: HashSet<String>,
    /// Mocks the shared textual chain reached when this query first needed it.
    chain_reached: HashSet<String>,
}

impl ReaderContext<'_> {
    /// Resolve a test-authored query, failing if its own CTE calls a reference nothing resolves.
    fn resolve(
        &mut self,
        sql: &str,
        fixtures: &TestFixtures,
        authored: &AuthoredReader<'_>,
    ) -> Result<ResolvedReader, String> {
        if let Some(call) = unresolvable_call(
            authored.sql,
            fixtures,
            self.ordered_names,
            &self.context.patterns,
        ) {
            return Err(unresolved_reference_error(
                self.test_name,
                self.file_label,
                authored.cte_name,
                &call,
            ));
        }
        self.resolve_references(sql, fixtures)
    }

    fn resolve_references(
        &mut self,
        sql: &str,
        fixtures: &TestFixtures,
    ) -> Result<ResolvedReader, String> {
        let context = self.context;
        let table_functions = resolve_table_function_fixtures(sql, fixtures, &context.patterns)?;
        let mut reached: HashSet<String> = table_functions.reached.clone();
        let fixture_resolved_sql = &table_functions.sql;
        let analyzed = if context.sql_analysis_enabled
            && self.analysis_resolved.len() == self.ordered_names.len()
        {
            analyze_and_resolve_sql(AnalysisResolutionRequest {
                query_sql: fixture_resolved_sql,
                fixture_ctes: &table_functions.ctes,
                fixtures,
                resolved_chain: self.analysis_resolved,
                functions: &context.functions,
                file_label: self.file_label,
                parser: context.parser,
                templates: &context.analysis_templates,
                patterns: &context.patterns,
            })?
        } else {
            None
        };
        if let Some(value) = analyzed
            && !has_unresolved_test_reference(&value.resolved_sql, &context.patterns)
        {
            reached.extend(value.reachable_mock_names);
            return Ok(ResolvedReader {
                resolved_sql: value.resolved_sql,
                lifted_ctes: value.generated_ctes,
                body_sql: value.cte_body_sql,
                reached,
                chain_reached: HashSet::new(),
            });
        }
        let mut chain_reached: HashSet<String> = HashSet::new();
        if self.textual_chain.is_none() {
            let (chain, chain_mocks) =
                build_textual_chain(self.ordered_names, self.overrides, fixtures, context)?;
            chain_reached = chain_mocks;
            self.textual_chain = Some(chain);
        }
        let chain = self
            .textual_chain
            .as_ref()
            .ok_or_else(|| planner_error("textual assertion chain is unavailable"))?;
        let (resolved, textual_reached) =
            resolve_assertion_textual_sql(AssertionResolutionRequest {
                assertion_sql: fixture_resolved_sql,
                fixtures,
                chain,
                functions: &context.functions,
                requires_flat_ctes: context.requires_derived_table_aliases,
                patterns: &context.patterns,
            })?;
        reached.extend(textual_reached);
        let lifted = with_unique_ctes(table_functions.ctes, resolved.lifted_ctes);
        Ok(ResolvedReader {
            resolved_sql: with_leading_ctes(&lifted, &resolved.body_sql),
            lifted_ctes: lifted,
            body_sql: resolved.body_sql,
            reached,
            chain_reached,
        })
    }
}

/// Drop SQL from chain steps the renderer never emits, keeping plan output linear in chain length.
fn omit_unrendered_step_sql(
    chain: Vec<SqlTestChainStep>,
    assertions: &[SqlTestAssertionStep],
) -> Vec<SqlTestChainStep> {
    let rendered_steps = rendered_chain_steps(&chain, assertions);
    chain
        .into_iter()
        .zip(rendered_steps)
        .map(|(step, rendered)| {
            if rendered {
                step
            } else {
                SqlTestChainStep {
                    model_name: step.model_name,
                    resolved_sql: String::new(),
                    expected_cte_sql: step.expected_cte_sql,
                    expected_lifted_ctes: step.expected_lifted_ctes,
                    lifted_ctes: Vec::new(),
                    comparison_body_sql: None,
                    expected_columns: step.expected_columns,
                }
            }
        })
        .collect()
}

fn ensure_textual_chain_through(
    request: TextualChainRequest<'_>,
) -> Result<(TextualStep, HashSet<String>), String> {
    let reachable: HashSet<String> = request.chain.extend(&TextualChainInputs {
        ordered_names: request.ordered_names,
        overrides: request.overrides,
        fixtures: request.fixtures,
        context: request.context,
    })?;
    let current_name = request
        .ordered_names
        .last()
        .ok_or_else(|| "textual SQL-test chain is empty".to_string())?;
    let step = request
        .chain
        .step(current_name)
        .ok_or_else(|| format!("SQL-test model '{current_name}' could not be resolved"))?;
    Ok((step, reachable))
}

fn build_textual_chain(
    ordered_names: &[String],
    overrides: &BTreeMap<String, String>,
    fixtures: &TestFixtures,
    context: &ProjectContext,
) -> Result<(TextualChain, HashSet<String>), String> {
    let mut chain: TextualChain = TextualChain::default();
    let reachable: HashSet<String> = chain.extend(&TextualChainInputs {
        ordered_names,
        overrides,
        fixtures,
        context,
    })?;
    Ok((chain, reachable))
}

fn resolve_assertion_textual_sql(
    request: AssertionResolutionRequest<'_>,
) -> Result<(TextualStep, HashSet<String>), String> {
    if request.requires_flat_ctes && leading_with_prefix_end(request.assertion_sql).is_some() {
        return Err(planner_error(
            "SQL test assertion fallback cannot safely flatten an assertion beginning with WITH",
        ));
    }
    let mut referenced: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for name in marker_names(
        &request.patterns.reference,
        &request.patterns.lexical,
        request.assertion_sql,
    ) {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(body) = request.chain.bodies.get(&name) else {
            continue;
        };
        if request.requires_flat_ctes && leading_with_prefix_end(body).is_some() {
            return Err(planner_error(
                "SQL test assertion fallback cannot safely flatten a referenced model beginning with WITH",
            ));
        }
        referenced.push(name);
    }
    let resolution = resolve_textual_sql(TextualResolutionRequest {
        query_sql: request.assertion_sql,
        fixtures: request.fixtures,
        resolved_chain: &request.chain.cte_names,
        chain_references: None,
        functions: request.functions,
        patterns: request.patterns,
    })?;
    let lifted_ctes: Vec<(String, String)> = with_unique_ctes(
        resolution.mock_ctes,
        request.chain.closure_ctes(&referenced, true),
    );
    let body_sql = resolution.sql;
    let reached = resolution.reached;
    let resolved_sql = with_leading_ctes(&lifted_ctes, &body_sql);
    Ok((
        TextualStep {
            resolved_sql,
            lifted_ctes,
            body_sql,
        },
        reached,
    ))
}

fn classify_fixtures(
    authored: Vec<SqlTestCte>,
    expected: Vec<SqlTestCte>,
    assertions: Vec<SqlTestCte>,
) -> TestFixtures {
    let mut fixtures = TestFixtures {
        mock_refs: BTreeMap::new(),
        mock_sources: BTreeMap::new(),
        mock_seeds: BTreeMap::new(),
        mock_dbt_refs: BTreeMap::new(),
        mock_table_functions: BTreeMap::new(),
        helpers: Vec::new(),
        scope: ScopeGraph::default(),
        expected: BTreeMap::new(),
        assertions: Vec::new(),
    };
    for cte in authored {
        if let Some(name) = cte.name.strip_prefix(REF_PREFIX) {
            fixtures.mock_refs.insert(name.to_string(), cte.sql_body);
        } else if let Some(name) = cte.name.strip_prefix(SOURCE_PREFIX) {
            fixtures.mock_sources.insert(name.to_string(), cte.sql_body);
        } else if let Some(name) = cte.name.strip_prefix(SEED_PREFIX) {
            fixtures.mock_seeds.insert(name.to_string(), cte.sql_body);
        } else if let Some(name) = cte.name.strip_prefix(DBT_REF_PREFIX) {
            fixtures
                .mock_dbt_refs
                .insert(name.to_string(), cte.sql_body);
        } else if let Some(name) = cte.name.strip_prefix(TABLE_FUNCTION_PREFIX) {
            fixtures
                .mock_table_functions
                .insert(name.to_string(), cte.sql_body);
        } else if !cte.name.starts_with(ASSERT_PREFIX) {
            fixtures.helpers.push(cte);
        }
    }
    for cte in expected {
        fixtures.expected.insert(
            cte.name
                .strip_prefix(EXPECTED_PREFIX)
                .unwrap_or(&cte.name)
                .to_string(),
            cte.sql_body,
        );
    }
    for cte in assertions {
        fixtures.assertions.push((
            cte.name
                .strip_prefix(ASSERT_PREFIX)
                .unwrap_or(&cte.name)
                .to_string(),
            cte.sql_body,
        ));
    }
    fixtures
}

fn topo_sort_model_chain(request: &TopoSortRequest<'_>) -> Vec<String> {
    let mut roots: Vec<String> = request.expected_names.to_vec();
    roots.sort();
    depth_first_postorder(roots, |name| real_model_inputs(request, name))
}

/// The unmocked project models one chain model reads, sorted and deduplicated.
fn real_model_inputs(request: &TopoSortRequest<'_>, name: &str) -> Vec<String> {
    let Some(model) = request.models.get(name) else {
        return Vec::new();
    };
    let mut dependencies: Vec<String> = match request.overrides.get(name) {
        Some(override_sql) => marker_names(
            &request.patterns.reference,
            &request.patterns.lexical,
            override_sql,
        ),
        None => model.model_dependencies.clone(),
    };
    dependencies.retain(|dependency| {
        !request.mock_refs.contains_key(dependency) && request.models.contains_key(dependency)
    });
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn analyze_and_resolve_sql(
    request: AnalysisResolutionRequest<'_>,
) -> Result<Option<AnalysisResolvedSql>, String> {
    let Some(template) =
        analysis_template(request.query_sql, request.parser, request.templates)?
    else {
        return Ok(None);
    };
    let mut generated_state = GeneratedCteState {
        names: template.existing_cte_names,
        ..GeneratedCteState::default()
    };
    for (name, sql) in request.fixture_ctes {
        generated_state.insert_helper(name, sql);
    }
    let mut replacements: HashMap<(&str, &str), String> = HashMap::new();
    for (function_name, referenced_name) in template.marker_calls.iter() {
        let target = if function_name == REF_FUNCTION {
            if let Some(chain_sql) = request.resolved_chain.get(referenced_name) {
                for (name, sql) in &chain_sql.generated_ctes {
                    if request.fixtures.scope.is_top_level_helper(name)
                        || name.starts_with(TABLE_FUNCTION_PREFIX)
                    {
                        generated_state.insert_helper(name, sql);
                    } else {
                        generated_state.insert(name, sql, request.file_label)?;
                    }
                }
                let generated_name = format!("{REF_PREFIX}{referenced_name}");
                generated_state.insert(
                    &generated_name,
                    &chain_sql.cte_body_sql,
                    request.file_label,
                )?;
                Some(generated_name)
            } else {
                generated_state.mock_target(MockTargetRequest {
                    prefix: REF_PREFIX,
                    referenced_name,
                    mocks: &request.fixtures.mock_refs,
                    fixtures: request.fixtures,
                    file_label: request.file_label,
                })?
            }
        } else if function_name == SOURCE_FUNCTION {
            generated_state.mock_target(MockTargetRequest {
                prefix: SOURCE_PREFIX,
                referenced_name,
                mocks: &request.fixtures.mock_sources,
                fixtures: request.fixtures,
                file_label: request.file_label,
            })?
        } else if function_name == SEED_FUNCTION {
            generated_state.mock_target(MockTargetRequest {
                prefix: SEED_PREFIX,
                referenced_name,
                mocks: &request.fixtures.mock_seeds,
                fixtures: request.fixtures,
                file_label: request.file_label,
            })?
        } else if function_name == DBT_REF_FUNCTION {
            generated_state.mock_target(MockTargetRequest {
                prefix: DBT_REF_PREFIX,
                referenced_name,
                mocks: &request.fixtures.mock_dbt_refs,
                fixtures: request.fixtures,
                file_label: request.file_label,
            })?
        } else if function_name == UDF_FUNCTION || function_name == TABLE_FUNCTION {
            request
                .functions
                .get(referenced_name)
                .map(|_| referenced_name.clone())
        } else {
            None
        };
        if let Some(target) = target {
            replacements.insert((function_name.as_str(), referenced_name.as_str()), target);
        }
    }
    let relations_resolved =
        replace_relation_markers(request.query_sql, &replacements, request.patterns);
    let functions_resolved = resolve_function_calls(
        &resolve_function_calls(
            &relations_resolved,
            request.functions,
            false,
            request.patterns,
        )?,
        request.functions,
        true,
        request.patterns,
    )?;
    let cte_body_sql = strip_statement_terminators(
        &functions_resolved,
        SliceDialect::new(Some(request.parser.name())),
    )
    .to_string();
    let resolved_sql = with_leading_ctes(&generated_state.generated, &cte_body_sql);
    Ok(Some(AnalysisResolvedSql {
        resolved_sql,
        cte_body_sql,
        generated_ctes: generated_state.generated,
        reachable_mock_names: generated_state.reachable,
    }))
}

#[cfg(test)]
thread_local! {
    static TEMPLATE_AST_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn take_template_ast_parse_count() -> usize {
    TEMPLATE_AST_PARSES.with(|count| count.replace(0))
}

pub(crate) fn analysis_template(
    query_sql: &str,
    parser: &dyn NativeDialect,
    templates: &AnalysisTemplateCache,
) -> Result<Option<AnalysisTemplate>, String> {
    cached_analysis_template(query_sql, templates, || {
        parsed_analysis_template(query_sql, parser)
    })
}

/// The template the project's parser reads from `query_sql`, if it reads one statement whole.
pub(crate) fn parsed_analysis_template(
    query_sql: &str,
    parser: &dyn NativeDialect,
) -> Option<AnalysisTemplate> {
    #[cfg(test)]
    TEMPLATE_AST_PARSES.with(|count| count.set(count.get() + 1));
    let mut statements = match parser.parse(query_sql) {
        Ok(Parsed { statements, gaps }) if gaps.is_empty() => statements,
        _ => return None,
    };
    if statements.len() != 1 {
        return None;
    }
    let expression = statements.remove(0);
    Some(AnalysisTemplate {
        existing_cte_names: top_level_cte_names(&expression),
        marker_calls: relation_marker_calls(&expression).into(),
    })
}

/// Why SQL tests cannot be planned for a project whose SQL analysis dialect has no native parser.
fn unavailable_sql_tests(dialect: Option<&str>) -> String {
    match dialect {
        Some(dialect) => format!(
            "SQL tests are unavailable for SQL analysis dialect '{dialect}', which SQLBuild has no native parser for"
        ),
        None => "SQL tests are unavailable without a SQL analysis dialect".to_owned(),
    }
}

pub(crate) fn cached_analysis_template<F>(
    query_sql: &str,
    templates: &AnalysisTemplateCache,
    initialize: F,
) -> Result<Option<AnalysisTemplate>, String>
where
    F: FnOnce() -> Option<AnalysisTemplate>,
{
    let cached = {
        let mut cache = templates.lock().map_err(|error| error.to_string())?;
        if let Some(cached) = cache.get(query_sql) {
            Arc::clone(cached)
        } else {
            let cached = Arc::new(OnceLock::new());
            cache.insert(query_sql.to_owned(), Arc::clone(&cached));
            cached
        }
    };
    Ok(cached.get_or_init(initialize).clone())
}

/// Textually resolved SQL plus the mocks it reached and the top-level mock CTEs it reads.
struct TextualResolution {
    sql: String,
    reached: HashSet<String>,
    mock_ctes: Vec<(String, String)>,
}

fn resolve_textual_sql(request: TextualResolutionRequest<'_>) -> Result<TextualResolution, String> {
    let mut reached: HashSet<String> = HashSet::new();
    let mut referenced_mocks: Vec<String> = Vec::new();
    let mut chain_references = request.chain_references;
    let mut result = replace_named_markers(
        request.query_sql,
        &request.patterns.reference,
        &request.patterns.lexical,
        |name| {
            if let Some(sql) = request.resolved_chain.get(name) {
                if let Some(references) = chain_references.as_deref_mut()
                    && !references.iter().any(|existing| existing == name)
                {
                    references.push(name.to_string());
                }
                return Some(sql.clone());
            }
            request.fixtures.mock_refs.get(name)?;
            let generated_name = format!("{REF_PREFIX}{name}");
            reached.insert(name.to_string());
            referenced_mocks.push(generated_name.clone());
            Some(generated_name)
        },
    );
    result = replace_named_markers(
        &result,
        &request.patterns.source,
        &request.patterns.lexical,
        |name| {
            request.fixtures.mock_sources.get(name)?;
            let generated_name = format!("{SOURCE_PREFIX}{name}");
            reached.insert(name.to_string());
            referenced_mocks.push(generated_name.clone());
            Some(generated_name)
        },
    );
    result = replace_named_markers(
        &result,
        &request.patterns.seed,
        &request.patterns.lexical,
        |name| {
            request.fixtures.mock_seeds.get(name)?;
            let generated_name = format!("{SEED_PREFIX}{name}");
            reached.insert(name.to_string());
            referenced_mocks.push(generated_name.clone());
            Some(generated_name)
        },
    );
    result = replace_dbt_ref_markers(
        &result,
        &request.patterns.dbt_reference,
        &request.patterns.lexical,
        |name| {
            request.fixtures.mock_dbt_refs.get(name)?;
            let generated_name = format!("{DBT_REF_PREFIX}{name}");
            reached.insert(name.to_string());
            referenced_mocks.push(generated_name.clone());
            Some(generated_name)
        },
    );
    let table_functions =
        resolve_table_function_fixtures(&result, request.fixtures, request.patterns)?;
    reached.extend(table_functions.reached);
    let mut result = resolve_function_calls(
        &table_functions.sql,
        request.functions,
        false,
        request.patterns,
    )?;
    result = resolve_function_calls(&result, request.functions, true, request.patterns)?;
    let mut mock_ctes: Vec<(String, String)> = table_functions.ctes;
    for generated_name in referenced_mocks {
        let scope = &request.fixtures.scope;
        let dependencies = scope.mock_dependencies(&generated_name);
        reached.extend(
            dependencies
                .iter()
                .filter_map(|dependency| dependency.mock_name.map(str::to_string)),
        );
        let mock_sql = scope
            .mock_sql(&generated_name)
            .unwrap_or_default()
            .to_string();
        mock_ctes = with_unique_ctes(
            mock_ctes,
            dependencies
                .into_iter()
                .map(|dependency| {
                    (
                        dependency.generated_name.to_string(),
                        dependency.sql.to_string(),
                    )
                })
                .chain(std::iter::once((generated_name, mock_sql))),
        );
    }
    Ok(TextualResolution {
        sql: result,
        reached,
        mock_ctes,
    })
}

/// Model or assertion SQL with table-function fixtures replaced by their top-level CTEs.
struct TableFunctionFixtures<'a> {
    sql: std::borrow::Cow<'a, str>,
    reached: HashSet<String>,
    ctes: Vec<(String, String)>,
}

fn resolve_table_function_fixtures<'a>(
    sql: &'a str,
    fixtures: &TestFixtures,
    patterns: &SqlTestPatterns,
) -> Result<TableFunctionFixtures<'a>, String> {
    let mut resolved = TableFunctionFixtures {
        sql: std::borrow::Cow::Borrowed(sql),
        reached: HashSet::new(),
        ctes: Vec::new(),
    };
    if fixtures.mock_table_functions.is_empty() {
        return Ok(resolved);
    }
    let (replaced, reached) = replace_callable_markers(
        sql,
        &patterns.table_function,
        &patterns.lexical,
        |name, _call_suffix| {
            fixtures
                .mock_table_functions
                .contains_key(name)
                .then(|| (format!("{TABLE_FUNCTION_PREFIX}{name}"), true))
        },
    )?;
    resolved.sql = std::borrow::Cow::Owned(replaced);
    for (name, body) in &fixtures.mock_table_functions {
        if !reached.contains(name) {
            continue;
        }
        let scope = fixtures.scope.reader_scope(body, patterns);
        resolved.reached.extend(scope.reached_mocks);
        resolved.ctes = with_unique_ctes(
            std::mem::take(&mut resolved.ctes),
            scope.ctes.into_iter().chain(std::iter::once((
                format!("{TABLE_FUNCTION_PREFIX}{name}"),
                fixtures.scope.reader_sql(body, patterns),
            ))),
        );
    }
    resolved.reached.extend(reached);
    Ok(resolved)
}

fn resolve_function_calls(
    sql: &str,
    functions: &HashMap<String, SqlTestPlanFunction>,
    table_function: bool,
    patterns: &SqlTestPatterns,
) -> Result<String, String> {
    let pattern = if table_function {
        &patterns.table_function
    } else {
        &patterns.udf
    };
    let (result, _) =
        replace_callable_markers(sql, pattern, &patterns.lexical, |name, call_suffix| {
            let function = functions.get(name)?;
            let (prefix, suffix) = if table_function {
                (
                    function.table_function_prefix.as_deref()?,
                    function.table_function_suffix.as_deref()?,
                )
            } else {
                (
                    function.udf_prefix.as_deref()?,
                    function.udf_suffix.as_deref()?,
                )
            };
            Some((format!("{prefix}{call_suffix}{suffix}"), false))
        })?;
    Ok(result)
}

fn top_level_cte_names(expression: &Expression) -> HashSet<String> {
    let Expression::Select(select) = expression else {
        return HashSet::new();
    };
    let Some(with) = select.with.as_ref() else {
        return HashSet::new();
    };
    with.ctes.iter().map(|cte| cte.alias.name.clone()).collect()
}

fn replace_relation_markers<'a>(
    sql: &'a str,
    replacements: &HashMap<(&str, &str), String>,
    patterns: &SqlTestPatterns,
) -> std::borrow::Cow<'a, str> {
    let mut result = std::borrow::Cow::Borrowed(sql);
    for (function_name, pattern) in [
        (REF_FUNCTION, &patterns.reference),
        (SOURCE_FUNCTION, &patterns.source),
        (SEED_FUNCTION, &patterns.seed),
    ] {
        if !replacements.keys().any(|(name, _)| *name == function_name) {
            continue;
        }
        result = std::borrow::Cow::Owned(replace_named_markers(
            &result,
            pattern,
            &patterns.lexical,
            |name| replacements.get(&(function_name, name)).cloned(),
        ));
    }
    if !replacements
        .keys()
        .any(|(name, _)| *name == DBT_REF_FUNCTION)
    {
        return result;
    }
    std::borrow::Cow::Owned(replace_dbt_ref_markers(
        &result,
        &patterns.dbt_reference,
        &patterns.lexical,
        |name| replacements.get(&(DBT_REF_FUNCTION, name)).cloned(),
    ))
}

struct UnresolvedReferenceRequest<'a> {
    sql: &'a str,
    test_name: &'a str,
    model_name: &'a str,
    patterns: &'a SqlTestPatterns,
    reported: &'a mut HashSet<(&'static str, String)>,
}

/// Report each unmocked reference once per test, attributed to the first step reaching it.
fn unresolved_reference_warnings(
    request: UnresolvedReferenceRequest<'_>,
) -> Vec<SqlTestPlanWarning> {
    let UnresolvedReferenceRequest {
        sql,
        test_name,
        model_name,
        patterns,
        reported,
    } = request;
    if !patterns.test_reference.is_match(sql) {
        return Vec::new();
    }
    let mut warnings: Vec<SqlTestPlanWarning> = Vec::new();
    let mut warn = |kind: &'static str, name: String, message: String| {
        if reported.insert((kind, name)) {
            warnings.push(SqlTestPlanWarning {
                model_name: Some(model_name.to_string()),
                severity: "error",
                message,
            });
        }
    };
    let mut protected = ProtectedRanges::new(&patterns.lexical, sql);
    for name in marker_names_in(&patterns.reference, &mut protected) {
        let message = format!(
            "test '{test_name}': model '{model_name}' references __ref(\"{name}\") which has no mock and is not in the expected chain"
        );
        warn(REF_FUNCTION, name, message);
    }
    for (pattern, function_name) in [
        (&patterns.source, SOURCE_FUNCTION),
        (&patterns.seed, SEED_FUNCTION),
    ] {
        for name in marker_names_in(pattern, &mut protected) {
            let message = format!(
                "test '{test_name}': model '{model_name}' references {function_name}(\"{name}\") which has no mock"
            );
            warn(function_name, name, message);
        }
    }
    for captures in patterns.dbt_reference.captures_iter(sql) {
        let Some(full) = captures.get(0) else {
            continue;
        };
        if protected.contains(full.start()) {
            continue;
        }
        let Some(first) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let name = captures.get(2).map_or_else(
            || first.to_string(),
            |second| format!("{first}__{}", second.as_str()),
        );
        let message = format!(
            "test '{test_name}': model '{model_name}' references __dbt_ref__{name} which has no mock"
        );
        warn(DBT_REF_FUNCTION, name, message);
    }
    warnings
}

fn unreachable_mock_warnings(
    test_name: &str,
    reachable: &HashSet<String>,
    fixtures: &TestFixtures,
) -> Vec<SqlTestPlanWarning> {
    let groups = [
        (
            &fixtures.mock_refs,
            REF_PREFIX,
            " is unreachable because a downstream model is also in the expected chain",
        ),
        (&fixtures.mock_sources, SOURCE_PREFIX, " is unreachable"),
        (&fixtures.mock_seeds, SEED_PREFIX, " is unreachable"),
        (&fixtures.mock_dbt_refs, DBT_REF_PREFIX, " is unreachable"),
        (
            &fixtures.mock_table_functions,
            TABLE_FUNCTION_PREFIX,
            " is unreachable",
        ),
    ];
    let mut warnings: Vec<SqlTestPlanWarning> = Vec::new();
    for (mocks, prefix, suffix) in groups {
        for name in mocks.keys().filter(|name| !reachable.contains(*name)) {
            warnings.push(SqlTestPlanWarning {
                model_name: None,
                severity: "warning",
                message: format!("test '{test_name}' mock {prefix}{name}{suffix}"),
            });
        }
    }
    warnings
}

fn has_unresolved_test_reference(sql: &str, patterns: &SqlTestPatterns) -> bool {
    if !patterns.test_reference.is_match(sql) {
        return false;
    }
    !marker_names(&patterns.reference, &patterns.lexical, sql).is_empty()
        || !marker_names(&patterns.source, &patterns.lexical, sql).is_empty()
        || !marker_names(&patterns.seed, &patterns.lexical, sql).is_empty()
        || patterns.dbt_reference.is_match(sql)
        || patterns.table_function.is_match(sql)
}

fn helper_with_clause(helpers: &[SqlTestCte]) -> String {
    if helpers.is_empty() {
        return String::new();
    }
    format!(
        "WITH {}",
        helpers
            .iter()
            .map(|cte| cte_definition_sql(&cte.name, &cte.sql_body))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Prefix every helper CTE to a direct test's actual and expected queries.
fn with_helper_ctes(sql: &str, helpers: &[SqlTestCte]) -> String {
    if helpers.is_empty() {
        sql.to_string()
    } else {
        format!("{} {sql}", helper_with_clause(helpers))
    }
}

fn dedupe(mut values: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
    values
}

fn compile_pattern(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern)
        .map_err(|error| planner_error(&format!("invalid SQL-test pattern: {error}")))
}

pub(crate) fn compile_error(message: &str) -> String {
    format!("compile_input:{message}")
}

pub(crate) fn planner_error(message: &str) -> String {
    format!("planner_input:{message}")
}
