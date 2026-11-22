//! One task owns a warm interpreter; cancelled requests and source restarts always reap it.

use std::path::Path;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::dev_events::main::dev_event_line::dev_event_line;
use sqlbuild_orchestration::dev_events::models::DevEvent;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::compiler_discovery::{
    check_discovered, discovered, sleep_until,
};
use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
use crate::native_commands::_helpers::location::execution::exchange_reply;
use crate::native_commands::_helpers::location::execution::{
    open_location_runtime, stop_location_runtime,
};
use crate::native_commands::models::{
    LocationCompileRequest, LocationCompiler, LocationLoad, LocationPlanRequest,
    LocationRuntimeRequest, LocationServe,
};
use crate::native_commands::types::DiscoveryControls;

pub(crate) async fn run_location_compiler(
    options: &LocationServe,
    script: &Path,
    mut requests: mpsc::Receiver<LocationRuntimeRequest>,
    controls: DiscoveryControls<'_>,
) -> Result<(), ControlError> {
    let mut compiler: Option<LocationCompiler> = None;
    let mut sources: Vec<PathStamp> = Vec::new();
    let mut discovery: LocationDiscovery = LocationDiscovery::default();
    let result = async {
        loop {
            let due: Option<Instant> = discovery.next_due().filter(|_| compiler.is_some());
            let request: Option<LocationRuntimeRequest> = tokio::select! {
                request = requests.recv() => match request {
                    Some(request) => Some(request),
                    None => break,
                },
                () = sleep_until(due) => None,
            };
            let Some(request) = request else {
                compiler = check_discovered(compiler.take(), &mut discovery, controls).await?;
                continue;
            };
            let carried_on;
            (compiler, carried_on) = match request {
                LocationRuntimeRequest::Publish(request) => {
                    let before: Vec<PathStamp> =
                        std::mem::replace(&mut sources, request.python_sources.clone());
                    publish(
                        (options, script),
                        compiler.take(),
                        (request, &before),
                        (&mut discovery, controls),
                    )
                    .await?
                }
                LocationRuntimeRequest::Plan(request) => {
                    plan(
                        (options, script),
                        (compiler.take(), &sources),
                        (request, &requests),
                    )
                    .await?
                }
            };
            if !carried_on {
                break;
            }
        }
        Ok(())
    }
    .await;
    stop_location_runtime(compiler).await?;
    result
}

/// Publish one generation; false when its requester stopped waiting, which only shutdown does.
async fn publish(
    (options, script): (&LocationServe, &Path),
    mut compiler: Option<LocationCompiler>,
    (mut request, before): (LocationCompileRequest, &[PathStamp]),
    (discovery, (republish, (bearer, session_id))): (&mut LocationDiscovery, DiscoveryControls<'_>),
) -> Result<(Option<LocationCompiler>, bool), ControlError> {
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
    let publication = discovery.publish_request(
        request.generation,
        (before, &request.python_sources),
        republish.borrow().rediscovers,
    );
    let mut active = match compiler {
        Some(active) => active,
        None => open_location_runtime(options, script, request.python_sources.clone())?,
    };
    let result = tokio::select! {
        _ = request.reply.closed() => return Ok((Some(active), false)),
        result = exchange_reply::<LocationLoad>(&mut active, &publication, "publication record") => result,
    };
    if let Ok(load) = &result {
        let session = (bearer, session_id);
        discovered(options, session, (request.generation, load, &*discovery)).await;
        discovery.published(load, Instant::now());
    }
    let failed = result.is_err();
    let _ = request
        .reply
        .send(result.map_err(|error| error.to_string()));
    if failed {
        stop_location_runtime(Some(active)).await?;
        return Ok((None, true));
    }
    Ok((Some(active), true))
}

/// Plan one preview; an ended lease lets it finish so the held project survives, and only shutdown stops it.
async fn plan(
    (options, script): (&LocationServe, &Path),
    (compiler, sources): (Option<LocationCompiler>, &[PathStamp]),
    (mut request, requests): (LocationPlanRequest, &mpsc::Receiver<LocationRuntimeRequest>),
) -> Result<(Option<LocationCompiler>, bool), ControlError> {
    let mut active = match compiler {
        Some(active) => active,
        None => open_location_runtime(options, script, sources.to_vec())?,
    };
    let result = {
        let exchange = exchange_reply::<serde_json::Value>(&mut active, &request.request, "plan");
        tokio::pin!(exchange);
        tokio::select! {
            () = request.reply.closed() => {
                if requests.is_closed() {
                    None
                } else {
                    Some((&mut exchange).await)
                }
            },
            result = &mut exchange => Some(result),
        }
    };
    let Some(result) = result else {
        return Ok((Some(active), false));
    };
    let failed = result.is_err();
    let _ = request
        .reply
        .send(result.map_err(|error| error.to_string()));
    if failed {
        stop_location_runtime(Some(active)).await?;
        return Ok((None, true));
    }
    Ok((Some(active), true))
}
