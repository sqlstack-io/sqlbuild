//! Stable function definition text used for change fingerprints.

use crate::planner_identity::models::FunctionFingerprintFields;

/// The language, signature, runtime fields, any frame library and body, one field per line.
pub fn function_fingerprint_sql(fields: &FunctionFingerprintFields) -> String {
    let render = |pairs: &[(String, String)]| -> String {
        pairs
            .iter()
            .map(|(name, kind)| format!("{name}:{kind}"))
            .collect::<Vec<String>>()
            .join(",")
    };
    let mut lines: Vec<String> = vec![
        format!("language={}", fields.language),
        format!("arguments={}", render(&fields.arguments)),
        format!("returns={}", fields.returns),
        format!("return_columns={}", render(&fields.return_columns)),
        format!("runtime_version={}", fields.runtime_version),
        format!("entry_point={}", fields.entry_point),
        format!("packages={}", fields.packages.join(",")),
    ];
    lines.extend(
        fields
            .dataframe
            .as_ref()
            .map(|dataframe| format!("dataframe={dataframe}")),
    );
    lines.extend(["body=".to_owned(), fields.body_sql.clone()]);
    lines.join("\n")
}
