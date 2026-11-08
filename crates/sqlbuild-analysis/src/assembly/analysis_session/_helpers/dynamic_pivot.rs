//! Python's `analyze_dynamic_column_contract` over the crate's parse of the model SQL.

use std::collections::{HashMap, HashSet};

use rayon::ThreadPool;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde_json::{Map, Value};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::assembly::analysis_session::_helpers::dict_walk::{
    casefold, column_name, dict_list, identifier_name, is_single_wildcard, nested, node_key,
    parse_one_or_error, payload, relation_name, render_type, to_dict, truthy, upper,
};
use crate::assembly::analysis_session::constants::{
    ALIAS_AST_KIND, CAST_AST_KIND, DUCKDB_DIALECT, DYNAMIC_VALUE_SOURCE_KINDS, MOTHERDUCK_DIALECT,
    PIVOT_AST_KIND, QUERY_AST_KINDS, SELECT_AST_KIND, SIMPLIFIED_PIVOT_DIALECTS,
    SUPPORTED_PIVOT_DIALECTS, TYPE_PRESERVING_AGGREGATES, UNKNOWN_NULLABILITY, UNKNOWN_TYPE,
};
use crate::assembly::analysis_session::models::{
    ColumnFact, ContractProof, DynamicFamily, PivotModel, PivotOutcome, PivotTables,
};
use crate::assembly::analysis_session::types::Shapes;
use sqlbuild_core::constants::CTE_AST_KIND;

/// What an internal native failure of a pivot proof names.
const PIVOT_CONTEXT: &str = "native dynamic pivot proof";

/// Stands in for Python's `{}`: neither has a node key.
static MISSING_EXPRESSION: Value = Value::Null;

/// The relation facts and declared families a pivot proof reads.
pub(crate) struct PivotFacts<'a> {
    pub(crate) dialect: &'a str,
    pub(crate) column_types: &'a Shapes,
    pub(crate) authoritative_types: &'a Shapes,
    pub(crate) column_nullability: &'a Shapes,
    pub(crate) families_by_table: &'a [(String, Vec<DynamicFamily>)],
}

/// Each model's proof in order; the first model whose proof fails internally fails them all.
pub(crate) fn pivot_outcomes(
    models: &[PivotModel],
    tables: &PivotTables,
) -> Result<Vec<PivotOutcome>, String> {
    let facts = pivot_facts(tables);
    models
        .iter()
        .map(|model| pivot_outcome(&model.sql, &model.families, &facts))
        .collect()
}

/// Each model's proof in order on `pool`; the first internal failure in model order fails them all.
pub(crate) fn pooled_pivot_outcomes(
    pool: &ThreadPool,
    models: &[PivotModel],
    tables: &PivotTables,
) -> Result<Vec<PivotOutcome>, String> {
    let facts = pivot_facts(tables);
    let outcomes: Vec<Result<PivotOutcome, String>> = pool.install(|| {
        models
            .par_iter()
            .map(|model| pivot_outcome(&model.sql, &model.families, &facts))
            .collect()
    });
    outcomes.into_iter().collect()
}

fn pivot_facts(tables: &PivotTables) -> PivotFacts<'_> {
    PivotFacts {
        dialect: &tables.dialect,
        column_types: &tables.column_types,
        authoritative_types: &tables.authoritative_types,
        column_nullability: &tables.column_nullability,
        families_by_table: &tables.families_by_table,
    }
}

/// One CTE body by folded name; None for a CTE with column aliases.
type CteMap<'a> = HashMap<String, Option<&'a Value>>;

/// The resolved output boundary: a pivot payload or a passthrough relation.
enum Boundary<'a> {
    Pivot(&'a Map<String, Value>),
    Passthrough(&'a str),
}

/// The proof Python computes; fails on a panic or a parsed tree the proof cannot read.
pub(crate) fn pivot_outcome(
    sql: &str,
    families: &[DynamicFamily],
    facts: &PivotFacts<'_>,
) -> Result<PivotOutcome, String> {
    if families.is_empty() {
        return Ok(PivotOutcome::Absent);
    }
    let folded_dialect: String = casefold(facts.dialect);
    if !SUPPORTED_PIVOT_DIALECTS.contains(&folded_dialect.as_str()) {
        return Ok(failure(format!(
            "adapter dialect '{}' does not support compiler-proven dynamic pivots",
            facts.dialect
        )));
    }
    let parse_dialect: &str = if folded_dialect == MOTHERDUCK_DIALECT {
        DUCKDB_DIALECT
    } else {
        folded_dialect.as_str()
    };
    catch_compiler_panic(|| {
        Ok(match parsed(sql, parse_dialect)? {
            Ok(root) => proof(&root, &folded_dialect, families, facts),
            Err(error) => failure(format!("dynamic pivot SQL could not be parsed: {error}")),
        })
    })
    .map_err(|reason| native_failure(PIVOT_CONTEXT, &reason))
}

/// Python's `parse_one(...).to_dict()`, or the parse error Python reports.
fn parsed(sql: &str, dialect: &str) -> Result<Result<Value, String>, String> {
    parse_one_or_error(sql, dialect)?
        .map(|expression| to_dict(&expression))
        .map_or_else(|error| Ok(Err(error)), |value| value.map(Ok))
}

fn proof(
    root: &Value,
    dialect: &str,
    families: &[DynamicFamily],
    facts: &PivotFacts<'_>,
) -> PivotOutcome {
    if !root.is_object() {
        return failure("dynamic pivot SQL did not produce a structured query".to_owned());
    }
    let ctes: CteMap<'_> = collect_ctes(root);
    match resolve_boundary(Some(root), &ctes, &HashSet::new()) {
        None => failure(
            "output must be a wildcard projection from one supported dynamic pivot or a proven \
             dynamic-family passthrough"
                .to_owned(),
        ),
        Some(Boundary::Passthrough(name)) => passthrough(name, families, facts),
        Some(Boundary::Pivot(pivot)) => pivot_proof(
            pivot,
            &PivotContext {
                dialect,
                ctes: &ctes,
                facts,
                bare: node_key(Some(root)) == PIVOT_AST_KIND,
            },
            families,
        ),
    }
}

struct PivotContext<'a, 'b> {
    dialect: &'a str,
    ctes: &'a CteMap<'b>,
    facts: &'a PivotFacts<'a>,
    bare: bool,
}

fn passthrough(name: &str, families: &[DynamicFamily], facts: &PivotFacts<'_>) -> PivotOutcome {
    let upstream: Option<&Vec<DynamicFamily>> = casefold_lookup(facts.families_by_table, name);
    if !upstream.is_some_and(|upstream| families_equal(families, upstream)) {
        return failure(format!(
            "dynamic-family passthrough from '{name}' must redeclare the upstream families exactly"
        ));
    }
    let Some(columns) = external_schema(name, facts.column_types, facts.column_nullability) else {
        return failure(format!(
            "dynamic-family passthrough source '{name}' has no authoritative schema"
        ));
    };
    PivotOutcome::Proof(ContractProof {
        output_proven: true,
        fixed_columns: columns,
        families: families
            .iter()
            .map(|family| (family.name.clone(), Some(family.data_type.clone())))
            .collect(),
        input_relations: vec![name.to_owned()],
        failure_reason: None,
        bare_dynamic_pivot: false,
    })
}

fn pivot_proof(
    pivot: &Map<String, Value>,
    context: &PivotContext<'_, '_>,
    families: &[DynamicFamily],
) -> PivotOutcome {
    if truthy(pivot.get("unpivot")) {
        return failure("UNPIVOT does not produce a dynamic column family".to_owned());
    }
    let PivotComponents {
        pivot_columns,
        aggregates,
        dynamic,
        columns_valid,
    } = components(pivot, context.dialect);
    if !dynamic {
        return failure(
            "static pivot values must use ordinary exact column declarations".to_owned(),
        );
    }
    let [pivot_column] = pivot_columns.as_slice() else {
        return failure(
            "dynamic column contracts currently require exactly one pivot column".to_owned(),
        );
    };
    if !columns_valid {
        return failure("dynamic pivot columns must be direct column references".to_owned());
    }
    let source: Option<&Value> = pivot.get("this");
    let schema_facts = SchemaFacts {
        ctes: context.ctes,
        types: context.facts.authoritative_types,
        nullability: context.facts.column_nullability,
    };
    let Some(source_columns) = relation_schema(source, &schema_facts, &HashSet::new()) else {
        return failure(
            "dynamic pivot input does not have an authoritative, explicit schema".to_owned(),
        );
    };
    let Some(input_relation) = external_relation_name(source, context.ctes, &HashSet::new()) else {
        return failure("dynamic pivot input relation could not be resolved".to_owned());
    };
    let mut matched: HashSet<usize> = HashSet::new();
    let mut family_proofs: Vec<(String, Option<String>)> = Vec::new();
    for family in families {
        if casefold(&family.pivot_column) != casefold(pivot_column) {
            return failure(format!(
                "dynamic family '{}' declares pivot_column '{}' but the output pivot uses '{}'",
                family.name, family.pivot_column, pivot_column
            ));
        }
        let matches: Vec<(usize, Option<String>)> = aggregates
            .iter()
            .enumerate()
            .filter_map(|(index, aggregate)| {
                let (name, value, inferred) = aggregate_fact(aggregate, &source_columns);
                let same_value: bool =
                    value.is_some_and(|value| casefold(value) == casefold(&family.value_column));
                (casefold(&name) == casefold(&family.aggregate) && same_value)
                    .then_some((index, inferred))
            })
            .collect();
        let [(index, inferred)] = matches.as_slice() else {
            return failure(format!(
                "dynamic family '{}' must match exactly one {}({}) pivot aggregate",
                family.name, family.aggregate, family.value_column
            ));
        };
        if !matched.insert(*index) {
            return failure(
                "one pivot aggregate cannot satisfy multiple dynamic families".to_owned(),
            );
        }
        family_proofs.push((family.name.clone(), inferred.clone()));
    }
    if matched.len() != aggregates.len() {
        return failure(
            "every dynamic pivot aggregate must have one declared column family".to_owned(),
        );
    }
    let mut excluded: HashSet<String> = HashSet::from([casefold(pivot_column)]);
    excluded.extend(families.iter().map(|family| casefold(&family.value_column)));
    let fixed_names: Vec<String> = fixed_names(pivot, context.dialect, &excluded, &source_columns);
    let by_name: HashMap<String, &ColumnFact> = source_columns
        .iter()
        .map(|column| (casefold(&column.name), column))
        .collect();
    let mut fixed_columns: Vec<ColumnFact> = Vec::with_capacity(fixed_names.len());
    for name in fixed_names {
        let Some(source) = by_name.get(&casefold(&name)) else {
            return failure(
                "dynamic pivot grouping columns are not present in the pivot input".to_owned(),
            );
        };
        fixed_columns.push(ColumnFact {
            name,
            data_type: source.data_type.clone(),
            nullability: source.nullability.clone(),
        });
    }
    PivotOutcome::Proof(ContractProof {
        output_proven: true,
        fixed_columns,
        families: family_proofs,
        input_relations: vec![input_relation.to_owned()],
        failure_reason: None,
        bare_dynamic_pivot: context.bare,
    })
}

/// The fixed output names: DuckDB's GROUP BY columns, otherwise the unpivoted input columns.
fn fixed_names(
    pivot: &Map<String, Value>,
    dialect: &str,
    excluded: &HashSet<String>,
    source_columns: &[ColumnFact],
) -> Vec<String> {
    let group: Option<&Value> = pivot.get("group");
    if SIMPLIFIED_PIVOT_DIALECTS.contains(&dialect) && group.is_some_and(Value::is_object) {
        return dict_list(nested(group, &["group", "expressions"]))
            .into_iter()
            .filter_map(|expression| column_name(Some(expression)).map(str::to_owned))
            .collect();
    }
    source_columns
        .iter()
        .filter(|column| !excluded.contains(&casefold(&column.name)))
        .map(|column| column.name.clone())
        .collect()
}

/// Python's `_pivot_components` for one pivot clause.
struct PivotComponents<'a> {
    pivot_columns: Vec<&'a str>,
    aggregates: Vec<&'a Value>,
    /// Whether the pivot values come from a dynamic source.
    dynamic: bool,
    /// Whether every pivot column resolved to a name.
    columns_valid: bool,
}

/// Python's `_pivot_components`: pivot columns, aggregates, dynamic values, columns valid.
fn components<'a>(pivot: &'a Map<String, Value>, dialect: &str) -> PivotComponents<'a> {
    if SIMPLIFIED_PIVOT_DIALECTS.contains(&dialect) && truthy(pivot.get("using")) {
        let expressions: Vec<&Value> = dict_list(pivot.get("expressions"));
        let columns: Vec<&str> = expressions
            .iter()
            .filter_map(|expression| column_name(Some(expression)))
            .collect();
        let valid: bool = columns.len() == expressions.len();
        return PivotComponents {
            pivot_columns: columns,
            aggregates: dict_list(pivot.get("using")),
            dynamic: !truthy(pivot.get("fields")),
            columns_valid: valid,
        };
    }
    let fields: Vec<&Value> = dict_list(pivot.get("fields"));
    let columns: Vec<&str> = fields
        .iter()
        .filter_map(|field| column_name(nested(Some(field), &["in", "this"])))
        .collect();
    let dynamic: bool = fields.iter().any(|field| has_dynamic_values(field));
    let valid: bool = columns.len() == fields.len();
    PivotComponents {
        pivot_columns: columns,
        aggregates: dict_list(pivot.get("expressions")),
        dynamic,
        columns_valid: valid,
    }
}

/// Python's `_has_dynamic_value_source` for one pivot field.
fn has_dynamic_values(field: &Value) -> bool {
    dict_list(nested(Some(field), &["in", "expressions"]))
        .into_iter()
        .any(|value| DYNAMIC_VALUE_SOURCE_KINDS.contains(&node_key(Some(value))))
}

/// Python's `_aggregate_fact`: aggregate name, value column and type-preserving inferred type.
fn aggregate_fact<'a>(
    aggregate: &'a Value,
    source_columns: &[ColumnFact],
) -> (String, Option<&'a str>, Option<String>) {
    let key: &str = node_key(Some(aggregate));
    let mut name: String = upper(key);
    let Some(payload) = aggregate.get(key).and_then(Value::as_object) else {
        return (name, None, None);
    };
    if let Some(authored) = payload
        .get("name")
        .and_then(Value::as_str)
        .filter(|authored| !authored.is_empty())
    {
        name = upper(authored);
    }
    let preserving: bool = TYPE_PRESERVING_AGGREGATES.contains(&name.as_str());
    let value: Option<&Value> = payload.get("this");
    let mut value_name: Option<&str> = column_name(value);
    let mut inferred: Option<String> = None;
    let cast: Option<&Map<String, Value>> = value
        .filter(|value| node_key(Some(value)) == CAST_AST_KIND)
        .and_then(|value| value.get(CAST_AST_KIND))
        .and_then(Value::as_object);
    if let Some(cast) = cast {
        value_name = column_name(cast.get("this"));
        if preserving {
            inferred = render_type(cast.get("to"));
        }
    }
    if inferred.is_none() && preserving {
        let by_name: HashMap<String, &ColumnFact> = source_columns
            .iter()
            .map(|column| (casefold(&column.name), column))
            .collect();
        inferred = value_name
            .and_then(|value| by_name.get(&casefold(value)))
            .and_then(|column| column.data_type.clone());
    }
    (name, value_name, inferred)
}

fn resolve_boundary<'a>(
    node: Option<&'a Value>,
    ctes: &CteMap<'a>,
    seen: &HashSet<String>,
) -> Option<Boundary<'a>> {
    let key: &str = node_key(node);
    let payload: &Map<String, Value> = payload(node)?;
    if key == PIVOT_AST_KIND {
        return Some(Boundary::Pivot(payload));
    }
    if QUERY_AST_KINDS.contains(&key) {
        return resolve_boundary(payload.get("this"), ctes, seen);
    }
    if key != SELECT_AST_KIND
        || truthy(payload.get("joins"))
        || !is_single_wildcard(payload.get("expressions"))
    {
        return None;
    }
    let relations: Vec<&Value> =
        dict_list(payload.get("from").and_then(|from| from.get("expressions")));
    let [relation] = relations.as_slice() else {
        return None;
    };
    if node_key(Some(relation)) == PIVOT_AST_KIND {
        return relation
            .get(PIVOT_AST_KIND)
            .and_then(Value::as_object)
            .map(Boundary::Pivot);
    }
    let name: &str = relation_name(Some(relation))?;
    let folded: String = casefold(name);
    let Some(cte) = ctes.get(&folded) else {
        return Some(Boundary::Passthrough(name));
    };
    let cte: &Value = (*cte)?;
    if seen.contains(&folded) {
        return None;
    }
    let mut next_seen: HashSet<String> = seen.clone();
    next_seen.insert(folded);
    resolve_boundary(Some(cte), ctes, &next_seen)
}

/// The relation facts schema lookups read.
struct SchemaFacts<'a, 'b> {
    ctes: &'a CteMap<'b>,
    types: &'a Shapes,
    nullability: &'a Shapes,
}

fn relation_schema(
    node: Option<&Value>,
    facts: &SchemaFacts<'_, '_>,
    seen: &HashSet<String>,
) -> Option<Vec<ColumnFact>> {
    let name: &str = relation_name(node)?;
    let folded: String = casefold(name);
    let Some(cte) = facts.ctes.get(&folded) else {
        return external_schema(name, facts.types, facts.nullability);
    };
    let cte: &Value = (*cte)?;
    if seen.contains(&folded) {
        return None;
    }
    let mut next_seen: HashSet<String> = seen.clone();
    next_seen.insert(folded);
    select_schema(Some(cte), facts, &next_seen)
}

fn external_relation_name<'a>(
    node: Option<&'a Value>,
    ctes: &CteMap<'a>,
    seen: &HashSet<String>,
) -> Option<&'a str> {
    let key: &str = node_key(node);
    if let Some(payload) = payload(node) {
        if QUERY_AST_KINDS.contains(&key) {
            return external_relation_name(payload.get("this"), ctes, seen);
        }
        if key == SELECT_AST_KIND {
            if truthy(payload.get("joins")) {
                return None;
            }
            let relations: Vec<&Value> =
                dict_list(payload.get("from").and_then(|from| from.get("expressions")));
            let [relation] = relations.as_slice() else {
                return None;
            };
            return external_relation_name(Some(relation), ctes, seen);
        }
    }
    let name: &str = relation_name(node)?;
    let folded: String = casefold(name);
    let Some(cte) = ctes.get(&folded) else {
        return Some(name);
    };
    let cte: &Value = (*cte)?;
    if seen.contains(&folded) {
        return None;
    }
    let mut next_seen: HashSet<String> = seen.clone();
    next_seen.insert(folded);
    external_relation_name(Some(cte), ctes, &next_seen)
}

fn select_schema(
    node: Option<&Value>,
    facts: &SchemaFacts<'_, '_>,
    seen: &HashSet<String>,
) -> Option<Vec<ColumnFact>> {
    let key: &str = node_key(node);
    let payload: &Map<String, Value> = payload(node)?;
    if QUERY_AST_KINDS.contains(&key) {
        return select_schema(payload.get("this"), facts, seen);
    }
    if key != SELECT_AST_KIND || truthy(payload.get("joins")) {
        return None;
    }
    let relations: Vec<&Value> =
        dict_list(payload.get("from").and_then(|from| from.get("expressions")));
    let [relation] = relations.as_slice() else {
        return None;
    };
    let source: Vec<ColumnFact> = relation_schema(Some(relation), facts, seen)?;
    let expressions: Option<&Value> = payload.get("expressions");
    if is_single_wildcard(expressions) {
        return Some(source);
    }
    let by_name: HashMap<String, &ColumnFact> = source
        .iter()
        .map(|column| (casefold(&column.name), column))
        .collect();
    let mut output: Vec<ColumnFact> = Vec::new();
    for expression in expressions?.as_array()? {
        if !expression.is_object() {
            return None;
        }
        output.push(projected_column(expression, &by_name)?);
    }
    Some(output)
}

/// One projection of a CTE body over its single source relation.
fn projected_column(
    projection: &Value,
    by_name: &HashMap<String, &ColumnFact>,
) -> Option<ColumnFact> {
    let (output_name, expression) = projection_parts(projection);
    let (source_name, cast_type): (Option<&str>, Option<String>) =
        if node_key(Some(expression)) == CAST_AST_KIND {
            let cast: &Map<String, Value> = expression.get(CAST_AST_KIND)?.as_object()?;
            (column_name(cast.get("this")), render_type(cast.get("to")))
        } else {
            (column_name(Some(expression)), None)
        };
    let source: &ColumnFact = by_name.get(&casefold(source_name?))?;
    Some(ColumnFact {
        name: output_name?.to_owned(),
        data_type: cast_type.or_else(|| source.data_type.clone()),
        nullability: source.nullability.clone(),
    })
}

/// Python's `_projection`: the output name and the aliased expression, `{}` when absent.
fn projection_parts(node: &Value) -> (Option<&str>, &Value) {
    if node_key(Some(node)) != ALIAS_AST_KIND {
        return (column_name(Some(node)), node);
    }
    let Some(alias) = node.get(ALIAS_AST_KIND).and_then(Value::as_object) else {
        return (None, node);
    };
    let expression: &Value = alias
        .get("this")
        .filter(|expression| expression.is_object())
        .unwrap_or(&MISSING_EXPRESSION);
    (identifier_name(alias.get("alias")), expression)
}

fn collect_ctes(root: &Value) -> CteMap<'_> {
    let mut ctes: CteMap<'_> = HashMap::new();
    let Some(with) = payload(Some(root)).and_then(|payload| payload.get("with")) else {
        return ctes;
    };
    if !with.is_object() {
        return ctes;
    }
    for cte in dict_list(with.get("ctes")) {
        let body: &Value = if node_key(Some(cte)) == CTE_AST_KIND {
            match cte.get(CTE_AST_KIND) {
                Some(body) => body,
                None => continue,
            }
        } else {
            cte
        };
        let Some(body) = body.as_object() else {
            continue;
        };
        let Some(name) = identifier_name(body.get("alias")) else {
            continue;
        };
        let query: Option<&Value> = body.get("this");
        if truthy(body.get("columns")) {
            ctes.insert(casefold(name), None);
        } else if let Some(query) = query.filter(|query| query.is_object()) {
            ctes.insert(casefold(name), Some(query));
        }
    }
    ctes
}

/// Python's `_external_schema`: the relation's typed columns with their nullability.
fn external_schema(name: &str, types: &Shapes, nullability: &Shapes) -> Option<Vec<ColumnFact>> {
    let types = casefold_lookup(types, name)?;
    let mut by_name: HashMap<String, &str> = HashMap::new();
    for (column, value) in casefold_lookup(nullability, name).into_iter().flatten() {
        by_name.insert(casefold(column), value);
    }
    Some(
        types
            .iter()
            .map(|(column, data_type)| ColumnFact {
                name: column.clone(),
                data_type: (data_type != UNKNOWN_TYPE).then(|| data_type.clone()),
                nullability: by_name
                    .get(&casefold(column))
                    .map_or(UNKNOWN_NULLABILITY, |value| value)
                    .to_owned(),
            })
            .collect(),
    )
}

/// Python's `_casefold_lookup`: the first entry whose folded name matches.
fn casefold_lookup<'a, T>(entries: &'a [(String, T)], key: &str) -> Option<&'a T> {
    let requested: String = casefold(key);
    entries
        .iter()
        .find(|(candidate, _)| casefold(candidate) == requested)
        .map(|(_, value)| value)
}

fn families_equal(left: &[DynamicFamily], right: &[DynamicFamily]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| normalized(left) == normalized(right))
}

fn failure(reason: String) -> PivotOutcome {
    PivotOutcome::Proof(ContractProof {
        output_proven: false,
        fixed_columns: Vec::new(),
        families: Vec::new(),
        input_relations: Vec::new(),
        failure_reason: Some(reason),
        bare_dynamic_pivot: false,
    })
}

/// Python's `_families_equal` key for one family.
#[derive(PartialEq, Eq)]
struct FamilyKey<'a> {
    name: String,
    pivot_column: String,
    value_column: String,
    aggregate: String,
    data_type: String,
    name_pattern: Option<&'a str>,
}

/// Python's `_families_equal` key for one family.
fn normalized(family: &DynamicFamily) -> FamilyKey<'_> {
    FamilyKey {
        name: casefold(&family.name),
        pivot_column: casefold(&family.pivot_column),
        value_column: casefold(&family.value_column),
        aggregate: casefold(&family.aggregate),
        data_type: casefold(&family.data_type),
        name_pattern: family.name_pattern.as_deref(),
    }
}
