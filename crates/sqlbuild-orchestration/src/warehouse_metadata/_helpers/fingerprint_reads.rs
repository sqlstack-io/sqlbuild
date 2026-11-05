//! One indexed exact-node/destination read, shared by evidence and definition comparison.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::{FingerprintObservation, FingerprintQuery};

pub(crate) async fn latest_fingerprint(
    transaction: ControlTransaction,
    (location_id, target, options): (Uuid, &str, &FingerprintQuery),
) -> Result<(ControlTransaction, Option<(FingerprintObservation, String)>), ControlError> {
    if options.node_type.trim().is_empty() || options.node_name.trim().is_empty() {
        return Err(ControlError::Invalid(
            "fingerprint reads require an exact node".into(),
        ));
    }
    let mut parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Text(options.node_type.clone()),
        Parameter::Text(options.node_name.clone()),
    ];
    let mut predicates: Vec<String> = Vec::new();
    for (column, value) in [
        ("target_database", &options.database),
        ("target_schema", &options.schema),
        ("target_name", &options.name),
    ] {
        predicates.push(if let Some(value) = value {
            parameters.push(Parameter::Text(value.clone()));
            format!("{column}=${}", parameters.len())
        } else {
            format!("{column} IS NULL")
        });
    }
    let (transaction, rows) = query(transaction,
        &format!("SELECT event_id,observed_at,source_event_id,record_json ->> 'run_id' AS run_id, \
            record_json ->> 'definition_hash' AS definition_hash,record_json ->> 'version_hash' AS version_hash, \
            record_json ->> 'schema_fingerprint' AS schema_fingerprint,record_json ->> 'metadata_json' AS metadata_json FROM sqb_warehouse_fingerprints \
            WHERE location_id=$1 AND target=$2 AND node_type=$3 AND node_name=$4 AND {} \
            ORDER BY target_database,target_schema,target_name,observed_at DESC,event_id DESC LIMIT 1", predicates.join(" AND ")), parameters).await?;
    let evidence = rows
        .into_iter()
        .next()
        .map(|mut row| {
            let metadata = row
                .remove("metadata_json")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            Ok::<_, ControlError>((decode_row(row)?, metadata))
        })
        .transpose()?;
    Ok((transaction, evidence))
}
