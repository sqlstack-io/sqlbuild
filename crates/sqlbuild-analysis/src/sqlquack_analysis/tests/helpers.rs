use crate::sqlquack_analysis::main::compile_lineage::compile_lineage;
use crate::sqlquack_analysis::models::{CompileLineageColumn, CompiledModelSql};

const NO_ANSWER: &str = "no answer";
const UNREAD: &str = "unread";

/// One model's compile lineage lines over orders and an export with a quoted column.
pub(crate) fn compile_lines(dialect: &str, sql: &str) -> Vec<String> {
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
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("the pool builds");
    let lineage = compile_lineage(dialect, &[model], &pool).expect("lineage runs");
    let Some(mut models) = lineage else {
        return vec![NO_ANSWER.to_owned()];
    };
    models.pop().flatten().map_or_else(
        || vec![UNREAD.to_owned()],
        |columns| columns.iter().map(described).collect(),
    )
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
