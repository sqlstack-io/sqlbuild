//! One task owns a warm interpreter; cancelled requests and source restarts always reap it.

use std::path::Path;

use sqlbuild_orchestration::dev_events::main::dev_event_line::dev_event_line;
use sqlbuild_orchestration::dev_events::models::DevEvent;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::exchange_location_generation;
use crate::native_commands::_helpers::execution::{open_location_runtime, stop_location_runtime};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::models::{LocationCompileRequest, LocationCompiler, LocationServe};

pub(crate) async fn run_location_compiler(
    options: &LocationServe,
    script: &Path,
    mut requests: mpsc::Receiver<LocationCompileRequest>,
) -> Result<(), ControlError> {
    let mut compiler: Option<LocationCompiler> = None;
    let result = async {
        while let Some(mut request) = requests.recv().await {
            if compiler
                .as_ref()
                .is_some_and(|compiler| compiler.python_sources != request.python_sources)
            {
                stop_location_runtime(compiler.take()).await?;
                if options.dev_events {
                    drop(write_output(&[(
                        Stream::Stdout,
                        &dev_event_line(&DevEvent::Restarted {
                            reason: "Python changed".into(),
                        }),
                    )]));
                }
            }
            if compiler.is_none() {
                compiler = Some(open_location_runtime(
                    options,
                    script,
                    request.python_sources,
                )?);
            }
            let active = compiler
                .as_mut()
                .ok_or_else(|| ControlError::Invalid("location compiler is unavailable".into()))?;
            let result = tokio::select! {
                _ = request.reply.closed() => break,
                result = exchange_location_generation(active, request.generation) => result,
            };
            let failed = result.is_err();
            let _ = request
                .reply
                .send(result.map_err(|error| error.to_string()));
            if failed {
                stop_location_runtime(compiler.take()).await?;
            }
        }
        Ok(())
    }
    .await;
    stop_location_runtime(compiler).await?;
    result
}
