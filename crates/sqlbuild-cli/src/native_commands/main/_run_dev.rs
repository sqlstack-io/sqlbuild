//! Collapse the production API, scheduler and outbound location into one local process tree.

use std::net::SocketAddr;

use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Namespace;
use crate::dev_screen::main::start_dev_screen::start_dev_screen;
use crate::dev_screen::models::DevScreenHandle;
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::_helpers::command::project_facts::development_target;
use crate::native_commands::_helpers::location::execution::run_development_roles;
use crate::native_commands::_helpers::location::execution::{
    announced_store, loopback_address, screen_setup,
};

pub(super) async fn run_dev(namespace: &Namespace) -> Result<(), ControlError> {
    let project =
        std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into()))?;
    let target = development_target(&project, text(namespace, "target"))?;
    let address: SocketAddr = loopback_address(namespace)?;
    let executable = std::env::current_exe()?;
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
    }
    .await;
    let _ = finish.send(result.is_err());
    drop(task.await);
    result
}
