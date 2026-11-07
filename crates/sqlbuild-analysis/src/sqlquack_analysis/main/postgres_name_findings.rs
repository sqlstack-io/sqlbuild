//! `postgres_name_findings`: SQLQuack's name findings in one PostgreSQL model.

use sqlquack_semantics::dialects::constants::POSTGRES;
use sqlquack_semantics::lineage::models::{LineageContext, NameFinding};
use sqlquack_semantics::statement_name_findings;
use sqlquack_syntax_postgres::{Parsed, parse_strict};

use crate::assembly::analysis_session::types::Shapes;
use crate::sqlquack_analysis::_helpers::schema::RelationColumns;

/// The name findings of the PostgreSQL query `sql` over the relations of `schema`, `None` unless
/// SQLQuack's PostgreSQL parser reads it as one statement and leaves no part of it unparsed.
pub(crate) fn postgres_name_findings(sql: &str, schema: &Shapes) -> Option<Vec<NameFinding>> {
    let parsed: Parsed = parse_strict(sql).ok()?;
    let [statement] = parsed.statements.as_slice() else {
        return None;
    };
    if !parsed.gaps.is_empty() {
        return None;
    }
    let relations: RelationColumns = RelationColumns::from_shapes(schema, &POSTGRES);
    let context = LineageContext {
        profile: &POSTGRES,
        schema: &relations,
        sql: Some(sql),
    };
    Some(statement_name_findings(statement, context))
}
