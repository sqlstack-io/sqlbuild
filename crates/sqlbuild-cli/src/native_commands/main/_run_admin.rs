//! Direct database ownership authorizes bootstrap, not an untrusted HTTP request or supplied role.

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use serde_json::{Value, json};
use sqlbuild_orchestration::authorization::main::people::bootstrap_user::bootstrap_user;
use sqlbuild_orchestration::authorization::main::roles::grant_admin::grant_admin;
use sqlbuild_orchestration::authorization::main::service_accounts::{
    create_service_account::create_service_account, find_service_account::find_service_account,
    list_service_accounts::list_service_accounts, rotate_service_key::rotate_service_key,
    set_service_account_disabled::set_service_account_disabled,
};
use sqlbuild_orchestration::credentials::main::identities::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::locations::create_location::create_location;
use sqlbuild_orchestration::credentials::main::sessions::set_password::set_password;
use sqlbuild_orchestration::credentials::main::tokens::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::tokens::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::main::tokens::rotate_token::rotate_token;
use sqlbuild_orchestration::credentials::models::{
    AuditContext, PasswordAuthority, ServiceAccountAuthority, ServiceKeyRotation,
};
use sqlbuild_orchestration::declarations::main::change_trigger::change_trigger;
use sqlbuild_orchestration::declarations::models::TriggerChangeRequest;
use sqlbuild_orchestration::platform::main::work::location_work::location_work;
use sqlbuild_orchestration::platform::main::work::request_location_work::request_location_work;
use sqlbuild_orchestration::runs::main::execution::change_run::change_run;
use sqlbuild_orchestration::runs::main::requests::request_job_run::request_job_run;
use sqlbuild_orchestration::runs::main::requests::request_rerun::request_rerun;
use sqlbuild_orchestration::runs::models::JobRunRequest;
use sqlbuild_orchestration::runs::models::{RunAuthority, RunChange, RunChangeRequest};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::command::decision::control_admin;
use crate::native_commands::_helpers::location::execution::open_control_store;
use crate::native_commands::models::ControlAdmin;

pub(crate) async fn run_admin(namespace: &Namespace) -> Result<Value, ControlError> {
    let action = control_admin(namespace)?;
    let store = open_control_store(namespace, None).await?;
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    match action {
        ControlAdmin::BootstrapUser(name) => {
            let id = bootstrap_user(&store, &context, &name).await?;
            Ok(json!({"principal_id":id,"name":name,"kind":"person","role":"admin"}))
        }
        ControlAdmin::CreateLocation(name) => {
            let id = create_location(&store, &name, &context).await?;
            Ok(json!({"location_id": id, "name": name}))
        }
        ControlAdmin::CreatePrincipal(name) => {
            let id = create_principal(&store, &context, &name).await?;
            Ok(json!({"principal_id": id, "name": name}))
        }
        ControlAdmin::IssueToken(scope, expires_at) => {
            let token = issue_token(&store, &context, scope, expires_at).await?;
            Ok(json!(token))
        }
        ControlAdmin::RevokeToken(token_id) => {
            let revoked = revoke_token(&store, &context, token_id).await?;
            Ok(json!({"token_id": token_id, "revoked": revoked}))
        }
        ControlAdmin::RotateToken(token_id) => {
            Ok(json!(rotate_token(&store, &context, token_id).await?))
        }
        ControlAdmin::SetPassword(principal_id, password) => {
            set_password(
                &store,
                &context,
                PasswordAuthority::Operator(principal_id),
                &password,
            )
            .await?;
            Ok(json!({"principal_id": principal_id, "password_set": true}))
        }
        ControlAdmin::GrantAdmin(principal_id) => {
            grant_admin(&store, &context, principal_id).await?;
            Ok(json!({"principal_id": principal_id, "role": "admin"}))
        }
        ControlAdmin::RequestJobRun(request) => queue_job_run(&store, &context, &request).await,
        ControlAdmin::Rerun(request) => {
            let applied = request_rerun(&store, (&context, None), &request).await?;
            Ok(json!({"run_id": request.run_id, "applied": applied}))
        }
        ControlAdmin::RequestLocationWork(request) => {
            let applied = request_location_work(&store, (&context, None), &request).await?;
            Ok(json!({"request_id": request.request_id, "applied": applied}))
        }
        ControlAdmin::LocationWork(request_id) => {
            Ok(json!(location_work(&store, request_id).await?))
        }
        ControlAdmin::CancelRun {
            event_id,
            run_id,
            reason,
        } => {
            let applied = change_run(
                &store,
                RunAuthority::Operator(&context),
                &RunChangeRequest {
                    event_id,
                    run_id,
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: context.actor.clone(),
                        reason,
                    },
                },
            )
            .await?;
            Ok(json!({"event_id": event_id, "run_id": run_id, "applied": applied}))
        }
        ControlAdmin::ChangeTrigger {
            event_id,
            trigger_id,
            change,
            reason,
        } => {
            let applied = change_trigger(
                &store,
                &context,
                &TriggerChangeRequest {
                    event_id,
                    trigger_id,
                    actor: context.actor.clone(),
                    reason,
                    change,
                },
            )
            .await?;
            Ok(json!({"event_id": event_id, "trigger_id": trigger_id, "applied": applied}))
        }
        ControlAdmin::CreateServiceAccount { request, key } => {
            let owner = (&context, ServiceAccountAuthority::Owner);
            let issued = create_service_account(&store, owner, (&request, key.as_deref())).await?;
            // An imported key is the operator's own; it isn't echoed into logs that capture the result.
            Ok(service_key_result(
                &issued.account,
                key.is_none().then_some(&issued.key),
            ))
        }
        ControlAdmin::RotateServiceAccount {
            name,
            overlap_minutes,
        } => {
            let principal_id = find_service_account(&store, &name).await?;
            let rotation = ServiceKeyRotation {
                overlap_minutes: Some(overlap_minutes),
            };
            let issued = rotate_service_key(
                &store,
                (&context, ServiceAccountAuthority::Owner),
                (principal_id, rotation),
            )
            .await?;
            Ok(service_key_result(&issued.account, Some(&issued.key)))
        }
        ControlAdmin::SetServiceAccountDisabled { name, disabled } => {
            let principal_id = find_service_account(&store, &name).await?;
            let account = set_service_account_disabled(
                &store,
                (&context, ServiceAccountAuthority::Owner),
                (principal_id, disabled),
            )
            .await?;
            Ok(json!(account))
        }
        ControlAdmin::ListServiceAccounts => Ok(json!(
            list_service_accounts(&store, (&context, ServiceAccountAuthority::Owner)).await?
        )),
    }
}

/// A service account with its new key, which appears only here; the store keeps only its hash.
fn service_key_result(
    account: &sqlbuild_orchestration::credentials::models::ServiceAccountSummary,
    key: Option<&String>,
) -> Value {
    let mut result = json!({
        "principal_id": account.principal_id,
        "name": account.name,
        "role": account.role,
        "key_prefix": account.keys.first().map(|key| &key.prefix),
        "account": account,
    });
    if let Some(key) = key {
        result["key"] = json!(key);
    }
    result
}

/// A trusted job run, narrowed with the same planner the server resolves ad-hoc commands with.
async fn queue_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: &JobRunRequest,
) -> Result<Value, ControlError> {
    let planner = adhoc_planner();
    let applied = request_job_run(store, (context, Some(planner.as_ref())), request).await?;
    Ok(json!({"run_id": request.run_id, "applied": applied}))
}
