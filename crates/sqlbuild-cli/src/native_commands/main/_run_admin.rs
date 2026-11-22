//! Direct database ownership authorizes bootstrap, not an untrusted HTTP request or supplied role.

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use sqlbuild_orchestration::authorization::main::people::bootstrap_user::bootstrap_user;
use sqlbuild_orchestration::authorization::main::roles::grant_admin::grant_admin;
use sqlbuild_orchestration::credentials::main::identities::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::locations::create_location::create_location;
use sqlbuild_orchestration::credentials::main::sessions::set_password::set_password;
use sqlbuild_orchestration::credentials::models::{AuditContext, PasswordAuthority};
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
use crate::native_commands::_helpers::admin::store::open_admin_store;
use crate::native_commands::_helpers::command::admin_actions::{
    name_or_id, run_service_account_admin, run_token_admin,
};
use crate::native_commands::_helpers::command::decision::control_admin;
use crate::native_commands::models::{AdminDone, ControlAdmin};

pub(crate) async fn run_admin(namespace: &Namespace) -> Result<AdminDone, ControlError> {
    let action = control_admin(namespace)?;
    let store = open_admin_store(namespace).await?;
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    match action {
        ControlAdmin::BootstrapUser(name) => Ok(AdminDone::Bootstrapped {
            id: bootstrap_user(&store, &context, &name).await?,
            name,
        }),
        ControlAdmin::CreateLocation(name) => Ok(AdminDone::LocationCreated {
            id: create_location(&store, &name, &context).await?,
            name,
        }),
        ControlAdmin::CreatePrincipal(name) => Ok(AdminDone::PrincipalCreated {
            id: create_principal(&store, &context, &name).await?,
            name,
        }),
        ControlAdmin::Token(action) => run_token_admin(&store, &context, action).await,
        ControlAdmin::SetPassword(id, password) => {
            let name = name_or_id(&store, id).await?;
            let authority = PasswordAuthority::Operator(id);
            set_password(&store, &context, authority, &password).await?;
            Ok(AdminDone::PasswordSet { id, name })
        }
        ControlAdmin::GrantAdmin(id) => {
            let name = name_or_id(&store, id).await?;
            grant_admin(&store, &context, id).await?;
            Ok(AdminDone::AdminGranted { id, name })
        }
        ControlAdmin::RequestJobRun(request) => queue_job_run(&store, &context, request).await,
        ControlAdmin::Rerun(request) => Ok(AdminDone::RerunQueued {
            run_id: request.run_id,
            source: request.source_run_id,
            applied: request_rerun(&store, (&context, None), &request).await?,
        }),
        ControlAdmin::RequestLocationWork(request) => Ok(AdminDone::CheckRequested {
            location: name_or_id(&store, request.location_id).await?,
            applied: request_location_work(&store, (&context, None), &request).await?,
            request_id: request.request_id,
            target: request.target,
        }),
        ControlAdmin::LocationWork(request_id) => Ok(AdminDone::WorkRead {
            request_id,
            record: location_work(&store, request_id).await?.map(Box::new),
            now: chrono::Utc::now(),
        }),
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
            Ok(AdminDone::CancelRequested {
                event_id,
                run_id,
                applied,
            })
        }
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
            let applied = change_trigger(&store, &context, &request).await?;
            Ok(AdminDone::TriggerChanged {
                event_id,
                trigger_id,
                change: request.change,
                applied,
            })
        }
        ControlAdmin::ServiceAccount(action) => {
            run_service_account_admin(&store, &context, action).await
        }
    }
}

/// A trusted job run, narrowed with the same planner the server resolves ad-hoc commands with.
async fn queue_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: JobRunRequest,
) -> Result<AdminDone, ControlError> {
    let planner = adhoc_planner();
    let applied = request_job_run(store, (context, Some(planner.as_ref())), &request).await?;
    Ok(AdminDone::RunQueued {
        run_id: request.run_id,
        job: request.job_key,
        target: request.target,
        applied,
    })
}
