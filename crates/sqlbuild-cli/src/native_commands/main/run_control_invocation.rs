//! Control roles are served before Python/package discovery, including their JSON contract.

use sqlbuild_orchestration::api::main::openapi_document::openapi_document;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{flag, text};
use crate::native_commands::_helpers::terminal::{finish_lost_stdout, write_output};
use crate::native_commands::constants::{ADMIN_COMMAND, SCHEDULER_COMMAND, SERVER_COMMAND};
use crate::native_commands::main::_run_admin::run_admin;
use crate::native_commands::main::_run_control::run_control;

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
    let admin = command == ADMIN_COMMAND;
    let role = if admin { "administration" } else { "service" };
    let result = runtime.block_on(async {
        if admin {
            run_admin(namespace).await.map(Some)
        } else {
            run_control(namespace, command == SCHEDULER_COMMAND)
                .await
                .map(|()| None)
        }
    });
    Some(match result {
        Ok(output) => {
            let lost =
                output.and_then(|value| write_output(&[(Stream::Stdout, &format!("{value}\n"))]));
            if admin && lost.is_some() {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Control administration JSON result delivery failed; changes may already be committed\n",
                )]));
                return Some(1);
            }
            let exit_code = finish_lost_stdout(lost.as_ref(), 0);
            if exit_code == 0 {
                let message = if admin {
                    "Control administration completed successfully\n"
                } else {
                    "Control service stopped successfully\n"
                };
                drop(write_output(&[(Stream::Stderr, message)]));
            }
            exit_code
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
