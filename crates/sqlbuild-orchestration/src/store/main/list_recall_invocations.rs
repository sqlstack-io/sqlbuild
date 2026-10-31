//! Discover invocation namespaces from retained Recall facts, including output-only attempts.

use sqlbuild_recall::schema::constants::INVOCATION_TABLES;
use sqlbuild_recall::schema::main::table_spec::table_spec;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

/// The caller authorizes exact namespace roots; prefix matching escapes LIKE patterns and includes the slash boundary.
pub async fn list_recall_invocations(
    store: &ControlStore,
    namespaces: &[String],
    limit: u32,
) -> Result<Vec<Row>, ControlError> {
    if namespaces.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut parameters: Vec<Parameter> = Vec::new();
    let predicates = namespaces
        .iter()
        .map(|namespace| {
            let exact = parameters.len() + 1;
            parameters.push(Parameter::Text(namespace.clone()));
            let pattern = namespace
                .replace('!', "!!")
                .replace('%', "!%")
                .replace('_', "!_");
            parameters.push(Parameter::Text(format!("{pattern}/%")));
            format!(
                "invocation_id=${exact} OR invocation_id LIKE ${} ESCAPE '!'",
                exact + 1
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let tables = INVOCATION_TABLES
        .iter()
        .map(|table| {
            format!(
                "SELECT invocation_id, invocation_started_at FROM {} WHERE {predicates}",
                table_spec(*table).name,
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    parameters.push(Parameter::Integer(i64::from(limit.min(1000))));
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT invocation_id, MIN(invocation_started_at) AS invocation_started_at \
            FROM ({tables}) AS records GROUP BY invocation_id ORDER BY invocation_id LIMIT ${}",
            parameters.len()
        ),
        parameters,
    )
    .await?;
    commit(transaction).await?;
    Ok(rows)
}
