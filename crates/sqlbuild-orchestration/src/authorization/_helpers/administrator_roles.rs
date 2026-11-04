//! Authenticated installation administrators assign additive built-ins; code never assigns people.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::administrator_changes::apply_change;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{
    AdministratorAuthentication, AdministratorChange, AuditContext, BuiltinRole,
    RoleAssignmentRequest, RoleKey,
};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn assign_role_in(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &RoleAssignmentRequest,
    authentication: &AdministratorAuthentication,
) -> Result<bool, ControlError> {
    if request
        .target
        .as_ref()
        .is_some_and(|target| target.trim().is_empty())
        || (request.target.is_some() && request.location_id.is_none())
    {
        return Err(ControlError::Invalid(
            "role target requires a location and a nonempty name".into(),
        ));
    }
    if let RoleKey::Custom(key) = &request.role
        && (key.trim().is_empty() || request.location_id.is_none())
    {
        return Err(ControlError::Invalid(
            "custom role assignment requires a location and a nonempty role key".into(),
        ));
    }
    if request.role == RoleKey::Builtin(BuiltinRole::Admin) {
        if request.location_id.is_some() || request.target.is_some() {
            return deny(
                transaction,
                context,
                json!(request),
                "administrator assignment must be installation-wide",
            )
            .await;
        }
        return apply_change(
            transaction,
            context,
            (
                request.principal_id,
                if request.revoke {
                    AdministratorChange::Revoke
                } else {
                    AdministratorChange::Grant
                },
            ),
            authentication,
        )
        .await;
    }
    let transaction = if let RoleKey::Custom(key) = &request.role
        && !request.revoke
    {
        let (transaction, published) = query(transaction,
            "SELECT role_key FROM sqb_role_definition_versions WHERE location_id=$1 AND role_key=$2 AND ($3 IS NULL OR target=$3) LIMIT 1",
            vec![Parameter::OptionalId(request.location_id), Parameter::Text(key.clone()), Parameter::OptionalText(request.target.clone())]).await?;
        if published.is_empty() {
            return Err(ControlError::Invalid(format!(
                "role {key} is neither a built-in role nor a custom role this location has published"
            )));
        }
        transaction
    } else {
        transaction
    };
    let (transaction, people) = query(transaction, "SELECT principal_id,disabled_at FROM sqb_principals WHERE principal_id=$1 AND kind='person'",
        vec![Parameter::Id(request.principal_id)]).await?;
    if people
        .first()
        .is_none_or(|person| !request.revoke && !person["disabled_at"].is_null())
    {
        return deny(
            transaction,
            context,
            json!(request),
            "role assignment requires an enabled person; revocation requires an existing person",
        )
        .await;
    }
    assign_scoped_role(transaction, context, request).await
}

async fn assign_scoped_role(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &RoleAssignmentRequest,
) -> Result<bool, ControlError> {
    let role = serde_json::to_value(&request.role)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let key = serde_json::to_string(&(
        request.principal_id,
        &request.role,
        request.location_id,
        &request.target,
    ))
    .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let assignment_id = Uuid::now_v7();
    let now = transaction.now;
    let (mut transaction, changed) = if request.revoke {
        query(transaction, "UPDATE sqb_role_assignments SET revoked_at=$1,revoked_by=$2,active_key=NULL WHERE active_key=$3 RETURNING assignment_id",
            vec![Parameter::Time(now),Parameter::Text(context.actor.clone()),Parameter::Text(key)]).await?
    } else {
        query(transaction, "INSERT INTO sqb_role_assignments (assignment_id,principal_id,role_key,location_id,target,source,granted_by,granted_at,active_key) VALUES ($1,$2,$3,$4,$5,'manual',$6,$7,$8) ON CONFLICT (active_key) DO NOTHING RETURNING assignment_id",
            vec![Parameter::Id(assignment_id),Parameter::Id(request.principal_id),Parameter::Text(role.as_str().unwrap_or_default().into()),Parameter::OptionalId(request.location_id),Parameter::OptionalText(request.target.clone()),Parameter::Text(context.actor.clone()),Parameter::Time(now),Parameter::Text(key)]).await?
    };
    let applied = !changed.is_empty();
    if applied {
        transaction = append_audit(
            transaction,
            context,
            (
                assignment_id,
                if request.revoke {
                    "role.revoked"
                } else {
                    "role.assigned"
                },
            ),
            (json!(request), json!(null), json!({"assignments":changed})),
        )
        .await?;
        transaction.changes.insert(Change::Credentials);
    }
    commit(transaction).await?;
    Ok(applied)
}
