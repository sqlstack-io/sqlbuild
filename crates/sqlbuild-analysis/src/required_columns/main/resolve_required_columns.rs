use std::collections::HashMap;
use std::rc::Rc;

use polyglot_sql::{Dialect, Expression};
use sqlbuild_core::text::main::python_casefold::python_casefold;

use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use crate::required_columns::_helpers::queries::query_reads;
use crate::required_columns::_helpers::relations::{RelationMap, RelationReads, unique_reads};
use crate::required_columns::constants::{GENERIC_DIALECT, TABLE_FUNCTION_STUB_PREFIX};
use crate::required_columns::errors::RequiredColumnsError;
use crate::required_columns::models::{ColumnRead, ReferenceInput};
use crate::semantic_validation::main::normalize::normalize_analysis_sql;
use crate::semantic_validation::models::NormalizationInput;

/// The external columns a query reads through its CTEs and subqueries; unparseable SQL reads none.
pub fn resolve_required_columns(
    query_sql: &str,
    references: &[ReferenceInput],
    dialect: Option<&str>,
) -> Result<Vec<ColumnRead>, RequiredColumnsError> {
    let dialect_name: &str = dialect.unwrap_or(GENERIC_DIALECT);
    let sql: String = normalize_analysis_sql(NormalizationInput {
        sql: query_sql.to_owned(),
        dialect: dialect_name.to_owned(),
        stubs: HashMap::new(),
        placeholders: HashMap::new(),
    })
    .map_err(RequiredColumnsError::Normalization)?;
    let parser: Dialect = Dialect::get_by_name(dialect_name)
        .ok_or_else(|| RequiredColumnsError::UnknownDialect(dialect_name.to_owned()))?;
    let Ok(mut statements) = parser.parse_with_options(&sql, &polyglot_parse_options()) else {
        return Ok(Vec::new());
    };
    if statements.len() != 1 {
        return Ok(Vec::new());
    }
    let parsed: Expression = statements.remove(0);
    let mut relations: RelationMap = RelationMap::default();
    for reference in references {
        let Some((resource_type, analysis_name)) = lineage_resource(reference) else {
            continue;
        };
        let relation: RelationReads = RelationReads {
            slots: Vec::new(),
            passthrough: vec![(resource_type.to_owned(), reference.name.clone())],
        };
        relations.insert(python_casefold(&analysis_name), Rc::new(relation));
    }
    let (_, required) = query_reads(&parsed, &relations, &RelationMap::default());
    Ok(unique_reads(required))
}

/// A reference's lineage resource type and the relation name its query analyses as.
fn lineage_resource(reference: &ReferenceInput) -> Option<(&'static str, String)> {
    let resource_type: &'static str = match reference.kind.as_str() {
        "ref" => "model",
        "source" => "source",
        "seed" => "seed",
        "table_fn" => {
            let name: String = format!("{TABLE_FUNCTION_STUB_PREFIX}{}", reference.name);
            return Some(("table_fn", name));
        }
        _ => return None,
    };
    Some((resource_type, reference.name.clone()))
}
