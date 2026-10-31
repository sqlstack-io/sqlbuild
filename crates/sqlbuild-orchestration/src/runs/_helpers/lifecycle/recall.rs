//! Namespace Recall facts and validate artifact ownership before publication.

use std::collections::BTreeMap;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use sqlbuild_recall::records::models::{RecallRow, RecallValue, RowEnvelope};
use sqlbuild_recall::schema::models::Table;
use sqlbuild_recall::store::models::InsertOutcome;

use crate::runs::models::RecallRecord;
use crate::store::errors::ControlError;
use crate::store::main::append_recall::append_recall;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Recall statement projections join correlations globally, not only by invocation.
pub(crate) fn claim_record(
    envelope: &RowEnvelope,
    record: &RecallRecord,
) -> Result<RecallRow, ControlError> {
    let mut values = record.values.clone();
    for name in [
        "event_id",
        "run_id",
        "resource_id",
        "resource_attempt_id",
        "operation_id",
        "statement_id",
    ] {
        if let Some(RecallValue::Text(id)) = values.get_mut(name) {
            *id = serde_json::json!([envelope.invocation_id, id]).to_string();
        }
    }
    match record.table {
        Table::Meta | Table::Conflicts => {
            return Err(ControlError::Invalid(
                "Recall store metadata is server-owned".into(),
            ));
        }
        Table::Artifacts => {
            let Some(RecallValue::Blob(content)) = values.get("content") else {
                return Err(ControlError::Invalid(
                    "Recall artifact content must be bytes".into(),
                ));
            };
            if values.get("digest")
                != Some(&RecallValue::Text(format!(
                    "sha256:{}",
                    sha256_hex(content)
                )))
                || values.get("size") != Some(&RecallValue::Integer(content.len() as i64))
            {
                return Err(ControlError::Invalid(
                    "Recall artifact digest or size differs from its bytes".into(),
                ));
            }
        }
        _ => {}
    }
    RecallRow::from_values(envelope, record.table, &values)
        .map_err(|error| ControlError::Invalid(error.to_string()))
}

pub(crate) async fn append_records(
    mut transaction: ControlTransaction,
    envelope: &RowEnvelope,
    rows: &[RecallRow],
) -> Result<(ControlTransaction, InsertOutcome), ControlError> {
    let mut result = InsertOutcome::default();
    let mut delivered: BTreeMap<Table, u64> = BTreeMap::new();
    for row in rows {
        if row.table == Table::ArtifactRefs
            && !rows.iter().any(|candidate| {
                candidate.table == Table::Artifacts
                    && candidate.value("digest") == row.value("digest")
            })
        {
            let Some(RecallValue::Text(digest)) = row.value("digest") else {
                return Err(ControlError::Invalid(
                    "Recall artifact digest must be text".into(),
                ));
            };
            let references;
            (transaction, references) = query(transaction,
                "SELECT 1 AS present FROM recall_artifact_refs WHERE invocation_id=$1 AND digest=$2 LIMIT 1",
                vec![Parameter::Text(envelope.invocation_id.clone()), Parameter::Text(digest.clone())]).await?;
            if references.is_empty() {
                return Err(ControlError::Invalid(
                    "Recall artifact reference has no invocation-owned content".into(),
                ));
            }
        }
        let outcome;
        (transaction, outcome) = append_recall(transaction, row).await?;
        if let Some(conflict) = outcome.conflict {
            result.conflicts.push(conflict);
        } else {
            *delivered.entry(row.table).or_insert(0) += 1;
        }
    }
    result.delivered = delivered.into_iter().collect();
    Ok((transaction, result))
}
