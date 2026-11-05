//! Collapse the production API, scheduler and outbound location into one local process tree.

use std::path::Path;

use sqlbuild_orchestration::api::models::DevelopmentScope;
use sqlbuild_orchestration::credentials::main::locations::development_location::development_location;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::main::pause_development_triggers::pause_development_triggers;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::set_release_version::set_release_version;
use sqlbuild_orchestration::store::main::{migrate_store::migrate_store, open_store::open_store};
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{control_address, flag, text};
use crate::native_commands::_helpers::execution::run_development_roles;
use crate::native_commands::_helpers::project_facts::development_target;
use crate::native_commands::_helpers::terminal::write_output;

pub(super) async fn run_dev(namespace: &Namespace) -> Result<(), ControlError> {
    let project =
        std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into()))?;
    let target = development_target(&project, text(namespace, "target"))?;
    let address = control_address(namespace, false)?
        .ok_or_else(|| ControlError::Invalid("development address is required".into()))?;
    if !address.ip().is_loopback() {
        return Err(ControlError::Invalid(
            "sqb dev only listens on loopback".into(),
        ));
    }
    let executable = std::env::current_exe()?;
    drop(write_output(&[(
        Stream::Stderr,
        "Starting development control store...\n",
    )]));
    let (store, scope) = development_store(namespace, &project, &target).await?;
    drop(write_output(&[(
        Stream::Stderr,
        "Development control store ready\nStarting development API...\n",
    )]));
    let listener = tokio::net::TcpListener::bind(address).await?;
    run_development_roles(
        (store, scope),
        (project, executable),
        listener,
        !flag(namespace, "control_no_scheduler"),
    )
    .await
}

/// Complete retained identity and startup pauses before starting any scheduling roles.
async fn development_store(
    namespace: &Namespace,
    project: &Path,
    target: &str,
) -> Result<(ControlStore, DevelopmentScope), ControlError> {
    let database = text(namespace, "development_store_url").unwrap_or_else(|| {
        project
            .join(".sqlbuild/control.sqlite3")
            .to_string_lossy()
            .into_owned()
    });
    set_release_version(env!("CARGO_PKG_VERSION"));
    let store = open_store(&database).await?;
    migrate_store(&store).await?;
    let context = AuditContext {
        actor: "local-development".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let location_id =
        development_location(&store, &project.to_string_lossy(), target, &context).await?;
    let scope = DevelopmentScope {
        location_id,
        target: target.into(),
        triggers: flag(namespace, "development_triggers"),
    };
    if !scope.triggers {
        pause_development_triggers(&store, (location_id, target)).await?;
    }
    Ok((store, scope))
}
