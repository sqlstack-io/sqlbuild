//! A run's Recall rows: the scheduler invocation and its claimed worker's invocations, with fixed column lists.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::models::{
    RunErrorText, RunEventRecord, RunOutput, RunOutputLine, RunStatementRecord, RunStatementSql,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// The Recall invocations of one run and the nodes it selected.
pub(crate) struct RunRecallScope {
    namespaces: Vec<String>,
    selection: BTreeSet<String>,
}

/// The run's transaction and scope, plus the deciding context and granted scope of a sensitive read.
pub(crate) type RunRecallRead = (
    ControlTransaction,
    RunRecallScope,
    Option<(AuditContext, PermissionScope)>,
);

/// Listing view first so unknown runs look refused, then the run, then any sensitive action on its selection.
pub(crate) async fn begin_run_recall(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, run_id): (Uuid, &str, Uuid),
    (default_role, sensitive): (DefaultRole, Option<Action>),
) -> Result<Option<RunRecallRead>, ControlError> {
    let (transaction, context) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, scope) = run_recall_scope(transaction, (location_id, target, run_id)).await?;
    let Some(scope) = scope else {
        commit(transaction).await?;
        return Ok(None);
    };
    let Some(action) = sensitive else {
        return Ok(Some((transaction, scope, None)));
    };
    let permission = PermissionScope {
        action,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: scope.selection.clone(),
    };
    let (transaction, context) = authorize_person(
        transaction,
        (&context, credentials),
        &permission,
        DefaultRole::None,
    )
    .await?;
    Ok(Some((transaction, scope, Some((context, permission)))))
}

/// A sensitive run read, authorized for `action` and audited as `audit_action` in the same transaction.
pub(crate) async fn sensitive_run_read<T>(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (run, default_role, action): ((Uuid, &str, Uuid), DefaultRole, Action),
    (audit_action, read, details): (
        &str,
        impl AsyncFnOnce(
            ControlTransaction,
            &RunRecallScope,
        ) -> Result<(ControlTransaction, T), ControlError>,
        impl FnOnce(&T) -> Value,
    ),
) -> Result<Option<T>, ControlError> {
    let Some((transaction, scope, Some((context, permission)))) =
        begin_run_recall(store, credentials, run, (default_role, Some(action))).await?
    else {
        return Ok(None);
    };
    let (transaction, result) = read(transaction, &scope).await?;
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::now_v7(), audit_action),
        (
            json!(permission),
            json!(null),
            json!({"run_id": run.2, "read": details(&result)}),
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(Some(result))
}

/// `None` when the run is not in this location and target.
async fn run_recall_scope(
    transaction: ControlTransaction,
    (location_id, target, run_id): (Uuid, &str, Uuid),
) -> Result<(ControlTransaction, Option<RunRecallScope>), ControlError> {
    let (transaction, runs) = query(
        transaction,
        "SELECT selection_json FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    let Some(run) = runs.first() else {
        return Ok((transaction, None));
    };
    let selection = match &run["selection_json"] {
        Value::String(text) => serde_json::from_str(text),
        other => serde_json::from_value(other.clone()),
    }
    .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let scheduler = format!("scheduler/{run_id}");
    let (transaction, claims) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE invocation_id=$1 AND event_type='run_claimed' ORDER BY invocation_sequence DESC LIMIT 1",
        vec![Parameter::Text(scheduler.clone())],
    )
    .await?;
    let mut namespaces = vec![scheduler];
    if let Some(claim_id) = claims
        .first()
        .and_then(|row| row["payload"]["claim"]["claim_id"].as_str())
    {
        namespaces.push(format!("worker/{claim_id}"));
    }
    Ok((
        transaction,
        Some(RunRecallScope {
            namespaces,
            selection,
        }),
    ))
}

pub(crate) async fn run_events_in(
    transaction: ControlTransaction,
    scope: &RunRecallScope,
    (after, limit): (i64, u32),
) -> Result<(ControlTransaction, Vec<RunEventRecord>), ControlError> {
    let (filter, mut parameters) = invocation_filter(scope, 1);
    let next = parameters.len() + 1;
    parameters.extend([
        Parameter::Integer(after),
        Parameter::Integer(i64::from(limit.min(5000))),
    ]);
    query_records(transaction,
        &format!("SELECT storage_id, event_id, invocation_id, occurred_at, event_type, severity, producer, resource_id, \
         resource_kind, resource_name, resource_attempt_id, statement_id, error_type, error_code, duration_ms, outcome, \
         row_count, affected_rows FROM events WHERE {filter} AND storage_id>${next} ORDER BY storage_id LIMIT ${}", next + 1),
        parameters).await
}

pub(crate) async fn run_statements_in(
    transaction: ControlTransaction,
    scope: &RunRecallScope,
    limit: u32,
) -> Result<(ControlTransaction, Vec<RunStatementRecord>), ControlError> {
    let (filter, mut parameters) = invocation_filter(scope, 1);
    let next = parameters.len() + 1;
    parameters.push(Parameter::Integer(i64::from(limit.min(5000))));
    let (transaction, rows) = query(transaction,
        &format!("SELECT invocation_id, statement_id, resource_id, resource_name, resource_kind, intent, statement_kind, \
         purpose, query_id, started_at, ended_at, outcome, error_type, error_code, duration_ms, row_count, affected_rows, \
         sql_available, sql_unavailable_reason FROM statements WHERE {filter} ORDER BY started_at, statement_id LIMIT ${next}"),
        parameters).await?;
    let records = rows
        .into_iter()
        .map(|mut row| {
            let available = row
                .get("sql_available")
                .is_some_and(|value| value.as_bool().unwrap_or_else(|| value.as_i64() == Some(1)));
            row.insert("sql_available".into(), Value::Bool(available));
            decode_row(row)
        })
        .collect::<Result<_, _>>()?;
    Ok((transaction, records))
}

pub(crate) async fn statement_sql_in(
    transaction: ControlTransaction,
    scope: &RunRecallScope,
    statement_id: &str,
) -> Result<(ControlTransaction, Option<RunStatementSql>), ControlError> {
    let (filter, mut parameters) = invocation_filter(scope, 1);
    let next = parameters.len() + 1;
    parameters.push(Parameter::Text(statement_id.into()));
    let (transaction, statements) = query(
        transaction,
        &format!("SELECT statement_id FROM statements WHERE {filter} AND statement_id=${next}"),
        parameters.clone(),
    )
    .await?;
    if statements.is_empty() {
        return Ok((transaction, None));
    }
    let (transaction, rows) = query(transaction,
        &format!("SELECT text FROM artifacts WHERE {filter} AND statement_id=${next} AND kind='sql' LIMIT 1"),
        parameters).await?;
    let sql = rows
        .first()
        .map(|row| display_text(&row["text"]))
        .transpose()?;
    Ok((
        transaction,
        Some(RunStatementSql {
            statement_id: statement_id.into(),
            sql: sql.flatten(),
        }),
    ))
}

pub(crate) async fn run_output_in(
    transaction: ControlTransaction,
    scope: &RunRecallScope,
) -> Result<(ControlTransaction, RunOutput), ControlError> {
    let (filter, parameters) = invocation_filter(scope, 1);
    let (transaction, rows) = query(transaction,
        &format!("SELECT invocation_id, sequence, stream, occurred_at, text FROM output_tail WHERE {filter} \
         ORDER BY occurred_at, invocation_id, sequence"),
        parameters.clone()).await?;
    let lines = rows
        .into_iter()
        .map(|mut row| {
            let text = display_text(&row["text"])?.unwrap_or_default();
            row.insert("text".into(), Value::String(text));
            decode_row::<RunOutputLine>(row)
        })
        .collect::<Result<_, _>>()?;
    let (transaction, failures) = query(transaction,
        &format!("SELECT invocation_id, ended_at AS occurred_at, error_type, error_message AS message FROM invocations \
         WHERE {filter} AND error_message IS NOT NULL"),
        parameters.clone()).await?;
    let (transaction, diagnostics) = query(
        transaction,
        &format!(
            "SELECT invocation_id, occurred_at, logger AS error_type, message FROM diagnostics \
         WHERE {filter} AND severity IN ('error','critical') AND message IS NOT NULL"
        ),
        parameters,
    )
    .await?;
    let mut errors: Vec<RunErrorText> = failures
        .into_iter()
        .chain(diagnostics)
        .map(decode_row)
        .collect::<Result<_, _>>()?;
    errors.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
    Ok((transaction, RunOutput { lines, errors }))
}

/// Exact scheduler and worker invocations, plus the worker's nested invocations.
fn invocation_filter(scope: &RunRecallScope, first: usize) -> (String, Vec<Parameter>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut parameters: Vec<Parameter> = Vec::new();
    for namespace in &scope.namespaces {
        clauses.push(format!("invocation_id=${}", first + parameters.len()));
        parameters.push(Parameter::Text(namespace.clone()));
        if namespace.starts_with("worker/") {
            clauses.push(format!("invocation_id LIKE ${}", first + parameters.len()));
            parameters.push(Parameter::Text(format!("{namespace}/%")));
        }
    }
    (format!("({})", clauses.join(" OR ")), parameters)
}

/// Captured bytes decode lossily for display; text columns pass through.
fn display_text(value: &Value) -> Result<Option<String>, ControlError> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        other => {
            let bytes: Vec<u8> = serde_json::from_value(other.clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }
}
