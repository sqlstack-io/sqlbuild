//! Whether a person may assign roles server-wide; unaudited, for deciding what a read may name.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::_helpers::person_grants::{allowed, person_role_rows};
use crate::credentials::models::{Action, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Inside the caller's transaction, so the answer matches the rest of its read.
pub(crate) async fn person_assigns_roles(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = person_role_rows(transaction, principal_id, None).await?;
    let assigns = allowed(
        &rows.unwrap_or_default(),
        (None, None),
        &BTreeSet::new(),
        DefaultRole::None,
    )
    .contains(&Action::RoleAssign);
    Ok((transaction, assigns))
}
