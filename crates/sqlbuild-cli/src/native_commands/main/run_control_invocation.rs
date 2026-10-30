//! Control roles are served before Python/package discovery, including their JSON contract.

use sqlbuild_orchestration::api::main::openapi_document::openapi_document;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{flag, text};
use crate::native_commands::_helpers::terminal::{finish_lost_stdout, write_output};
use crate::native_commands::constants::{SCHEDULER_COMMAND, SERVER_COMMAND};
use crate::native_commands::main::_run_control::run_control;

pub(crate) fn run_control_invocation(namespace: &Namespace) -> Option<i32> {
    let command = text(namespace, "command")?;
    if !matches!(command.as_str(), SERVER_COMMAND | SCHEDULER_COMMAND) {
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
    Some(
        match runtime.block_on(run_control(namespace, command == SCHEDULER_COMMAND)) {
            Ok(()) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Control service stopped successfully\n",
                )]));
                0
            }
            Err(error) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("Control service failed: {error}\n"),
                )]));
                1
            }
        },
    )
}
