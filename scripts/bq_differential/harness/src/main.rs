//! Reads `{"id", "sql", "normalize", "placeholders"}` JSON lines on stdin and writes, per line, whether Polyglot's
//! BigQuery dialect and SQLQuack's native BigQuery parser parse the same text, with SQLQuack's
//! gaps and statement kinds. With `normalize`, the text first goes through SQLBuild's
//! `normalize_analysis_sql` for BigQuery with the model's `@@@` placeholder values, as SQLBuild
//! does before handing a body to Polyglot.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use polyglot_sql::DialectType;
use serde_json::{Value, json};
use sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql;
use sqlbuild_analysis::semantic_validation::models::NormalizationInput;

fn kind(debug: String) -> String {
    debug.split(['(', ' ', '{']).next().unwrap_or_default().to_owned()
}

fn compare(case: &Value) -> Value {
    let raw = case["sql"].as_str().unwrap_or_default().to_owned();
    let sql = if case["normalize"].as_bool().unwrap_or(false) {
        match normalize_analysis_sql(NormalizationInput {
            sql: raw.clone(),
            dialect: "bigquery".to_owned(),
            stubs: HashMap::new(),
            placeholders: case["placeholders"]
                .as_object()
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| (key.clone(), value.as_str().unwrap_or_default().to_owned()))
                        .collect()
                })
                .unwrap_or_default(),
        }) {
            Ok(sql) => sql,
            Err(error) => return json!({"id": case["id"], "normalize_error": error}),
        }
    } else {
        raw
    };
    let polyglot = std::panic::catch_unwind(|| polyglot_sql::parse(&sql, DialectType::BigQuery));
    let (polyglot_ok, polyglot_error) = match polyglot {
        Ok(Ok(_)) => (true, None),
        Ok(Err(error)) => (false, Some(error.to_string())),
        Err(_) => (false, Some("panic".to_owned())),
    };
    let quack = std::panic::catch_unwind(|| sqlquack_syntax_bigquery::parse(&sql));
    let (quack_verdict, quack_error, gaps, kinds) = match quack {
        Ok(Ok(parsed)) => (
            "ok",
            None,
            parsed
                .gaps
                .iter()
                .map(|gap| json!({"construct": gap.construct, "text": sql.get(gap.start..gap.end).unwrap_or_default()}))
                .collect::<Vec<_>>(),
            parsed.statements.iter().map(|s| kind(format!("{s:?}"))).collect::<Vec<_>>(),
        ),
        Ok(Err(error)) => ("error", Some(format!("{error:?}")), vec![], vec![]),
        Err(_) => ("panic", None, vec![], vec![]),
    };
    json!({
        "id": case["id"], "sql": sql,
        "polyglot": polyglot_ok, "polyglot_error": polyglot_error,
        "sqlquack": quack_verdict, "sqlquack_error": quack_error,
        "gaps": gaps, "kinds": kinds,
    })
}

fn main() {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(run)
        .expect("thread")
        .join()
        .expect("join");
}

fn run() {
    std::panic::set_hook(Box::new(|_| {}));
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let case: Value = serde_json::from_str(&line).expect("case json");
        writeln!(out, "{}", compare(&case)).expect("stdout");
    }
}
