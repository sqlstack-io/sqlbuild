//! One case's SQLQuack analysis and its comparison with SQLBuild's Polyglot results.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use serde_json::{Value, json};
use sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql;
use sqlbuild_analysis::semantic_validation::main::validation_json;
use sqlbuild_analysis::semantic_validation::models::NormalizationInput;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::main::same_type::same_type;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{
    LineageContext, Nullability, OutputLineage, QueryLineage, SchemaColumn, TransformKind,
};
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_semantics::{statement_lineage, statement_name_findings};
use sqlquack_syntax::Expression;

use crate::cases::{Case, Column, PolyglotResult, Source};
use crate::engine::{Engine, Parsed};
use crate::fingerprint::compare_fingerprint;

/// Binding findings both engines report are `E2xx`; Polyglot's unknown-table `E200` comes
/// from relations the capture has no closed shape for, which the harness leaves out.
const FINDING_PREFIX: &str = "E2";
const UNKNOWN_TABLE: &str = "E200";
const POLYGLOT_UNNAMED: &str = "polyglot_unnamed";
/// A type Polyglot left unknown where SQLQuack inferred one.
const POLYGLOT_UNTYPED: &str = "polyglot_untyped";
/// Nullability Polyglot left unknown where SQLQuack inferred it.
const POLYGLOT_UNKNOWN_NULLABILITY: &str = "polyglot_unknown_nullability";
/// Differences where SQLQuack only adds what Polyglot left out; they do not make a measure differ.
const REFINEMENTS: [&str; 3] = [
    POLYGLOT_UNNAMED,
    POLYGLOT_UNTYPED,
    POLYGLOT_UNKNOWN_NULLABILITY,
];

/// The comparison of one case.
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub id: String,
    /// `ok`, `opaque` (a construct SQLQuack keeps as a command), `parse_error` or `panic`.
    pub parse: String,
    pub detail: Option<String>,
    /// Per measure: `agree`, `differ`, `refines` or `not_measured`.
    pub lineage: String,
    pub findings: String,
    pub types: String,
    /// Whether SQLBuild's migration fingerprint is the same from either parser.
    pub fingerprint: String,
    /// Polyglot's compared findings, as `<code> <message>`.
    pub polyglot_findings: Vec<String>,
    /// Polyglot's other validation errors, which no measure compares.
    pub polyglot_other: Vec<String>,
    /// SQLQuack's compared findings, as `<code> <message>`.
    pub sqlquack_findings: Vec<String>,
    pub differences: Vec<Difference>,
}

/// One difference, with both sides as text.
#[derive(Debug, Serialize)]
pub struct Difference {
    pub measure: &'static str,
    pub output: String,
    pub kind: &'static str,
    pub polyglot: String,
    pub sqlquack: String,
}

/// SQLBuild's analysis-name stubs for a project function, as `function_names` declares them.
const UDF_PREFIX: &str = "__sqlbuild_udf_";
const TABLE_FUNCTION_PREFIX: &str = "__sqlbuild_table_function_";

/// The catalog the compile knew: relations by resolved name, and the project's functions by
/// their analysis names in lower case, as SQLBuild hands them to Polyglot.
#[derive(Debug, Default)]
struct Catalog {
    relations: HashMap<String, Vec<SchemaColumn>>,
    /// Known function names, with the return type of each scalar UDF's stub.
    functions: HashMap<String, Option<String>>,
}

impl LineageSchema for Catalog {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.relations.get(relation).map(Vec::as_slice)
    }

    fn function_type(&self, name: &str) -> Option<&str> {
        self.functions.get(&name.to_lowercase())?.as_deref()
    }

    fn known_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_lowercase())
    }
}

/// A comparison for a case whose analysis panicked.
pub fn panicked(case: &Case) -> Comparison {
    unmeasured(case, "panic", None, (Vec::new(), Vec::new()))
}

/// Compare every measure of the case, the migration fingerprint included.
pub fn compare_case(case: &Case) -> Comparison {
    let engine: Engine = match Engine::from_polyglot(&case.dialect) {
        Ok(engine) => engine,
        Err(error) => return unmeasured(case, "dialect", Some(error), (Vec::new(), Vec::new())),
    };
    let mut comparison: Comparison = analysed_case(case, engine);
    comparison.fingerprint =
        compare_fingerprint(engine, &case.query_sql, &mut comparison.differences);
    comparison
}

/// Parse and analyse the case with SQLQuack and compare each measure with Polyglot's results.
fn analysed_case(case: &Case, engine: Engine) -> Comparison {
    let sql: String = match normalize_analysis_sql(NormalizationInput {
        sql: case.query_sql.clone(),
        dialect: engine.analysis_name().to_owned(),
        stubs: HashMap::new(),
        placeholders: HashMap::new(),
    }) {
        Ok(sql) => sql,
        Err(error) => {
            return unmeasured(
                case,
                "normalize_error",
                Some(error),
                (Vec::new(), Vec::new()),
            );
        }
    };
    let polyglot_findings: (Vec<String>, Vec<String>) =
        match polyglot_validation(case, engine, &sql) {
            Ok(findings) => findings,
            Err(error) => (Vec::new(), vec![format!("validation failed: {error}")]),
        };
    let statement: Expression = match engine.parse(&sql) {
        Parsed::Statement(statement) => statement,
        Parsed::Unmeasured(reason, detail) => {
            return unmeasured(case, reason, Some(detail), polyglot_findings);
        }
    };
    let statement: &Expression = &statement;
    let profile: &'static dyn DialectProfile = engine.profile();
    let catalog: Catalog = catalog(case, engine);
    let context = LineageContext {
        profile,
        schema: &catalog,
        sql: Some(&sql),
    };
    let lineage: QueryLineage = statement_lineage(statement, context);
    let sqlquack_findings: Vec<String> = statement_name_findings(statement, context)
        .iter()
        .filter(|finding| is_compared(finding.code))
        .map(|finding| format!("{} {}", finding.code, finding.message))
        .collect();
    let resources: HashMap<String, (String, String)> = resources(case, profile);
    let mut differences: Vec<Difference> = Vec::new();
    let findings_verdict: String =
        compare_findings(&polyglot_findings, &sqlquack_findings, &mut differences);
    let (lineage_verdict, types_verdict): (String, String) = match &case.polyglot {
        Some(polyglot) => (
            compare_lineage(polyglot, &lineage, &resources, &mut differences),
            compare_types(engine, polyglot, &lineage, &mut differences),
        ),
        None => ("not_measured".to_owned(), "not_measured".to_owned()),
    };
    let (polyglot_findings, polyglot_other) = polyglot_findings;
    Comparison {
        id: case.id.clone(),
        parse: "ok".to_owned(),
        detail: None,
        lineage: lineage_verdict,
        findings: findings_verdict,
        types: types_verdict,
        fingerprint: "not_measured".to_owned(),
        polyglot_findings,
        polyglot_other,
        sqlquack_findings,
        differences,
    }
}

fn unmeasured(
    case: &Case,
    parse: &str,
    detail: Option<String>,
    polyglot_findings: (Vec<String>, Vec<String>),
) -> Comparison {
    let (polyglot_findings, polyglot_other) = polyglot_findings;
    Comparison {
        id: case.id.clone(),
        parse: parse.to_owned(),
        detail,
        lineage: "not_measured".to_owned(),
        findings: "not_measured".to_owned(),
        types: "not_measured".to_owned(),
        fingerprint: "not_measured".to_owned(),
        polyglot_findings,
        polyglot_other,
        sqlquack_findings: Vec::new(),
        differences: Vec::new(),
    }
}

/// Whether a finding code is one measure C compares.
fn is_compared(code: &str) -> bool {
    code.starts_with(FINDING_PREFIX) && code != UNKNOWN_TABLE
}

/// Polyglot's validation of the normalized SQL with the options SQLBuild's binding catalog
/// uses for every dialect (`ProjectCatalog::new`): the project's functions as known, strict, with
/// type, reference and semantic checks.
/// Returns `(compared findings, other errors)` as `<code> <message>`.
fn polyglot_validation(
    case: &Case,
    engine: Engine,
    sql: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let tables: Vec<Value> = case
        .relations
        .iter()
        .filter(|relation| !relation.open)
        .map(|relation| {
            let columns: Vec<Value> = relation
                .columns
                .iter()
                .map(|column| {
                    json!({
                        "name": column.name,
                        "type": column.data_type.clone().unwrap_or_default(),
                    })
                })
                .collect();
            json!({"name": relation.name, "columns": columns})
        })
        .collect();
    let known_functions: Vec<String> = case
        .functions
        .iter()
        .flat_map(|function| {
            [
                function.name.clone(),
                format!("{UDF_PREFIX}{}", function.name),
                format!("{TABLE_FUNCTION_PREFIX}{}", function.name),
            ]
        })
        .collect();
    let request: Value = json!({
        "sql": sql,
        "dialect": engine.polyglot_name(),
        "schema": {"tables": tables, "strict": true},
        "options": {
            "known_functions": known_functions,
            "check_types": true,
            "check_references": true,
            "semantic": true,
            "strict": true,
            "complexity_guard": {"maxFunctionCallDepth": 512},
        },
    });
    let result: Value = serde_json::from_str(&validation_json(&request.to_string())?)
        .map_err(|error| error.to_string())?;
    let mut names: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    for error in result["errors"].as_array().into_iter().flatten() {
        let code: &str = error["code"].as_str().unwrap_or_default();
        let text: String = format!("{code} {}", error["message"].as_str().unwrap_or_default());
        if is_compared(code) {
            names.push(text);
        } else {
            other.push(text);
        }
    }
    Ok((names, other))
}

fn catalog(case: &Case, engine: Engine) -> Catalog {
    let profile: &dyn DialectProfile = engine.profile();
    let relations: HashMap<String, Vec<SchemaColumn>> = case
        .relations
        .iter()
        .filter(|relation| !relation.open)
        .map(|relation| {
            let columns: Vec<SchemaColumn> = relation
                .columns
                .iter()
                .map(|column| SchemaColumn {
                    quoted: engine.exact_name(&column.name),
                    name: column.name.clone(),
                    data_type: column.data_type.clone(),
                    nullability: nullability(&column.nullability),
                })
                .collect();
            (profile.resolved_identifier(&relation.name, false), columns)
        })
        .collect();
    let mut functions: HashMap<String, Option<String>> = HashMap::new();
    for function in &case.functions {
        let name: String = function.name.to_lowercase();
        functions.insert(format!("{UDF_PREFIX}{name}"), function.returns.clone());
        functions.insert(format!("{TABLE_FUNCTION_PREFIX}{name}"), None);
        functions.insert(name, None);
    }
    Catalog {
        relations,
        functions,
    }
}

fn resources(case: &Case, profile: &dyn DialectProfile) -> HashMap<String, (String, String)> {
    case.relations
        .iter()
        .filter_map(|relation| {
            Some((
                profile.resolved_identifier(&relation.name, false),
                (
                    relation.resource_type.clone()?,
                    relation.resource_name.clone()?,
                ),
            ))
        })
        .collect()
}

fn nullability(text: &str) -> Nullability {
    match text {
        "non_null" => Nullability::NonNull,
        "nullable" => Nullability::Nullable,
        _ => Nullability::Unknown,
    }
}

fn nullability_text(nullability: Nullability) -> &'static str {
    match nullability {
        Nullability::NonNull => "non_null",
        Nullability::Nullable => "nullable",
        Nullability::Unknown => "unknown",
    }
}

fn kind_text(kind: TransformKind) -> &'static str {
    match kind {
        TransformKind::Constant => "constant",
        TransformKind::Direct => "direct",
        TransformKind::Cast => "cast",
        TransformKind::Expression => "expression",
        TransformKind::Window => "window",
        TransformKind::Aggregation => "aggregation",
    }
}

fn sources_text(sources: &BTreeSet<Source>) -> String {
    sources
        .iter()
        .map(|source| {
            format!(
                "{}:{}.{}",
                source.resource_type, source.resource_name, source.column
            )
        })
        .collect::<Vec<String>>()
        .join(", ")
}

fn quack_sources(
    output: &OutputLineage,
    resources: &HashMap<String, (String, String)>,
) -> BTreeSet<Source> {
    output
        .sources
        .iter()
        .filter_map(|source| {
            let (resource_type, resource_name) = resources.get(&source.relation)?;
            Some(Source {
                resource_type: resource_type.clone(),
                resource_name: resource_name.clone(),
                column: source.column.clone(),
            })
        })
        .collect()
}

/// B: per output, the kind and the upstream resource columns.
fn compare_lineage(
    polyglot_result: &PolyglotResult,
    lineage: &QueryLineage,
    resources: &HashMap<String, (String, String)>,
    differences: &mut Vec<Difference>,
) -> String {
    if polyglot_result.has_star || lineage.open_star || polyglot_result.lineage.is_empty() {
        return "not_measured".to_owned();
    }
    let polyglot: BTreeMap<&str, (&str, BTreeSet<Source>)> = polyglot_result
        .lineage
        .iter()
        .map(|column| {
            (
                column.output.as_str(),
                (
                    column.kind.as_str(),
                    column.sources.iter().cloned().collect(),
                ),
            )
        })
        .collect();
    let before: usize = differences.len();
    for output in &lineage.outputs {
        let quack: BTreeSet<Source> = quack_sources(output, resources);
        let Some((kind, sources)) = polyglot.get(output.name.as_str()) else {
            differences.push(unnamed_output("lineage", output));
            continue;
        };
        if *sources != quack {
            differences.push(Difference {
                measure: "lineage",
                output: output.name.clone(),
                kind: "sources",
                polyglot: sources_text(sources),
                sqlquack: sources_text(&quack),
            });
        }
        if *kind != kind_text(output.kind) {
            differences.push(Difference {
                measure: "lineage",
                output: output.name.clone(),
                kind: "transform_kind",
                polyglot: (*kind).to_owned(),
                sqlquack: kind_text(output.kind).to_owned(),
            });
        }
    }
    let names: BTreeSet<&str> = lineage.outputs.iter().map(|o| o.name.as_str()).collect();
    for name in polyglot.keys() {
        if !names.contains(name) {
            differences.push(Difference {
                measure: "lineage",
                output: (*name).to_owned(),
                kind: "missing_output",
                polyglot: "present".to_owned(),
                sqlquack: "absent".to_owned(),
            });
        }
    }
    verdict(&differences[before..])
}

/// C: the binding findings, by code and message.
fn compare_findings(
    polyglot: &(Vec<String>, Vec<String>),
    quack: &[String],
    differences: &mut Vec<Difference>,
) -> String {
    let left: BTreeSet<&String> = polyglot.0.iter().collect();
    let right: BTreeSet<&String> = quack.iter().collect();
    if left == right {
        return "agree".to_owned();
    }
    differences.push(Difference {
        measure: "findings",
        output: String::new(),
        kind: "name_findings",
        polyglot: polyglot.0.join(" | "),
        sqlquack: quack.join(" | "),
    });
    "differ".to_owned()
}

fn same_type_text(engine: Engine, left: &str, right: &str) -> bool {
    let dialect: &str = engine.analysis_name();
    match (
        normalize_type(left, dialect),
        normalize_type(right, dialect),
    ) {
        (Ok(left), Ok(right)) => same_type(&left.normalized, &right.normalized, dialect),
        _ => left.eq_ignore_ascii_case(right),
    }
}

/// D: per output, the inferred type and nullability.
fn compare_types(
    engine: Engine,
    polyglot_result: &PolyglotResult,
    lineage: &QueryLineage,
    differences: &mut Vec<Difference>,
) -> String {
    let Some(columns) = &polyglot_result.columns else {
        return "not_measured".to_owned();
    };
    let polyglot: BTreeMap<&str, &Column> = columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();
    let before: usize = differences.len();
    for output in &lineage.outputs {
        let Some(column) = polyglot.get(output.name.as_str()) else {
            differences.push(unnamed_output("types", output));
            continue;
        };
        let kind: Option<&'static str> = match (&column.data_type, &output.data_type) {
            (Some(left), Some(right)) if same_type_text(engine, left, right) => None,
            (None, None) => None,
            (None, Some(_)) => Some(POLYGLOT_UNTYPED),
            (Some(_), None) => Some("sqlquack_untyped"),
            (Some(_), Some(_)) => Some("data_type"),
        };
        if let Some(kind) = kind {
            differences.push(Difference {
                measure: "types",
                output: output.name.clone(),
                kind,
                polyglot: column.data_type.clone().unwrap_or_default(),
                sqlquack: output.data_type.clone().unwrap_or_default(),
            });
        }
        let quack: &str = nullability_text(output.nullability);
        if column.nullability != quack {
            differences.push(Difference {
                measure: "types",
                output: output.name.clone(),
                kind: if column.nullability == "unknown" {
                    POLYGLOT_UNKNOWN_NULLABILITY
                } else {
                    "nullability"
                },
                polyglot: column.nullability.clone(),
                sqlquack: quack.to_owned(),
            });
        }
    }
    verdict(&differences[before..])
}

/// An output SQLQuack names and Polyglot's results lack: Polyglot names only aliased and
/// column outputs, where SQLQuack follows Postgres's output naming.
fn unnamed_output(measure: &'static str, output: &OutputLineage) -> Difference {
    Difference {
        measure,
        output: output.name.clone(),
        kind: POLYGLOT_UNNAMED,
        polyglot: String::new(),
        sqlquack: output.name.clone(),
    }
}

/// `differ` when any difference is a disagreement, `refines` when SQLQuack only adds what
/// Polyglot left out, `agree` when there is none.
fn verdict(differences: &[Difference]) -> String {
    if differences
        .iter()
        .any(|difference| !REFINEMENTS.contains(&difference.kind))
    {
        "differ"
    } else if differences.is_empty() {
        "agree"
    } else {
        "refines"
    }
    .to_owned()
}
