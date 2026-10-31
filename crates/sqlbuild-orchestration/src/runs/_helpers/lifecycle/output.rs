//! Normalize Recall output under its physical claim's invocation envelope.

use chrono::{DateTime, SecondsFormat};
use sqlbuild_recall::records::main::output_row::output_row;
use sqlbuild_recall::records::models::{OutputChunk, RecallRow, RowEnvelope};
use uuid::Uuid;

use crate::runs::constants::{OUTPUT_BATCH_BYTES, OUTPUT_BATCH_CHUNKS};
use crate::runs::models::ClaimMetadata;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn claim_envelope(
    transaction: ControlTransaction,
    (run_id, claim_id): (Uuid, Uuid),
) -> Result<(ControlTransaction, RowEnvelope), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT l.claimed_at, s.sqb_version FROM sqb_run_liveness l \
         JOIN sqb_runs r ON r.run_id=l.run_id JOIN sqb_snapshots s ON s.snapshot_id=r.snapshot_id \
         WHERE l.run_id=$1 AND l.worker_instance=$2",
        vec![Parameter::Id(run_id), Parameter::Id(claim_id)],
    )
    .await?;
    let metadata: ClaimMetadata = decode_row(rows.into_iter().next().ok_or_else(|| {
        ControlError::Invalid("publication does not belong to an active run claim".into())
    })?)?;
    Ok((
        transaction,
        RowEnvelope {
            invocation_id: format!("worker/{claim_id}"),
            invocation_started_at: metadata
                .claimed_at
                .to_rfc3339_opts(SecondsFormat::Micros, true),
            producer_version: metadata.sqb_version,
        },
    ))
}

/// Keep MAX(sequence) + 1 representable for Recall completeness accounting.
pub(crate) fn output_rows(
    envelope: &RowEnvelope,
    chunks: &[OutputChunk],
) -> Result<Vec<RecallRow>, ControlError> {
    if chunks.len() > OUTPUT_BATCH_CHUNKS
        || chunks.iter().map(|chunk| chunk.data.len()).sum::<usize>() > OUTPUT_BATCH_BYTES
    {
        return Err(ControlError::Invalid(
            "output batch exceeds its limit".into(),
        ));
    }
    let mut rows = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut chunk = chunk.clone();
        let mut integers = vec![chunk.sequence, chunk.byte_offset, chunk.dropped_before];
        if let Some(omitted) = chunk.omitted {
            integers.extend([omitted.bytes, omitted.sequences]);
            if !chunk.data.is_empty() || omitted.sequences == 0 {
                return Err(ControlError::Invalid(
                    "invalid output omission marker".into(),
                ));
            }
        }
        if chunk.sequence >= i64::MAX as u64
            || integers.iter().any(|value| i64::try_from(*value).is_err())
        {
            return Err(ControlError::Invalid(
                "output counter is out of range".into(),
            ));
        }
        chunk.occurred_at = DateTime::parse_from_rfc3339(&chunk.occurred_at)
            .map_err(|_| ControlError::Invalid("invalid output timestamp".into()))?
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Micros, true);
        rows.push(output_row(envelope, &chunk));
    }
    Ok(rows)
}
