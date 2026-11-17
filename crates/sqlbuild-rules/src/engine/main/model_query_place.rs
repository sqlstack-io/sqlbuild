//! What places a compiled model's expanded query in its file.

use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;

use crate::engine::_helpers::project_rows::expansion_spans;
use crate::models::{AuthoredQuery, ModelQueryPlace};

/// Where `facts`' authored query is and its expansion passes; finding the query is a suffix check.
#[must_use]
pub fn model_query_place(facts: &CompiledModelFacts) -> ModelQueryPlace {
    let file: &str = &facts.authored_sql;
    let query: &str = &facts.authored_query_sql;
    let start: Option<usize> = if file.ends_with(query) {
        Some(file.len() - query.len())
    } else {
        file.rfind(query)
    };
    ModelQueryPlace {
        authored_query: (!query.is_empty()).then(|| {
            start.map_or_else(
                || AuthoredQuery::Text(query.to_owned()),
                |start| AuthoredQuery::Range(start, start + query.len()),
            )
        }),
        passes: if facts.expansion_passes.iter().all(Vec::is_empty) {
            Vec::new()
        } else {
            expansion_spans(&facts.expansion_passes)
        },
    }
}
