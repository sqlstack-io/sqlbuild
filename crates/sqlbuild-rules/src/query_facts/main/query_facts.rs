use crate::query_facts::models::QueryFacts;

/// Parse normalized model SQL with SQLQuack and read the facts built-in rules evaluate.
pub(crate) fn query_facts(sql: &str, dialect: &str) -> Result<QueryFacts, String> {
    crate::query_facts::_helpers::facts::query_facts(sql, dialect)
}
