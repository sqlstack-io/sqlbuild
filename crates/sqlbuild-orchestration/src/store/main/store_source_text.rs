//! Snapshot sources and warehouse definitions share exact-content-addressed immutable bodies.

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn store_source_text(
    transaction: ControlTransaction,
    (hash, body): (String, Vec<u8>),
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_source_texts (text_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        vec![Parameter::Text(hash), Parameter::Bytes(body)],
    )
    .await?;
    Ok(transaction)
}
