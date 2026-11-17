//! Where a compiled model's expanded query sits in its file.

use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;

use crate::engine::_helpers::project_rows::{
    authored_query_map, query_column_offset, query_line_offset,
};
use crate::models::ModelQueryPlace;

/// The file place of `facts`' expanded query, read the way built-in model rules read it.
#[must_use]
pub fn model_query_place(facts: &CompiledModelFacts) -> ModelQueryPlace {
    let file: &str = &facts.authored_sql;
    let authored_query: &str = &facts.authored_query_sql;
    ModelQueryPlace {
        authored_sql: if authored_query.is_empty() {
            file
        } else {
            authored_query
        }
        .to_owned(),
        line_offset: query_line_offset(file, authored_query, &facts.query_sql),
        column_offset: query_column_offset(file, authored_query, &facts.query_sql),
        authored_query: authored_query_map(facts),
    }
}
