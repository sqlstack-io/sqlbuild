//! A completed synchronization resumes the maintained models whose pausing builds stopped heartbeating.

use std::time::Duration;

use sqlbuild_orchestration::warehouse_metadata::constants::MAINTAINED_RESUME_REQUEST;
use sqlbuild_orchestration::warehouse_metadata::models::{
    MaintainedResumeLocation, MaintainedResumeReply,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::location_warning;
use crate::native_commands::constants::{MAINTAINED_RESUME_SECONDS, METADATA_REPLY_BYTES};
use crate::native_commands::models::{LocationCompiler, LocationMetadataSync};

/// Ask the reader to resume expired holds in the synced locations; a failure only warns.
///
/// Each location sends the pause table token its last quiet pass returned, so a table no build wrote
/// since is not read and a quiet sync wakes no warehouse.
pub(crate) async fn resume_expired_holds(state: &mut LocationMetadataSync) {
    if state.halted {
        return;
    }
    let Some(reader) = state.reader.as_mut() else {
        return;
    };
    let locations: Vec<MaintainedResumeLocation> = state
        .locations
        .iter()
        .map(|location| MaintainedResumeLocation {
            location: location.clone(),
            quiet_token: state.maintained_quiet_tokens.get(location).cloned(),
        })
        .collect();
    let request = serde_json::json!({ MAINTAINED_RESUME_REQUEST: locations }).to_string();
    let reason = match tokio::time::timeout(
        Duration::from_secs(MAINTAINED_RESUME_SECONDS),
        exchange(reader, request),
    )
    .await
    {
        Ok(Ok(MaintainedResumeReply::Resumed(resumed))) => {
            for (location, token) in state.locations.iter().zip(resumed.quiet_tokens) {
                match token {
                    Some(token) => state
                        .maintained_quiet_tokens
                        .insert(location.clone(), token),
                    None => state.maintained_quiet_tokens.remove(location),
                };
            }
            for hold in resumed.holds {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!(
                        "Resumed maintained model {}: run {} stopped heartbeating.\n",
                        hold.relation, hold.run_id
                    ),
                )]));
            }
            return;
        }
        Ok(Ok(MaintainedResumeReply::Failed(reason))) => reason,
        Ok(Err(reason)) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Resuming expired maintained holds failed: {reason}\n"),
            )]));
            reason
        }
        Err(_) => {
            let reason = format!("no reply in {MAINTAINED_RESUME_SECONDS} s");
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Resuming expired maintained holds failed: {reason}\n"),
            )]));
            reason
        }
    };
    location_warning("maintained resume failed", reason);
}

/// One request line and its bounded reply line.
async fn exchange(
    reader: &mut LocationCompiler,
    request: String,
) -> Result<MaintainedResumeReply, String> {
    let fail = |error: std::io::Error| error.to_string();
    reader
        .stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .map_err(fail)?;
    reader.stdin.flush().await.map_err(fail)?;
    let mut line = String::new();
    let read = (&mut reader.stdout)
        .take(METADATA_REPLY_BYTES + 1)
        .read_line(&mut line)
        .await
        .map_err(fail)?;
    if read == 0 {
        return Err("location runtime exited before replying".into());
    }
    serde_json::from_str(&line).map_err(|_| "invalid location reply".into())
}
