//! A completed synchronization resumes the dynamic models whose pausing builds stopped heartbeating.

use std::time::Duration;

use sqlbuild_orchestration::warehouse_metadata::constants::DYNAMIC_RESUME_REQUEST;
use sqlbuild_orchestration::warehouse_metadata::models::{
    DynamicResumeLocation, DynamicResumeReply,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::location_warning;
use crate::native_commands::constants::{DYNAMIC_RESUME_SECONDS, METADATA_REPLY_BYTES};
use crate::native_commands::models::{LocationCompiler, LocationMetadataSync};

/// Ask the reader to resume expired holds, sending each location's quiet token; a failure only warns.
pub(crate) async fn resume_expired_holds(mut state: LocationMetadataSync) -> LocationMetadataSync {
    if state.halted {
        return state;
    }
    let Some(reader) = state.reader.take() else {
        return state;
    };
    let locations: Vec<DynamicResumeLocation> = state
        .locations
        .iter()
        .chain(
            state
                .dynamic_quiet_tokens
                .keys()
                .filter(|location| !state.locations.contains(location)),
        )
        .map(|location| DynamicResumeLocation {
            location: location.clone(),
            quiet_token: state.dynamic_quiet_tokens.get(location).cloned(),
        })
        .collect();
    let request = serde_json::json!({ DYNAMIC_RESUME_REQUEST: locations }).to_string();
    let reason = match tokio::time::timeout(
        Duration::from_secs(DYNAMIC_RESUME_SECONDS),
        exchange(reader, request),
    )
    .await
    {
        Ok((reader, Ok(DynamicResumeReply::Resumed(resumed)))) => {
            state.reader = Some(reader);
            state.dynamic_quiet_tokens = resumed
                .quiet_tokens
                .into_iter()
                .filter_map(|resume| Some((resume.location, resume.quiet_token?)))
                .collect();
            for failure in &resumed.failures {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("Resuming an expired dynamic hold failed: {failure}\n"),
                )]));
            }
            for hold in resumed.holds {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!(
                        "Resumed dynamic model {}: run {} stopped heartbeating.\n",
                        hold.relation, hold.run_id
                    ),
                )]));
            }
            return state;
        }
        Ok((reader, Ok(DynamicResumeReply::Failed(reason)))) => {
            state.reader = Some(reader);
            reason
        }
        Ok((reader, Err(reason))) => {
            state.reader = Some(reader);
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Resuming expired dynamic holds failed: {reason}\n"),
            )]));
            reason
        }
        Err(_) => {
            let reason = format!("no reply in {DYNAMIC_RESUME_SECONDS} s");
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Resuming expired dynamic holds failed: {reason}\n"),
            )]));
            reason
        }
    };
    location_warning("dynamic resume failed", reason);
    state
}

/// One request line and its bounded reply line, handing the reader back.
async fn exchange(
    mut reader: LocationCompiler,
    request: String,
) -> (LocationCompiler, Result<DynamicResumeReply, String>) {
    let reply: Result<DynamicResumeReply, String> = async {
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
        serde_json::from_str(&line).map_err(|_| "invalid location reply".to_owned())
    }
    .await;
    (reader, reply)
}
