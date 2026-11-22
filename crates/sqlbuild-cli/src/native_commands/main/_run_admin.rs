//! Direct database ownership authorizes bootstrap, not an untrusted HTTP request or supplied role.

use sqlbuild_orchestration::authorization::main::people::bootstrap_user::bootstrap_user;
use sqlbuild_orchestration::credentials::main::identities::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::locations::create_location::create_location;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::models::TriggerChangeRequest;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::admin::people::{provide_password, restore_admin};
use crate::native_commands::_helpers::admin::requests::{
    override_trigger, queue_job_run, queue_rerun, read_request, request_cancellation, request_check,
};
use crate::native_commands::_helpers::admin::store::open_admin_store;
use crate::native_commands::_helpers::command::admin_actions::{
    run_service_account_admin, run_token_admin,
};
use crate::native_commands::_helpers::command::decision::control_admin;
use crate::native_commands::models::{AdminDone, ControlAdmin};

pub(crate) async fn run_admin(namespace: &Namespace) -> Result<AdminDone, ControlError> {
    let action: ControlAdmin = control_admin(namespace)?;
    let store: ControlStore = open_admin_store(namespace).await?;
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    perform(&store, &context, action).await
}

async fn perform(
    store: &ControlStore,
    context: &AuditContext,
    action: ControlAdmin,
) -> Result<AdminDone, ControlError> {
    match action {
        ControlAdmin::BootstrapUser(name) => Ok(AdminDone::Bootstrapped {
            id: bootstrap_user(store, context, &name).await?,
            name,
        }),
        ControlAdmin::CreateLocation(name) => Ok(AdminDone::LocationCreated {
            id: create_location(store, &name, context).await?,
            name,
        }),
        ControlAdmin::CreatePrincipal(name) => Ok(AdminDone::PrincipalCreated {
            id: create_principal(store, context, &name).await?,
            name,
        }),
        ControlAdmin::Token(action) => run_token_admin(store, context, action).await,
        ControlAdmin::SetPassword(id, password) => {
            provide_password(store, context, (id, password)).await
        }
        ControlAdmin::GrantAdmin(id) => restore_admin(store, context, id).await,
        ControlAdmin::RequestJobRun(request) => queue_job_run(store, context, request).await,
        ControlAdmin::Rerun(request) => queue_rerun(store, context, request).await,
        ControlAdmin::RequestLocationWork(request) => request_check(store, context, request).await,
        ControlAdmin::LocationWork(request_id) => read_request(store, request_id).await,
        ControlAdmin::CancelRun {
            event_id,
            run_id,
            reason,
        } => request_cancellation(store, context, (event_id, run_id, reason)).await,
        ControlAdmin::ChangeTrigger {
            event_id,
            trigger_id,
            change,
            reason,
        } => {
            let request = TriggerChangeRequest {
                event_id,
                trigger_id,
                actor: context.actor.clone(),
                reason,
                change,
            };
            override_trigger(store, context, request).await
        }
        ControlAdmin::ServiceAccount(action) => {
            run_service_account_admin(store, context, action).await
        }
    }
}
