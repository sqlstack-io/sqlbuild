//! Live SQL permission on the check's node, its latest SQL and errors, and the sensitive-read audit share one writer fence.

use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::checks::{check_results, sensitive_check_read};
use crate::warehouse_metadata::models::CheckSql;

pub async fn read_person_check_sql(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    check: (Uuid, &str, &str),
) -> Result<Option<CheckSql>, ControlError> {
    let (transaction, mut rows) =
        check_results(begin(store, TransactionMode::Write).await?, check, 1).await?;
    sensitive_check_read(
        (transaction, credentials),
        check,
        (rows.pop(), Action::ViewSql, "sql.read"),
        |row| {
            Ok(CheckSql {
                event_id: row.event_id,
                binding_key: row.record.binding_key,
                executed_sql: row.record.executed_sql,
                measurement_sql: row.record.measurement_sql,
                evidence_sql: row.record.evidence_sql,
                execution_error: row.record.execution_error,
                evidence_error: row.record.evidence_error,
            })
        },
    )
    .await
}
