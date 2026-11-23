//! The compiler task's interpreter restarts when what it loaded at start-up changed.

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::dev_events::main::dev_event_line::dev_event_line;
use sqlbuild_orchestration::dev_events::models::DevEvent;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::env_reload::reload_env_file;
use crate::native_commands::_helpers::location::execution::stop_location_runtime;
use crate::native_commands::models::{LocationCompiler, LocationServe};

/// `compiler`, or none once it was stopped because Python or, under `sqb dev`, the env file changed.
pub(crate) async fn restarted_for_changes(
    options: &LocationServe,
    mut compiler: Option<LocationCompiler>,
    python_sources: &[PathStamp],
) -> Result<Option<LocationCompiler>, ControlError> {
    let env_changed: bool = reload_env_file(options);
    let python_changed: bool = compiler
        .as_ref()
        .is_some_and(|compiler| compiler.python_sources != python_sources);
    if compiler.is_some() && (python_changed || env_changed) {
        stop_location_runtime(compiler.take()).await?;
        if options.dev_events {
            let reason: &str = if python_changed {
                "Python changed"
            } else {
                "env file changed"
            };
            drop(write_output(&[(
                Stream::Stdout,
                &dev_event_line(&DevEvent::Restarted {
                    reason: reason.into(),
                }),
            )]));
        }
    }
    Ok(compiler)
}
