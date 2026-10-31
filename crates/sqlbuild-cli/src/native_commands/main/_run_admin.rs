//! Direct database ownership authorizes bootstrap, not an untrusted HTTP request or supplied role.

use serde_json::{Value, json};
use sqlbuild_orchestration::credentials::main::create_location::create_location;
use sqlbuild_orchestration::credentials::main::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::main::change_trigger::change_trigger;
use sqlbuild_orchestration::declarations::models::TriggerChangeRequest;
use sqlbuild_orchestration::platform::main::location_work::location_work;
use sqlbuild_orchestration::platform::main::request_location_work::request_location_work;
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::request_job_run::request_job_run;
use sqlbuild_orchestration::runs::main::request_rerun::request_rerun;
use sqlbuild_orchestration::runs::models::{RunAuthority, RunChange, RunChangeRequest};
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
        ControlAdmin::CreatePrincipal(name, kind) => {
            let id = create_principal(&store, &context, (&name, kind)).await?;
            Ok(json!({"principal_id": id, "name": name, "kind": kind}))
        }
        ControlAdmin::IssueToken(scope, expires_at) => {
            let token = issue_token(&store, &context, scope, expires_at).await?;
            Ok(json!(token))
        }
        ControlAdmin::RevokeToken(token_id) => {
            let revoked = revoke_token(&store, &context, token_id).await?;
            Ok(json!({"token_id": token_id, "revoked": revoked}))
        }
        ControlAdmin::RequestJobRun(request) => {
            let applied = request_job_run(&store, &context, &request).await?;
            Ok(json!({"run_id": request.run_id, "applied": applied}))
        }
        ControlAdmin::Rerun(request) => {
            let applied = request_rerun(&store, &context, &request).await?;
            Ok(json!({"run_id": request.run_id, "applied": applied}))
        }
        ControlAdmin::RequestLocationWork(request) => {
            let applied = request_location_work(&store, &context, &request).await?;
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
    }
}
