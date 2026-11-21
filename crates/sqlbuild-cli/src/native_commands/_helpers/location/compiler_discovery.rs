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
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
use crate::native_commands::_helpers::location::execution::{
    exchange_reply, report_location_health, request_republish, stop_location_runtime,
};
use crate::native_commands::models::{LocationCompiler, LocationLoad, LocationServe};
use crate::native_commands::types::DiscoveryControls;

/// Wait until `due`, or forever without one.
pub(crate) async fn sleep_until(due: Option<Instant>) {
    match due {
        Some(due) => tokio::time::sleep_until(due).await,
        None => std::future::pending().await,
    }
}

/// Call the set due for its `check_every` again; republish when its result changed.
pub(crate) async fn check_discovered(
    compiler: Option<LocationCompiler>,
    discovery: &mut LocationDiscovery,
    (republish, _): DiscoveryControls<'_>,
) -> Result<Option<LocationCompiler>, ControlError> {
    let (Some(mut active), Some(check)) = (compiler, discovery.due_check(Instant::now())) else {
        return Ok(None);
    };
    let request: DiscoveredSetCheck = check;
    let name: String = request.check_discovered.clone();
    let reply: Result<DiscoveredSetChecked, ControlError> =
        exchange_reply(&mut active, &request, "discovered set check").await;
    let reply: DiscoveredSetChecked = match reply {
        Ok(reply) => reply,
        Err(_) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Checking discovered set {name} failed; the location restarts its interpreter.\n"
                ),
            )]));
            discovery.checked(
                &name,
                DiscoveredSetChecked {
                    set: None,
                    changed: false,
                },
                Instant::now(),
            );
            stop_location_runtime(Some(active)).await?;
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
    (generation, load): (i64, &LocationLoad),
) {
    if load.discovered.is_empty() || load.published.is_none() {
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
