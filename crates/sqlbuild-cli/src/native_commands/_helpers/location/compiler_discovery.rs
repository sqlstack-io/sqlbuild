//! The compiler task's discovered sets: `check_every` checks, and what a publication reports of them.

use sqlbuild_orchestration::dev_events::main::dev_event_line::dev_event_line;
use sqlbuild_orchestration::dev_events::models::DevEvent;
use sqlbuild_orchestration::discovered_sets::main::discovery_health::discovery_health;
use sqlbuild_orchestration::discovered_sets::main::discovery_summary::discovery_summary;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredSetCheck, DiscoveredSetChecked, DiscoveredSetStatus,
};
use sqlbuild_orchestration::platform::models::LocationHealth;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
use crate::native_commands::_helpers::location::execution::{
    exchange_reply, report_location_health, request_republish, stop_location_runtime,
};
use crate::native_commands::models::{
    LocationCompiler, LocationLoad, LocationRuntimeRequest, LocationServe,
};
use crate::native_commands::types::DiscoveryControls;

/// How often a running check looks for shutdown or a waiting request.
const REQUEST_POLL: std::time::Duration = std::time::Duration::from_millis(100);
/// How long a running check may go on once a request waits for the interpreter it holds.
const CHECK_GRACE: std::time::Duration = std::time::Duration::from_secs(10);

/// Wait until every sender of `requests` is gone, which only shutdown does, without taking a request.
async fn shut_down(requests: &mpsc::Receiver<LocationRuntimeRequest>) {
    while !requests.is_closed() {
        tokio::time::sleep(REQUEST_POLL).await;
    }
}

/// Wait until a request has waited out the grace of the check, without taking the request.
async fn waited_out(requests: &mpsc::Receiver<LocationRuntimeRequest>) {
    while requests.is_empty() {
        tokio::time::sleep(REQUEST_POLL).await;
    }
    tokio::time::sleep(CHECK_GRACE).await;
}

/// Wait until `due`, or forever without one.
pub(crate) async fn sleep_until(due: Option<Instant>) {
    match due {
        Some(due) => tokio::time::sleep_until(due).await,
        None => std::future::pending().await,
    }
}

/// Call the set due for its `check_every` again; republish when its result changed, or with discovery when the check crashed, so the publication reports the set. Shutdown, or a request waiting out its grace, stops it.
pub(crate) async fn check_discovered(
    compiler: Option<LocationCompiler>,
    discovery: &mut LocationDiscovery,
    ((republish, _), requests): (
        DiscoveryControls<'_>,
        &mpsc::Receiver<LocationRuntimeRequest>,
    ),
) -> Result<Option<LocationCompiler>, ControlError> {
    let (Some(mut active), Some(check)) = (compiler, discovery.due_check(Instant::now())) else {
        return Ok(None);
    };
    let request: DiscoveredSetCheck = check;
    let name: String = request.check_discovered.clone();
    let exchanged: Option<Option<Result<DiscoveredSetChecked, ControlError>>> = tokio::select! {
        biased;
        reply = exchange_reply(&mut active, &request, "discovered set check") => Some(Some(reply)),
        () = shut_down(requests) => None,
        () = waited_out(requests) => Some(None),
    };
    let Some(reply) = exchanged else {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Stopped checking discovered set {name}; the location is shutting down.\n"),
        )]));
        stop_location_runtime(Some(active)).await?;
        return Ok(None);
    };
    let reply: DiscoveredSetChecked = match reply {
        Some(Ok(reply)) => reply,
        unfinished => {
            let crashed: bool = unfinished.is_some();
            let problem: String = match unfinished {
                Some(Err(error)) => format!(
                    "Checking discovered set {name} failed: {error}; the location restarts its interpreter and discovers again"
                ),
                _ => format!(
                    "Checking discovered set {name} did not finish within {} seconds of a request waiting for it; the location restarts its interpreter",
                    CHECK_GRACE.as_secs()
                ),
            };
            drop(write_output(&[(Stream::Stderr, &format!("{problem}.\n"))]));
            discovery.checked(
                &name,
                DiscoveredSetChecked {
                    set: None,
                    changed: false,
                },
                Instant::now(),
            );
            stop_location_runtime(Some(active)).await?;
            if crashed {
                request_republish(republish, true);
            }
            return Ok(None);
        }
    };
    if let Some(set) = reply
        .set
        .as_ref()
        .filter(|set| set.status == DiscoveredSetStatus::Failed)
    {
        drop(write_output(&[(
            Stream::Stderr,
            &format!(
                "Discovered set {name} check failed: {}\n",
                set.error.as_deref().unwrap_or("no reason given")
            ),
        )]));
    }
    if discovery.checked(&name, reply, Instant::now()) {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Discovered set {name} changed; publishing it.\n"),
        )]));
        request_republish(republish, false);
    }
    Ok(Some(active))
}

/// Report a publication's sets as health, and to `sqb dev` as its ready card's line.
pub(crate) async fn discovered(
    options: &LocationServe,
    (bearer, session_id): (&str, Uuid),
    (generation, load, discovery): (i64, &LocationLoad, &LocationDiscovery),
) {
    if !discovery.reports(load) {
        return;
    }
    if options.dev_events
        && let Some(summary) = discovery_summary(&load.discovered)
    {
        drop(write_output(&[(
            Stream::Stdout,
            &dev_event_line(&DevEvent::Discovered { summary }),
        )]));
    }
    let health = LocationHealth::Discovery {
        generation,
        sets: discovery_health(&load.discovered),
    };
    report_location_health(options, bearer, session_id, health).await;
}
