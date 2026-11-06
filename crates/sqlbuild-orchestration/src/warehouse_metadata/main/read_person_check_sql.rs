//! Live SQL permission on the check's node, its latest SQL and errors, and the sensitive-read audit share one writer fence.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::checks::{check_results, check_scope};
use crate::warehouse_metadata::models::CheckSql;

pub async fn read_person_check_sql(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, binding_key): (Uuid, &str, &str),
) -> Result<Option<CheckSql>, ControlError> {
    let (transaction, mut rows) = check_results(
        begin(store, TransactionMode::Write).await?,
        (location_id, target, binding_key),
        1,
    )
    .await?;
    let latest = rows.pop();
    let attached = latest
        .as_ref()
        .and_then(|row| row.record.attached_target_name.as_deref());
    let (transaction, scope) = check_scope(
        transaction,
        (location_id, target, attached),
        Action::ViewSql,
    )
    .await?;
    let (transaction, context) =
        authorize_person(transaction, credentials, &scope, DefaultRole::None).await?;
    let result = latest.map(|row| CheckSql {
        event_id: row.event_id,
        binding_key: row.record.binding_key,
        executed_sql: row.record.executed_sql,
        measurement_sql: row.record.measurement_sql,
        evidence_sql: row.record.evidence_sql,
        execution_error: row.record.execution_error,
        evidence_error: row.record.evidence_error,
    });
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::now_v7(), "sql.read"),
        (
            json!(scope),
            json!(null),
            json!({"binding_key": binding_key, "event_id": result.as_ref().map(|sql| sql.event_id), "found": result.is_some()}),
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(result)
}
