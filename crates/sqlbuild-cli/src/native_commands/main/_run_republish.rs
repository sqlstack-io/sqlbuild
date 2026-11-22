//! `sqb location republish`: 0 accepted or published, 1 failed or expired, 2 unanswered or still pending.

use std::io::Error;
use std::time::{Duration, Instant};

use sqlbuild_orchestration::platform::models::{
    PersonWorkRequest, RepublishState, RepublishStatus,
};
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream, Value};
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::republish_caller::{
    poll_republish, republish_caller, send_republish,
};
use crate::native_commands::constants::{REPUBLISH_POLL_SECONDS, REPUBLISH_TIMEOUT_SECONDS};

pub(super) fn run_republish(namespace: &Namespace) -> i32 {
    drop(write_output(&[(
        Stream::Stderr,
        "Requesting a republish…\n",
    )]));
    let waits: bool = flag(namespace, "location_wait");
    let result: Result<(RepublishStatus, String), Error> =
        republished(namespace, waits).and_then(|status| {
            let document: String = serde_json::to_string(&status).map_err(Error::other)?;
            Ok((status, document))
        });
    match result {
        Ok((status, document)) => {
            let (summary, code): (&str, i32) = match (status.state, waits) {
                (RepublishState::Published, _) => ("Republish published a snapshot", 0),
                (RepublishState::Failed, _) => ("Republish failed", 1),
                (RepublishState::Expired, _) => ("Republish expired before a location took it", 1),
                (RepublishState::Pending, false) => ("Republish accepted", 0),
                (RepublishState::Pending, true) => ("Republish is still pending", 2),
            };
            let detail: String = status
                .error
                .map(|error| format!(": {error}"))
                .unwrap_or_default();
            drop(write_output(&[
                (Stream::Stdout, &format!("{document}\n")),
                (Stream::Stderr, &format!("{summary}{detail}\n")),
            ]));
            code
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Republish request failed: {error}\n"),
            )]));
            2
        }
    }
}

fn republished(namespace: &Namespace, waits: bool) -> Result<RepublishStatus, Error> {
    let location: Uuid = text(namespace, "location_id")
        .unwrap_or_default()
        .parse()
        .map_err(|_| Error::other("--location-id must be a UUID"))?;
    let target: String = text(namespace, "target").unwrap_or_default();
    let timeout: u64 = match namespace.get("location_timeout") {
        Some(Value::Int(seconds)) => seconds
            .parse()
            .map_err(|_| Error::other("--timeout must be a number of seconds, zero or more"))?,
        _ => REPUBLISH_TIMEOUT_SECONDS,
    };
    let deadline: Instant = Instant::now()
        .checked_add(Duration::from_secs(timeout))
        .ok_or_else(|| Error::other("--timeout is too large"))?;
    let caller = republish_caller(
        text(namespace, "location_server_url").unwrap_or_default(),
        text(namespace, "login_ca").map(std::path::PathBuf::from),
    )?;
    let request = PersonWorkRequest {
        request_id: Uuid::now_v7(),
        reason: text(namespace, "location_reason")
            .unwrap_or_else(|| "Requested from the CLI".to_owned()),
    };
    let mut status: RepublishStatus = send_republish(&caller, (location, &target), &request)?;
    while waits && status.state == RepublishState::Pending {
        let left: u64 = deadline.saturating_duration_since(Instant::now()).as_secs();
        if left == 0 {
            break;
        }
        status = poll_republish(
            &caller,
            (location, &target, request.request_id),
            left.min(REPUBLISH_POLL_SECONDS),
        )?;
    }
    Ok(status)
}
