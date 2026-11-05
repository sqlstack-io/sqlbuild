//! Server startup replaces the stored group mapping, so a removed mapping narrows access at once.

use crate::api::models::GroupRole;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::write::write;
use crate::store::models::{Change, ControlStore, Parameter};

pub(crate) async fn publish_group_roles(
    store: &ControlStore,
    mappings: &[GroupRole],
) -> Result<(), ControlError> {
    write(store, |transaction| async move {
        let (mut transaction, _) = query(transaction, "DELETE FROM sqb_group_roles", vec![]).await?;
        for mapping in mappings {
            let role = serde_json::to_value(&mapping.role)
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            (transaction, _) = query(
                transaction,
                "INSERT INTO sqb_group_roles (group_name,role_key,location_id,target) VALUES ($1,$2,$3,$4)",
                vec![
                    Parameter::Text(mapping.group.clone()),
                    Parameter::Text(role.as_str().unwrap_or_default().into()),
                    Parameter::OptionalId(mapping.location_id),
                    Parameter::OptionalText(mapping.target.clone()),
                ],
            )
            .await?;
        }
        transaction.changes.insert(Change::Credentials);
        Ok((transaction, ()))
    })
    .await
}
