//! Audit actors name principals by ID; people read them by name.

use std::collections::BTreeMap;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::ControlTransaction;

/// Each principal's name, keyed by its audit actor text, `principal:<id>`.
pub(crate) async fn principal_names(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, BTreeMap<String, String>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT principal_id, name FROM sqb_principals",
        Vec::new(),
    )
    .await?;
    let names: BTreeMap<String, String> = rows
        .iter()
        .filter_map(|row| {
            Some((
                format!("principal:{}", row["principal_id"].as_str()?),
                row["name"].as_str()?.to_owned(),
            ))
        })
        .collect();
    Ok((transaction, names))
}
