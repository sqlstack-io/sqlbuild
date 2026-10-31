//! Direct database ownership authorizes bootstrap, not an untrusted HTTP request or supplied role.

use serde_json::{Value, json};
use sqlbuild_orchestration::credentials::main::create_location::create_location;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::store::errors::ControlError;
use uuid::Uuid;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::decision::control_admin;
use crate::native_commands::_helpers::execution::open_control_store;
use crate::native_commands::models::ControlAdmin;

pub(crate) async fn run_admin(namespace: &Namespace) -> Result<Value, ControlError> {
    let action = control_admin(namespace)?;
    let store = open_control_store(namespace).await?;
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    match action {
        ControlAdmin::CreateLocation(name) => {
            let id = create_location(&store, &name, &context).await?;
            Ok(json!({"location_id": id, "name": name}))
        }
        ControlAdmin::IssueLocationToken(location_id, expires_at) => {
            let token = issue_token(
                &store,
                &context,
                CredentialScope::Location { location_id },
                expires_at,
            )
            .await?;
            Ok(json!(token))
        }
        ControlAdmin::RevokeToken(token_id) => {
            let revoked = revoke_token(&store, &context, token_id).await?;
            Ok(json!({"token_id": token_id, "revoked": revoked}))
        }
    }
}
