//! `convert_local_sql`: captured Snowflake SQL as DuckDB SQL under SQLQuack's built-in default.

use std::collections::BTreeMap;

use sqlquack_convert::analysis::main::analyze::analyze;
use sqlquack_core::diagnostics::main::diagnose::diagnose;
use sqlquack_core::diagnostics::main::render_diagnostics::render_diagnostics;
use sqlquack_core::parsing::models::SourceDialect;
use sqlquack_rules::catalogue::main::builtin_catalogue::builtin_catalogue;
use sqlquack_rules::session::main::session_conversion_options::session_conversion_options;
use sqlquack_semantics::types::main::parse_schema::parse_schema;

use crate::local_sql::models::LocalSql;

/// `sql`, named `name` in diagnostics, converted against `schema_json` as `sqlquack convert` does by default.
pub fn convert_local_sql(sql: &str, schema_json: &str, name: &str) -> Result<LocalSql, String> {
    let schema = parse_schema(schema_json).map_err(|error| error.to_string())?;
    let (options, _profile) = session_conversion_options(None, (false, false), &BTreeMap::new())?;
    let analysis = analyze(
        sql,
        SourceDialect::Snowflake,
        &builtin_catalogue(),
        (&schema, &options),
    );
    if !analysis.parsed || !analysis.refusals.is_empty() {
        return Ok(LocalSql::Refused(render_diagnostics(
            &diagnose(&analysis.refusals, sql),
            sql,
            name,
        )));
    }
    let [converted] = analysis.conversions.as_slice() else {
        return Ok(LocalSql::Refused(format!(
            "expected one statement, got {}",
            analysis.statements
        )));
    };
    Ok(LocalSql::Converted {
        sql: converted.sql.clone(),
        columns: converted
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.data_type.to_string()))
            .collect(),
    })
}
