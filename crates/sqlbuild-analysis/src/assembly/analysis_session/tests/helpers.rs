use crate::assembly::analysis_session::models::RelationTables;
use std::collections::HashMap;

use crate::assembly::analysis_session::_helpers::cte_facts::{
    LegacyAnalysis, LegacyInput, Recovery, RecoveryInput, RecoveryProfile, legacy_analysis,
    recovery,
};
use crate::assembly::analysis_session::_helpers::mappings::{ShapeTable, catalog_relations};
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;

use crate::assembly::analysis_session::main::attach_analysis_cache::attach_analysis_cache;
use crate::assembly::analysis_session::main::finish_analysis_session::finish_analysis_session;
use crate::assembly::analysis_session::main::finished_fact_models::finished_fact_models;
use crate::assembly::analysis_session::main::finished_model_facts::finished_model_facts;
use crate::assembly::analysis_session::main::run_analysis_session::run_analysis_session;
use crate::assembly::analysis_session::main::start_analysis_session::start_analysis_session;
use crate::assembly::analysis_session::main::take_analysis_cache::take_analysis_cache;
use crate::assembly::analysis_session::models::{
    AnalysisCacheStats, AnalysisSession, ColumnFact, ContractProof, DynamicFamily, FinishedSession,
    LineageRow, ModelColumns, ModelOutcome, ModelReference, ModelRequest, PivotBatchRequest,
    PivotModel, PivotOutcome, PivotTables, SessionModelAnalysis, SessionModelFacts, SessionOutcome,
    SessionRequest, SessionStep,
};
use crate::assembly::analysis_session::tests::test_types::{
    CachedRun, ColumnView, ModelSpec, RecoveredFacts,
};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::models::{CatalogInput, ProjectCatalog};

const TRANSFORM_NAMES: [&str; 6] = [
    "direct",
    "cast",
    "expression",
    "aggregation",
    "star",
    "constant",
];
const CONFIDENCE_NAMES: [&str; 3] = ["unknown", "high", "medium"];

pub(crate) fn pairs(values: &[(&str, &str)]) -> Pairs {
    values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

pub(crate) fn shapes(values: &[(&str, &[(&str, &str)])]) -> Shapes {
    values
        .iter()
        .map(|(name, shape)| ((*name).to_owned(), pairs(shape)))
        .collect()
}

/// The compile catalog holding the closed `relations` Python registered before analysis.
pub(crate) fn catalog(dialect: &str, relations: &Shapes) -> ProjectCatalog {
    ProjectCatalog::new(CatalogInput {
        dialect: dialect.to_owned(),
        quoted_ignore_case: false,
        known_functions: Vec::new(),
        known_types: Vec::new(),
        relations: catalog_relations(relations),
    })
    .expect("the catalog dialect is known")
}

/// A model reading `sources` (source names) and `refs` (model names).
pub(crate) fn model(name: &str, sql: &str, sources: &[&str], refs: &[&str]) -> ModelRequest {
    let mut references: Vec<ModelReference> = Vec::new();
    let mut lineage_references: Vec<(String, String, String)> = Vec::new();
    for source in sources {
        references.push(ModelReference {
            analysis_name: (*source).to_owned(),
            model_ref: false,
        });
        lineage_references.push((
            (*source).to_owned(),
            "source".to_owned(),
            (*source).to_owned(),
        ));
    }
    for model_ref in refs {
        references.push(ModelReference {
            analysis_name: (*model_ref).to_owned(),
            model_ref: true,
        });
        lineage_references.push((
            (*model_ref).to_owned(),
            "model".to_owned(),
            (*model_ref).to_owned(),
        ));
    }
    let mut required_names: Vec<String> = sources
        .iter()
        .chain(refs)
        .map(|name| (*name).to_owned())
        .collect();
    required_names.sort();
    ModelRequest {
        name: name.to_owned(),
        query_sql: sql.to_owned(),
        placeholders: Vec::new(),
        references,
        lineage_references,
        required_names,
        recover_cte_facts: false,
        has_set_operation: sql.contains("UNION"),
        snapshot_columns: None,
        pivot_sql: sql.to_owned(),
        dynamic_families: Vec::new(),
        sql_macro_checks: Vec::new(),
    }
}

/// A DuckDB request whose `raw_orders` source is closed and typed.
pub(crate) fn orders_request(models: Vec<ModelRequest>) -> SessionRequest {
    let raw_orders: Shapes = shapes(&[(
        "raw_orders",
        &[("order_id", "INTEGER"), ("amount", "DOUBLE")],
    )]);
    SessionRequest {
        dialect: "duckdb".to_owned(),
        case_sensitive_shapes: false,
        function_return_types: Vec::new(),
        nullability_rules: Some(Vec::new()),
        nullability_callback: None,
        rich_type_inference: true,
        forced_order: None,
        retain_relations: false,
        column_types: raw_orders.clone(),
        column_nullability: shapes(&[(
            "raw_orders",
            &[("order_id", "non_null"), ("amount", "unknown")],
        )]),
        complete_schemas: raw_orders.clone(),
        catalog_schemas: raw_orders,
        dynamic_families_by_table: Vec::new(),
        models,
    }
}

pub(crate) fn started(request: SessionRequest) -> AnalysisSession {
    let catalog: ProjectCatalog = catalog(&request.dialect, &request.catalog_schemas);
    start_analysis_session(request, &catalog).expect("the session starts")
}

/// Run to completion: the step, outcomes and session.
pub(crate) fn completed(
    session: AnalysisSession,
) -> (Vec<SessionStep>, Vec<ModelOutcome>, FinishedSession) {
    let (steps, outcome, finished, ()) = completed_with(session, |_| ());
    (steps, outcome.models, finished)
}

/// [`completed`], reading the session with `before_finish` once every wave has run.
pub(crate) fn completed_with<T>(
    mut session: AnalysisSession,
    before_finish: impl FnOnce(&mut AnalysisSession) -> T,
) -> (Vec<SessionStep>, SessionOutcome, FinishedSession, T) {
    let steps: Vec<SessionStep> =
        vec![run_analysis_session(&mut session).expect("the session runs")];
    let read: T = before_finish(&mut session);
    let (outcome, finished) = finish_analysis_session(session).expect("the session finished");
    (steps, outcome, finished, read)
}

/// Run `request` reading and filling `store`, answering deferrals as [`session_lines`] does.
pub(crate) fn cached_run(request: SessionRequest, store: NativeStore) -> CachedRun {
    let mut session: AnalysisSession = started(request);
    attach_analysis_cache(&mut session, store);
    let (steps, outcome, finished, (keys, hits, (store, stats))) =
        completed_with(session, |session| {
            let (keys, hits) = session
                .cache
                .as_ref()
                .map(|cache| (cache.keys.clone(), cache.hits.clone()))
                .unwrap_or_default();
            let taken = take_analysis_cache(session).expect("the cache stays attached");
            (keys, hits, taken)
        });
    CachedRun {
        lines: (
            steps.iter().map(step_lines).collect(),
            outcome.models.iter().map(described).collect(),
        ),
        catalog: catalog_changes(outcome, &finished),
        store,
        stats,
        keys,
        hits,
    }
}

/// The catalog changes and fact models of `request` run without a cache.
pub(crate) fn uncached_catalog(request: SessionRequest) -> (Shapes, Vec<String>, Vec<String>) {
    let (_, outcome, finished, ()) = completed_with(started(request), |_| ());
    catalog_changes(outcome, &finished)
}

fn catalog_changes(
    outcome: SessionOutcome,
    finished: &FinishedSession,
) -> (Shapes, Vec<String>, Vec<String>) {
    let mut facts: Vec<String> = finished_fact_models(finished);
    facts.sort_unstable();
    (outcome.schema_additions, outcome.analysis_names, facts)
}

/// The models' keys from a cold cached run of `request`.
pub(crate) fn model_keys(request: SessionRequest) -> Vec<Option<ContentDigest>> {
    cached_run(request, NativeStore::default()).keys
}

/// `store` with every key in `keys` holding bytes no outcome encodes to.
pub(crate) fn damaged(mut store: NativeStore, keys: &[Option<ContentDigest>]) -> NativeStore {
    for key in keys.iter().flatten() {
        store.put(*key, b"not an outcome".to_vec());
    }
    store
}

/// `name type nullability` per column, or `failed` when analysis did not succeed.
pub(crate) fn described(outcome: &ModelOutcome) -> Vec<String> {
    let analysis = &outcome.analysis;
    let mut lines: Vec<String> = analysis.columns.iter().flatten().map(column_line).collect();
    lines.push(format!(
        "succeeded={} star={}/{} diagnostics={:?}",
        analysis.analysis_succeeded,
        analysis.has_star,
        analysis.star_resolved,
        analysis
            .binding_diagnostics
            .iter()
            .map(|row| row.code.as_str())
            .collect::<Vec<_>>()
    ));
    lines
}

/// `publish name col:TYPE ...` per publication.
pub(crate) fn step_lines(step: &SessionStep) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (name, shape) in &step.publications {
        let columns: Vec<String> = shape
            .iter()
            .map(|(column, data_type)| format!("{column}:{data_type}"))
            .collect();
        lines.push(format!("publish {name} {}", columns.join(" ")));
    }
    lines
}

pub(crate) fn model_requests(models: &[ModelSpec]) -> Vec<ModelRequest> {
    models
        .iter()
        .map(|(name, sql, sources, refs)| model(name, sql, sources, refs))
        .collect()
}

/// Run `models` over the orders request: each step's lines and each model's description.
pub(crate) fn session_lines(models: &[ModelSpec]) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let (steps, outcomes, _) = completed(started(orders_request(model_requests(models))));
    (
        steps.iter().map(step_lines).collect(),
        outcomes.iter().map(described).collect(),
    )
}

/// [`session_lines`]' steps with rich type inference as given, every model's types declared empty.
pub(crate) fn session_steps(models: &[ModelSpec], rich_type_inference: bool) -> Vec<Vec<String>> {
    let mut request: SessionRequest = orders_request(model_requests(models));
    request.rich_type_inference = rich_type_inference;
    for (name, ..) in models {
        request.column_types.push(((*name).to_owned(), Vec::new()));
    }
    let (steps, _, _) = completed(started(request));
    steps.iter().map(step_lines).collect()
}

/// Run `models` and describe the facts the finished session kept for each, or `none`.
pub(crate) fn fact_lines(models: &[ModelSpec]) -> Vec<String> {
    let (_, _, finished) = completed(started(orders_request(model_requests(models))));
    models
        .iter()
        .map(|(name, ..)| {
            finished_model_facts(&finished, name).map_or("none".to_owned(), fact_line)
        })
        .collect()
}

fn fact_line(facts: &SessionModelFacts) -> String {
    let lineage: Vec<String> = facts
        .lineage
        .iter()
        .map(|(output, sources)| format!("{output}<-{sources:?}"))
        .collect();
    let columns: Option<Vec<String>> = facts
        .columns
        .as_ref()
        .map(|columns| columns.iter().map(column_line).collect());
    format!("{columns:?} {}", lineage.join(" "))
}

/// The proof and published columns a finished session keeps for `pivot_request`'s models.
pub(crate) fn kept_model_analyses(
    dialect: &str,
    sql: &str,
    family: Option<(&str, &str, &str)>,
) -> Vec<(PivotOutcome, Option<Vec<String>>)> {
    let pivots: PivotBatchRequest = pivot_request(dialect, sql, family);
    let names: [&str; 2] = ["customer_amounts", "plain"];
    let models: Vec<ModelRequest> = names
        .iter()
        .zip(pivots.models)
        .map(|(name, pivot)| ModelRequest {
            pivot_sql: pivot.sql.clone(),
            dynamic_families: pivot.families,
            ..model(name, &pivot.sql, &["raw_orders"], &[])
        })
        .collect();
    let tables: PivotTables = pivots.tables;
    let request = SessionRequest {
        dialect: tables.dialect,
        column_types: tables.column_types.clone(),
        column_nullability: tables.column_nullability,
        complete_schemas: tables.authoritative_types,
        catalog_schemas: tables.column_types,
        dynamic_families_by_table: tables.families_by_table,
        ..orders_request(models)
    };
    let (_, _, finished) = completed(started(request));
    names
        .iter()
        .map(|name| kept_line(finished.model_analysis(name)))
        .collect()
}

fn kept_line(analysis: SessionModelAnalysis<'_>) -> (PivotOutcome, Option<Vec<String>>) {
    let proof: PivotOutcome = analysis
        .dynamic_proof
        .cloned()
        .map_or(PivotOutcome::Absent, PivotOutcome::Proof);
    let columns: Option<Vec<String>> = analysis
        .columns
        .map(|columns| columns.iter().map(column_line).collect());
    (proof, columns)
}

/// Proofs over typed `raw_orders` and pivot `status_amounts`: the model, then a family-less one.
pub(crate) fn pivot_request(
    dialect: &str,
    sql: &str,
    family: Option<(&str, &str, &str)>,
) -> PivotBatchRequest {
    let raw_orders: Shapes = shapes(&[(
        "raw_orders",
        &[
            ("order_id", "INTEGER"),
            ("customer_id", "INTEGER"),
            ("status", "VARCHAR"),
            ("amount", "DOUBLE"),
        ],
    )]);
    let upstream: Shapes = shapes(&[("status_amounts", &[("customer_id", "INTEGER")])]);
    let families: Vec<DynamicFamily> = family
        .map(|(pivot_column, value_column, aggregate)| {
            amounts(pivot_column, value_column, aggregate)
        })
        .into_iter()
        .collect();
    PivotBatchRequest {
        tables: PivotTables {
            dialect: dialect.to_owned(),
            column_types: [raw_orders.clone(), upstream].concat(),
            authoritative_types: raw_orders,
            column_nullability: shapes(&[("raw_orders", &[("customer_id", "non_null")])]),
            families_by_table: vec![(
                "status_amounts".to_owned(),
                vec![amounts("status", "amount", "MAX")],
            )],
        },
        models: vec![
            PivotModel {
                sql: sql.to_owned(),
                families,
            },
            PivotModel {
                sql: "SELECT 1".to_owned(),
                families: Vec::new(),
            },
        ],
    }
}

/// A finished session holding `tables` and an analysis pool.
pub(crate) fn finished_session(tables: PivotTables) -> FinishedSession {
    FinishedSession {
        tables,
        pool: catalog("duckdb", &Vec::new()).analysis_pool(),
        models: HashMap::new(),
        dynamic_proofs: HashMap::new(),
        relations: None,
        model_names: Vec::new(),
        declared_functions: Vec::new(),
    }
}

fn amounts(pivot_column: &str, value_column: &str, aggregate: &str) -> DynamicFamily {
    DynamicFamily {
        name: "amounts".to_owned(),
        pivot_column: pivot_column.to_owned(),
        value_column: value_column.to_owned(),
        aggregate: aggregate.to_owned(),
        data_type: "DOUBLE".to_owned(),
        name_pattern: None,
    }
}

/// A failed proof with Python's reason.
pub(crate) fn failed(reason: &str) -> PivotOutcome {
    PivotOutcome::Proof(ContractProof {
        output_proven: false,
        fixed_columns: Vec::new(),
        families: Vec::new(),
        input_relations: Vec::new(),
        failure_reason: Some(reason.to_owned()),
        bare_dynamic_pivot: false,
    })
}

/// A proven output: `(name, type, nullability)` fixed columns and the `amounts` family type.
pub(crate) fn proven(
    fixed: &[(&str, &str, &str)],
    family_type: Option<&str>,
    input: &str,
    bare: bool,
) -> PivotOutcome {
    PivotOutcome::Proof(ContractProof {
        output_proven: true,
        fixed_columns: fixed
            .iter()
            .map(|(name, data_type, nullability)| ColumnFact {
                name: (*name).to_owned(),
                data_type: Some((*data_type).to_owned()),
                nullability: (*nullability).to_owned(),
            })
            .collect(),
        families: vec![("amounts".to_owned(), family_type.map(str::to_owned))],
        input_relations: vec![input.to_owned()],
        failure_reason: None,
        bare_dynamic_pivot: bare,
    })
}

/// Python's recovery over `orders` for a contract-enforced model whose filter reads NULL.
pub(crate) fn recovered_facts(sql: &str) -> Option<(Pairs, Pairs, Vec<String>, Vec<String>)> {
    let input_schemas: Shapes = shapes(&[(
        "orders",
        &[
            ("order_id", "INTEGER"),
            ("amount", "DOUBLE"),
            ("status", "VARCHAR"),
        ],
    )]);
    let rules: Pairs = pairs(&[("UPPER", "first_arg")]);
    let recovered: Recovery = recovery(&RecoveryInput {
        cleaned_sql: sql,
        input_schemas: &input_schemas,
        recover: true,
        null_filter: true,
        profile: RecoveryProfile {
            dialect: "duckdb",
            function_return_types: &Vec::new(),
            rules: Some(&rules),
            callback: None,
        },
    })
    .ok()?;
    let mut direct: Vec<String> = recovered.direct_outputs.into_iter().collect();
    direct.sort();
    let mut non_null: Vec<String> = recovered.non_null_outputs.into_iter().collect();
    non_null.sort();
    let nullability: Pairs = recovered
        .nullability
        .into_iter()
        .map(|(name, value)| (name, value.to_owned()))
        .collect();
    Some((recovered.types, nullability, direct, non_null))
}

/// The expected recovery as owned values.
pub(crate) fn expected_facts(
    facts: Option<RecoveredFacts>,
) -> Option<(Pairs, Pairs, Vec<String>, Vec<String>)> {
    facts.map(|(types, nullability, direct, non_null)| {
        (
            pairs(types),
            pairs(nullability),
            direct.iter().map(|name| (*name).to_owned()).collect(),
            non_null.iter().map(|name| (*name).to_owned()).collect(),
        )
    })
}

/// Python's legacy analysis of `sql` over typed `orders` and `customers`, as outcome lines.
pub(crate) fn legacy_lines(sql: &str) -> Option<Vec<String>> {
    let types: ShapeTable = ShapeTable::from_shapes(&shapes(&[
        (
            "orders",
            &[
                ("order_id", "INTEGER"),
                ("amount", "DOUBLE"),
                ("status", "VARCHAR"),
            ],
        ),
        ("customers", &[("customer_id", "INTEGER")]),
    ]));
    let nullability: ShapeTable = ShapeTable::from_shapes(&shapes(&[
        (
            "orders",
            &[
                ("order_id", "non_null"),
                ("amount", "unknown"),
                ("status", "unknown"),
            ],
        ),
        ("customers", &[("customer_id", "unknown")]),
    ]));
    let references: Vec<(String, String, String)> = [
        ("orders", "model", "orders"),
        ("customers", "source", "customers"),
    ]
    .iter()
    .map(|(name, kind, resource)| {
        (
            (*name).to_owned(),
            (*kind).to_owned(),
            (*resource).to_owned(),
        )
    })
    .collect();
    let rules: Pairs = pairs(&[("UPPER", "first_arg")]);
    let analysis: LegacyAnalysis = legacy_analysis(&LegacyInput {
        cleaned_sql: sql,
        lineage_references: &references,
        recover: true,
        full_nullability: false,
        types: &types,
        nullability: &nullability,
        profile: RecoveryProfile {
            dialect: "duckdb",
            function_return_types: &Vec::new(),
            rules: Some(&rules),
            callback: None,
        },
    })
    .ok()?;
    let mut lines: Vec<String> = vec![format!(
        "succeeded={} star={}",
        analysis.succeeded, analysis.has_star
    )];
    lines.extend(analysis.columns.iter().flatten().map(column_line));
    lines.extend(analysis.lineage.iter().map(legacy_lineage_line));
    Some(lines)
}

fn legacy_lineage_line(row: &LineageRow) -> String {
    let sources: Vec<String> = row
        .sources
        .iter()
        .map(|(kind, name, column)| format!("{kind}:{name}:{column}"))
        .collect();
    format!(
        "{} {} {} [{}]",
        row.output_column,
        TRANSFORM_NAMES[usize::from(row.transform_code)],
        CONFIDENCE_NAMES[usize::from(row.confidence_code)],
        sources.join(", ")
    )
}

fn column_line(column: &ColumnFact) -> String {
    format!(
        "{} {} {}",
        column.name,
        column.data_type.as_deref().unwrap_or("-"),
        column.nullability
    )
}

/// `(hits, misses, stored)` of a cached run.
pub(crate) fn cache_stats(run: &CachedRun) -> (usize, usize, usize) {
    let AnalysisCacheStats {
        hits,
        misses,
        stored,
    } = run.stats;
    (hits, misses, stored)
}

/// The request's second model, the one key tests change.
pub(crate) fn second_model(request: &mut SessionRequest) -> &mut ModelRequest {
    &mut request.models[1]
}

/// One dynamic pivot family over orders' amounts by status.
pub(crate) fn amounts_family() -> DynamicFamily {
    DynamicFamily {
        name: "amounts".to_owned(),
        pivot_column: "status".to_owned(),
        value_column: "amount".to_owned(),
        aggregate: "SUM".to_owned(),
        data_type: "DOUBLE".to_owned(),
        name_pattern: None,
    }
}

/// The key of the session's model named `name` over its relations' current facts.
pub(crate) fn current_model_key(session: &AnalysisSession, name: &str) -> ContentDigest {
    let model: usize = session
        .request
        .models
        .iter()
        .position(|model| model.name == name)
        .expect("the model is in the request");
    let relations: HashMap<&str, ContentDigest> = session
        .model_relation_names(model)
        .into_iter()
        .map(|relation| (relation, session.relation_digest(relation)))
        .collect();
    session.model_key(&ContentDigest::default(), model, &Vec::new(), &relations)
}

/// Owned copies of expected lines.
pub(crate) fn lines(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|line| (*line).to_owned()).collect()
}

/// `(name, type)` pairs as typed columns of unknown nullability.
pub(crate) fn fact_columns(pairs: &[(&str, &str)]) -> Vec<ColumnFact> {
    pairs
        .iter()
        .map(|(name, data_type)| ColumnFact {
            name: (*name).to_owned(),
            data_type: Some((*data_type).to_owned()),
            nullability: "unknown".to_owned(),
        })
        .collect()
}

/// A pivot proof with these fixed columns.
pub(crate) fn contract_proof((output_proven, fixed): (bool, &[(&str, &str)])) -> ContractProof {
    ContractProof {
        output_proven,
        fixed_columns: fact_columns(fixed),
        families: Vec::new(),
        input_relations: Vec::new(),
        failure_reason: None,
        bare_dynamic_pivot: false,
    }
}

/// Final columns as `(name, type)` pairs and the forced star, for comparison with expectations.
pub(crate) fn column_view(finished: &ModelColumns) -> ColumnView<'_> {
    (
        finished.columns.as_ref().map(|columns| {
            columns
                .iter()
                .map(|column| (column.name.as_str(), column.data_type.as_deref()))
                .collect()
        }),
        finished.proven_star,
    )
}

/// `request` with `relations`' facts added for every relation it does not analyse.
pub(crate) fn injected_relations(
    mut request: SessionRequest,
    relations: &RelationTables,
) -> SessionRequest {
    let analysed: Vec<String> = request
        .models
        .iter()
        .map(|model| model.name.clone())
        .collect();
    for (table, facts) in [
        (&mut request.column_types, &relations.types),
        (&mut request.column_nullability, &relations.nullability),
        (&mut request.complete_schemas, &relations.complete),
    ] {
        let present: Vec<String> = table.iter().map(|(name, _)| name.clone()).collect();
        table.extend(
            facts
                .iter()
                .filter(|(name, _)| !analysed.contains(name) && !present.contains(name))
                .cloned(),
        );
    }
    request.forced_order = Some((relations.dependency_ordered, relations.cyclic));
    request
}

/// The request a fast-lineage contract compile sends for `typed` over `sql`, reading `staged`.
pub(crate) fn typed_marker_request(sql: &'static str) -> SessionRequest {
    let staged: ModelSpec = (
        "staged",
        "SELECT * FROM __source(\"raw_orders\")",
        &["raw_orders"],
        &[],
    );
    let mut requests = model_requests(&[("typed", sql, &[], &["staged"]), staged]);
    requests[0].recover_cte_facts = true;
    let mut request: SessionRequest = orders_request(requests);
    request.rich_type_inference = false;
    let own = shapes(&[("typed", &[("v", "VARCHAR")])]);
    request.column_types.extend(own.clone());
    request.complete_schemas.extend(own);
    request
}

/// The described line of the first model's column `v`.
pub(crate) fn described_column_v(lines: &[Vec<String>]) -> String {
    lines[0]
        .iter()
        .find(|line| line.starts_with("v "))
        .cloned()
        .unwrap_or_else(|| format!("no v in {:?}", lines[0]))
}
