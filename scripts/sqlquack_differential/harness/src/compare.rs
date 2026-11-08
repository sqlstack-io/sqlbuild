//! One case's SQLQuack analysis and its comparison with SQLBuild's Polyglot results.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use serde_json::{Value, json};
use sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql;
use sqlbuild_analysis::semantic_validation::main::validation_json;
use sqlbuild_analysis::semantic_validation::models::NormalizationInput;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::main::same_type::same_type;
use sqlquack_semantics::dialects::constants::{
    DUCKDB, SNOWFLAKE, SNOWFLAKE_QUOTED_IGNORING_CASE, SQLITE,
};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{
    LineageContext, Nullability, OutputLineage, QueryLineage, SchemaColumn, TransformKind,
};
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_semantics::{statement_lineage, statement_name_findings};
use sqlquack_syntax::{Dialect, Expression, ParseOptions, parse_strict};

use crate::cases::{Case, Column, PolyglotResult, Source};

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
    pub dialect: String,
    /// `ok`, `opaque` (a construct SQLQuack keeps as a command), `parse_error` or `panic`.
    pub parse: String,
    pub detail: Option<String>,
    /// Per measure: `agree`, `differ`, `refines` or `not_measured`.
    pub lineage: String,
    pub findings: String,
    pub types: String,
    /// Polyglot's compared findings, as `<code> <message>`.
    pub polyglot_findings: Vec<String>,
    /// Polyglot's other validation errors, which no measure compares.
    pub polyglot_other: Vec<String>,
    /// SQLQuack's compared findings, as `<code> <message>`.
    pub sqlquack_findings: Vec<String>,
    /// SQLQuack's other findings, such as its W2xx type findings, which no measure compares.
    pub sqlquack_other: Vec<String>,
    pub differences: Vec<Difference>,
    /// The analysis SQL both engines read.
    pub sql: String,
    /// SQLQuack's outputs as `(name, type, nullability)`.
    pub outputs: Vec<(String, Option<String>, &'static str)>,
}

/// One difference, with both sides as text.
#[derive(Debug, Serialize)]
pub struct Difference {
    pub measure: &'static str,
    pub output: String,
    pub kind: String,
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

/// Parse and analyse the case with SQLQuack and compare each measure with Polyglot's results.
pub fn compare_case(case: &Case) -> Comparison {
    let Some((dialect, profile)) = sqlquack_dialect(case) else {
        return unmeasured(
            case,
            "dialect",
            Some(case.dialect.clone()),
            (Vec::new(), Vec::new()),
        );
    };
    let sql: String = match normalize_analysis_sql(NormalizationInput {
        sql: case.query_sql.clone(),
        dialect: case.dialect.clone(),
        stubs: HashMap::new(),
        placeholders: case.placeholders.clone(),
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
    let polyglot_findings: (Vec<String>, Vec<String>) = match polyglot_validation(case, &sql) {
        Ok(findings) => findings,
        Err(error) => (Vec::new(), vec![format!("validation failed: {error}")]),
    };
    let statements: Vec<Expression> = match read(&sql, dialect) {
        Ok(statements) => statements,
        Err((parse, detail)) => return unmeasured(case, parse, Some(detail), polyglot_findings),
    };
    let [statement] = statements.as_slice() else {
        let count: usize = statements.len();
        let detail: String = format!("{count} statements");
        return unmeasured(case, "parse_error", Some(detail), polyglot_findings);
    };
    if let Expression::Command(command) = statement {
        let detail: String = format!("{command:?}").chars().take(120).collect();
        return unmeasured(case, "opaque", Some(detail), polyglot_findings);
    }
    let catalog: Catalog = catalog(case, profile);
    let context = LineageContext {
        profile,
        schema: &catalog,
        sql: Some(&sql),
    };
    let lineage: QueryLineage = statement_lineage(statement, context);
    let (compared, other): (Vec<_>, Vec<_>) = statement_name_findings(statement, context)
        .into_iter()
        .partition(|finding| is_compared(finding.code));
    let sqlquack_findings: Vec<String> = compared
        .iter()
        .map(|finding| format!("{} {}", finding.code, finding.message))
        .collect();
    let sqlquack_other: Vec<String> = other
        .iter()
        .map(|finding| format!("{} {}", finding.code, finding.message))
        .collect();
    let resources: HashMap<String, (String, String)> = resources(case, profile);
    let mut differences: Vec<Difference> = Vec::new();
    let findings_verdict: String =
        compare_findings(&polyglot_findings, &sqlquack_findings, &mut differences);
    let (lineage_verdict, types_verdict): (String, String) = match &case.polyglot {
        Some(polyglot) => (
            compare_lineage(polyglot, &lineage, &resources, &mut differences),
            compare_types(&case.dialect, polyglot, &lineage, &mut differences),
        ),
        None => ("not_measured".to_owned(), "not_measured".to_owned()),
    };
    let (polyglot_findings, polyglot_other) = polyglot_findings;
    Comparison {
        id: case.id.clone(),
        dialect: case.dialect.clone(),
        parse: "ok".to_owned(),
        detail: None,
        lineage: lineage_verdict,
        findings: findings_verdict,
        types: types_verdict,
        polyglot_findings,
        polyglot_other,
        sqlquack_findings,
        sqlquack_other,
        differences,
        outputs: lineage
            .outputs
            .iter()
            .map(|output| {
                (
                    output.name.clone(),
                    output.data_type.clone(),
                    nullability_text(output.nullability),
                )
            })
            .collect(),
        sql,
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
        dialect: case.dialect.clone(),
        parse: parse.to_owned(),
        detail,
        lineage: "not_measured".to_owned(),
        findings: "not_measured".to_owned(),
        types: "not_measured".to_owned(),
        polyglot_findings,
        polyglot_other,
        sqlquack_findings: Vec::new(),
        sqlquack_other: Vec::new(),
        differences: Vec::new(),
        sql: String::new(),
        outputs: Vec::new(),
    }
}

/// Whether a finding code is one measure C compares.
fn is_compared(code: &str) -> bool {
    code.starts_with(FINDING_PREFIX) && code != UNKNOWN_TABLE
}

/// Polyglot's validation of the normalized SQL with the options SQLBuild's binding catalog
/// uses for Postgres (`ProjectCatalog::new`): the project's functions as known, strict, with
/// type, reference and semantic checks.
/// Returns `(compared findings, other errors)` as `<code> <message>`.
fn polyglot_validation(case: &Case, sql: &str) -> Result<(Vec<String>, Vec<String>), String> {
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
        "dialect": case.dialect,
        "quoted_ignore_case": case.quoted_ignore_case,
        "schema": {"tables": tables, "strict": true},
        "options": {
            "known_functions": known_functions,
            "check_types": case.dialect != "sqlite",
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

fn catalog(case: &Case, profile: &dyn DialectProfile) -> Catalog {
    let relations: HashMap<String, Vec<SchemaColumn>> = case
        .relations
        .iter()
        .filter(|relation| !relation.open)
        .map(|relation| {
            let columns: Vec<SchemaColumn> = relation
                .columns
                .iter()
                .map(|column| {
                    let (name, quoted): (&str, bool) = unquoted(&column.name);
                    SchemaColumn {
                        quoted,
                        name: name.to_owned(),
                        data_type: column.data_type.clone(),
                        nullability: nullability(&column.nullability),
                    }
                })
                .collect();
            (relation_key(&relation.name, profile), columns)
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
                relation_key(&relation.name, profile),
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
                kind: "sources".to_owned(),
                polyglot: sources_text(sources),
                sqlquack: sources_text(&quack),
            });
        }
        if *kind != kind_text(output.kind) {
            differences.push(Difference {
                measure: "lineage",
                output: output.name.clone(),
                kind: "transform_kind".to_owned(),
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
                kind: "missing_output".to_owned(),
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
    let code = |text: &str| -> String { text.split(' ').next().unwrap_or_default().to_owned() };
    let mut codes: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for text in left.difference(&right) {
        codes.entry(code(text)).or_default().0.push((*text).clone());
    }
    for text in right.difference(&left) {
        codes.entry(code(text)).or_default().1.push((*text).clone());
    }
    for (code, (only_polyglot, only_quack)) in codes {
        let kind: String = match (only_polyglot.is_empty(), only_quack.is_empty()) {
            (false, true) => format!("only_polyglot:{code}"),
            (true, false) => format!("only_sqlquack:{code}"),
            _ => format!("message:{code}"),
        };
        differences.push(Difference {
            measure: "findings",
            output: String::new(),
            kind,
            polyglot: only_polyglot.join(" | "),
            sqlquack: only_quack.join(" | "),
        });
    }
    "differ".to_owned()
}

fn same_type_text(dialect: &str, left: &str, right: &str) -> bool {
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
    dialect: &str,
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
            (Some(left), Some(right)) if same_type_text(dialect, left, right) => None,
            (None, None) => None,
            (None, Some(_)) => Some(POLYGLOT_UNTYPED),
            (Some(_), None) => Some("sqlquack_untyped"),
            (Some(_), Some(_)) => Some("data_type"),
        };
        if let Some(kind) = kind {
            differences.push(Difference {
                measure: "types",
                output: output.name.clone(),
                kind: kind.to_owned(),
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
                }
                .to_owned(),
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
        kind: POLYGLOT_UNNAMED.to_owned(),
        polyglot: String::new(),
        sqlquack: output.name.clone(),
    }
}

/// `differ` when any difference is a disagreement, `refines` when SQLQuack only adds what
/// Polyglot left out, `agree` when there is none.
fn verdict(differences: &[Difference]) -> String {
    if differences
        .iter()
        .any(|difference| !REFINEMENTS.contains(&difference.kind.as_str()))
    {
        "differ"
    } else if differences.is_empty() {
        "agree"
    } else {
        "refines"
    }
    .to_owned()
}

/// SQLQuack's parser dialect and semantics profile for the case's binding-catalog dialect.
fn sqlquack_dialect(case: &Case) -> Option<(Dialect, &'static dyn DialectProfile)> {
    match case.dialect.as_str() {
        "snowflake" if case.quoted_ignore_case => {
            Some((Dialect::Snowflake, &SNOWFLAKE_QUOTED_IGNORING_CASE))
        }
        "snowflake" => Some((Dialect::Snowflake, &SNOWFLAKE)),
        "duckdb" => Some((Dialect::DuckDB, &DUCKDB)),
        "sqlite" => Some((Dialect::SQLite, &SQLITE)),
        _ => None,
    }
}

/// A relation name, each dot-separated part resolved as written unquoted.
fn relation_key(name: &str, profile: &dyn DialectProfile) -> String {
    name.split('.')
        .map(|part| profile.resolved_identifier(part, false))
        .collect::<Vec<String>>()
        .join(".")
}

/// A catalog column name without its double quotes, and whether it had them.
fn unquoted(name: &str) -> (&str, bool) {
    match name
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
    {
        Some(inner) => (inner, true),
        None => (name, false),
    }
}

/// The statements of `sql`, or `(parse outcome, detail)` when the parser cannot read it whole.
fn read(sql: &str, dialect: Dialect) -> Result<Vec<Expression>, (&'static str, String)> {
    #[cfg(feature = "native-duckdb")]
    if dialect == Dialect::DuckDB {
        let parsed = sqlquack_syntax_duckdb::parse_strict(sql)
            .map_err(|error| ("parse_error", format!("{error:?}")))?;
        if let Some(gap) = parsed.gaps.first() {
            return Err(("opaque", format!("{}@{}", gap.construct, gap.start)));
        }
        return Ok(parsed.statements);
    }
    #[cfg(feature = "native-sqlite")]
    if dialect == Dialect::SQLite {
        let parsed = sqlquack_syntax_sqlite::parse_strict(sql)
            .map_err(|error| ("parse_error", format!("{error:?}")))?;
        if let Some(gap) = parsed.gaps.first() {
            return Err(("opaque", format!("{}@{}", gap.construct, gap.start)));
        }
        return Ok(parsed.statements);
    }
    parse_strict(sql, dialect, &ParseOptions::default())
        .map_err(|error| ("parse_error", error.to_string()))
}
