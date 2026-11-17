//! Collapse the production API, scheduler and outbound location into one local process tree.

use std::net::SocketAddr;

use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Namespace;
use crate::dev_screen::main::start_dev_screen::start_dev_screen;
use crate::dev_screen::models::DevScreenHandle;
use crate::native_commands::_helpers::decision::{flag, text};
use crate::native_commands::_helpers::execution::run_development_roles;
<<<<<<< HEAD
use crate::native_commands::_helpers::project_facts::{
    development_project_name, development_target,
};
use crate::native_commands::_helpers::terminal::write_output;
use crate::server_config::models::ServerSection;
=======
use crate::native_commands::_helpers::execution::{
    announced_store, loopback_address, screen_setup,
};
use crate::native_commands::_helpers::project_facts::development_target;
>>>>>>> origin/main

pub(super) async fn run_dev(namespace: &Namespace) -> Result<(), ControlError> {
    let project =
        std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into()))?;
    let target = development_target(&project, text(namespace, "target"))?;
    let address: SocketAddr = loopback_address(namespace)?;
    let executable = std::env::current_exe()?;
<<<<<<< HEAD
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
    let location_id = development_location(
        &store,
        (
            &project.to_string_lossy(),
            &development_project_name(project)?,
        ),
        target,
        &context,
    )
    .await?;
    let scope = DevelopmentScope {
        location_id,
        target: target.into(),
        triggers: flag(namespace, "development_triggers"),
        principal_id: development_person(&store, &context).await?,
    };
    if !scope.triggers {
        pause_development_triggers(&store, (location_id, target)).await?;
=======
    let DevScreenHandle {
        messages,
        actions,
        keys,
        finish,
        task,
    } = start_dev_screen(screen_setup(&project, &target));
    let result: Result<(), ControlError> = async {
        let (store, scope) = announced_store(namespace, (&project, &target), &messages).await?;
        let listener = tokio::net::TcpListener::bind(address).await?;
        run_development_roles(
            (store, scope),
            (project.clone(), executable),
            listener,
            (
                !flag(namespace, "control_no_scheduler"),
                (messages.clone(), actions, keys),
            ),
        )
        .await
>>>>>>> origin/main
    }
    .await;
    let _ = finish.send(result.is_err());
    drop(task.await);
    result
}
