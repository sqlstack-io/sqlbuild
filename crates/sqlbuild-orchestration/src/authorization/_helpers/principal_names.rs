//! Audit actors name principals by ID; people read them by name and kind.

use std::collections::BTreeMap;

use crate::credentials::models::PrincipalKind;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::ControlTransaction;

/// Each principal's name and kind, keyed by its audit actor text, `principal:<id>`.
pub(crate) async fn principal_names(
    transaction: ControlTransaction,
) -> Result<
    (
        ControlTransaction,
        BTreeMap<String, (String, PrincipalKind)>,
    ),
    ControlError,
> {
    let (transaction, rows) = query(
        transaction,
        "SELECT p.principal_id, p.name, CASE WHEN s.principal_id IS NULL THEN 0 ELSE 1 END AS service \
         FROM sqb_principals p LEFT JOIN sqb_service_accounts s ON s.principal_id=p.principal_id",
        Vec::new(),
    )
    .await?;
    let names: BTreeMap<String, (String, PrincipalKind)> = rows
        .iter()
        .filter_map(|row| {
            let kind = if row["service"].as_i64() == Some(1) {
                PrincipalKind::Service
            } else {
                PrincipalKind::Person
            };
            Some((
                format!("principal:{}", row["principal_id"].as_str()?),
                (row["name"].as_str()?.to_owned(), kind),
            ))
        })
        .collect();
    Ok((transaction, names))
}
