//! `reads_added_columns`: whether a model must be analysed again when its inputs gain columns.

use std::collections::HashSet;

use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlquack_syntax::Parsed;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::semantic_validation::main::normalize_native::normalize_native_sql;
use crate::source_drift::_helpers::walk::{Drift, Walk};
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

/// Whether `query_sql` could return something else once the `drifted` relations gain `added`.
#[must_use]
pub fn reads_added_columns(
    dialect: &str,
    (query_sql, placeholders): (&str, &[(String, String)]),
    drifted: &[String],
    added: &[String],
) -> bool {
    let AnalysisRoute::Native(native) = analysis_route(Some(dialect)) else {
        return true;
    };
    let Ok(normalized) = normalize_native_sql(query_sql, native.name(), native.stub_prefix())
    else {
        return true;
    };
    let sql: String = placeholder_defaults(&normalized, placeholders);
    let Ok(Parsed { statements, .. }) = native.parse(&sql) else {
        return true;
    };
    let [statement] = statements.as_slice() else {
        return true;
    };
    let relations: HashSet<String> = drifted.iter().map(|name| python_casefold(name)).collect();
    let added: HashSet<String> = added.iter().map(|name| python_casefold(name)).collect();
    let mut walk: Walk<'_> = Walk::new(Drift {
        relations: &relations,
        added: &added,
    });
    walk.statement(statement);
    walk.affected
}
