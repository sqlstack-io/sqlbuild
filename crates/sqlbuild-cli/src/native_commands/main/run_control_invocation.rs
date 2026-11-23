//! Control roles are served before Python/package discovery, including what `sqb admin` prints.

use sqlbuild_orchestration::api::main::openapi_document::openapi_document;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::admin::delivery::deliver_admin_result;
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::_helpers::command::store_migration::run_store_migration;
use crate::native_commands::_helpers::command::terminal::{finish_lost_stdout, write_output};
use crate::native_commands::_helpers::location::execution::run_control_action;
use crate::native_commands::constants::{ADMIN_COMMAND, SCHEDULER_COMMAND, SERVER_COMMAND};

pub(crate) fn run_control_invocation(namespace: &Namespace) -> Option<i32> {
    let command = text(namespace, "command")?;
    if !matches!(
        command.as_str(),
        SERVER_COMMAND | SCHEDULER_COMMAND | ADMIN_COMMAND
    ) {
        return None;
    }
    if flag(namespace, "control_openapi") {
        return Some(match serde_json::to_string(&openapi_document()) {
            Ok(document) => {
                let document = document + "\n";
                let lost = write_output(&[(Stream::Stdout, document.as_str())]);
                finish_lost_stdout(lost.as_ref(), 0)
            }
            Err(_) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Control API contract generation failed\n",
                )]));
                1
            }
        });
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Control service failed to start: {error}\n"),
            )]));
            return Some(1);
        }
    };
    if text(namespace, "server_command").as_deref() == Some("migrate") {
        return Some(run_store_migration(namespace, &runtime));
    }
    let admin = command == ADMIN_COMMAND;
    let role = if admin { "administration" } else { "service" };
    let result = runtime.block_on(run_control_action(namespace, &command));
    Some(match result {
        Ok(Some(done)) => deliver_admin_result(&done, flag(namespace, "json")),
        Ok(None) => {
            drop(write_output(&[(
                Stream::Stderr,
                "Control service stopped successfully\n",
            )]));
            0
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Control {role} failed: {error}\n"),
            )]));
            1
        }
    })
}
