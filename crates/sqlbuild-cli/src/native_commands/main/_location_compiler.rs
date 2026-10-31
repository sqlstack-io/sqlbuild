//! One task owns a warm interpreter; cancelled requests and source restarts always reap it.

use std::path::Path;

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::io::BufReader;
use tokio::sync::mpsc;

use crate::native_commands::_helpers::execution::{
    exchange_location_generation, location_runtime_child,
};
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
                stop_compiler(compiler.take()).await?;
            }
            if compiler.is_none() {
                compiler = Some(open_compiler(options, script, request.python_sources)?);
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
                stop_compiler(compiler.take()).await?;
            }
        }
        Ok(())
    }
    .await;
    stop_compiler(compiler).await?;
    result
}

fn open_compiler(
    options: &LocationServe,
    script: &Path,
    python_sources: Vec<sqlbuild_cache::project_snapshot::models::PathStamp>,
) -> Result<LocationCompiler, ControlError> {
    let mut child = location_runtime_child(options, script)?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ControlError::Invalid("location interpreter input is unavailable".into()))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ControlError::Invalid("location interpreter output is unavailable".into())
    })?;
    Ok(LocationCompiler {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        python_sources,
    })
}

async fn stop_compiler(compiler: Option<LocationCompiler>) -> Result<(), ControlError> {
    if let Some(mut compiler) = compiler {
        if compiler.child.try_wait()?.is_none() {
            compiler.child.kill().await?;
        } else {
            compiler.child.wait().await?;
        }
    }
    Ok(())
}
