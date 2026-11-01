use crate::sqlquack_analysis::main::compile_lineage::compile_lineage;
use crate::sqlquack_analysis::models::{
    CompileFunctions, CompileLineageColumn, CompiledLineage, CompiledModelSql,
};

const NO_ANSWER: &str = "no answer";
const UNREAD: &str = "unread";

/// One model's compile lineage lines over orders and an export with a quoted column.
pub(crate) fn compile_lines(dialect: &str, sql: &str) -> Vec<String> {
    match compiled(dialect, sql) {
        None => vec![NO_ANSWER.to_owned()],
        Some(None) => vec![UNREAD.to_owned()],
        Some(Some(lineage)) => lineage.columns.iter().map(described).collect(),
    }
}

/// One model's name findings over the same relations, as `code message @start`.
pub(crate) fn compile_findings(dialect: &str, sql: &str) -> Vec<String> {
    match compiled(dialect, sql) {
        None => vec![NO_ANSWER.to_owned()],
        Some(None) => vec![UNREAD.to_owned()],
        Some(Some(lineage)) => lineage
            .findings
            .iter()
            .map(|finding| {
                let start: usize = finding.location.map_or(0, |(start, ..)| start);
                format!("{} {} @{start}", finding.code, finding.message)
            })
            .collect(),
    }
}

fn compiled(dialect: &str, sql: &str) -> Option<Option<CompiledLineage>> {
    let references: Vec<(String, String, String)> = vec![
        owned(("ORDERS_REL", "model", "orders")),
        owned(("EXPORTS_REL", "seed", "listing_export")),
    ];
    let schema: Vec<(String, Vec<(String, String)>)> = vec![
        (
            "ORDERS_REL".to_owned(),
            vec![
                ("ID".to_owned(), "NUMBER".to_owned()),
                ("AMOUNT".to_owned(), "NUMBER".to_owned()),
            ],
        ),
        (
            "EXPORTS_REL".to_owned(),
            vec![("\"listingId\"".to_owned(), "VARCHAR".to_owned())],
        ),
    ];
    let model = CompiledModelSql {
        sql,
        references: &references,
        schema: &schema,
        types: &[],
        nullability: &[],
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("the pool builds");
    compile_lineage(dialect, &[model], CompileFunctions::default(), &pool)
        .expect("lineage runs")
        .map(|mut models| models.pop().flatten())
}

fn owned((name, kind, resource): (&str, &str, &str)) -> (String, String, String) {
    (name.to_owned(), kind.to_owned(), resource.to_owned())
}

fn described(column: &CompileLineageColumn) -> String {
    let sources: Vec<String> = column
        .sources
        .iter()
        .map(|(kind, resource, name)| format!("{kind}:{resource}.{name}"))
        .collect();
    format!(
        "{} {} {} [{}]",
        column.output_column,
        column.transform.as_str(),
        column.confidence.as_str(),
        sources.join(", ")
    )
}

/// One model's output types over typed orders and events whose column list is not complete.
pub(crate) fn compile_types(sql: &str) -> Vec<String> {
    let references: Vec<(String, String, String)> = vec![
        owned(("ORDERS_REL", "model", "orders")),
        owned(("EVENTS_REL", "source", "events")),
    ];
    let schema: Vec<(String, Vec<(String, String)>)> = vec![(
        "ORDERS_REL".to_owned(),
        pairs(&[("ID", "NUMBER(38,0)"), ("AMOUNT", "VARCHAR")]),
    )];
    let types: Vec<(String, Vec<(String, String)>)> = vec![
        schema[0].clone(),
        (
            "EVENTS_REL".to_owned(),
            pairs(&[("user", "VARCHAR(16777216)"), ("kind", "VARCHAR")]),
        ),
    ];
    let nullability: Vec<(String, Vec<(String, String)>)> = vec![(
        "ORDERS_REL".to_owned(),
        pairs(&[("ID", "non_null"), ("AMOUNT", "nullable")]),
    )];
    let functions = CompileFunctions {
        return_types: &[("LISTAGG".to_owned(), "TEXT".to_owned())],
        nullability_rules: &[("UPPER".to_owned(), "first_arg".to_owned())],
        quoted_ignore_case: false,
    };
    let model = CompiledModelSql {
        sql,
        references: &references,
        schema: &schema,
        types: &types,
        nullability: &nullability,
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("the pool builds");
    let lineage = compile_lineage("snowflake", &[model], functions, &pool).expect("lineage runs");
    lineage
        .and_then(|mut models| models.pop().flatten())
        .map_or_else(
            || vec![UNREAD.to_owned()],
            |lineage| lineage.columns.iter().map(typed).collect(),
        )
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn typed(column: &CompileLineageColumn) -> String {
    format!(
        "{} {} {}",
        column.output_column,
        column.data_type.as_deref().unwrap_or("-"),
        column.nullability
    )
}
