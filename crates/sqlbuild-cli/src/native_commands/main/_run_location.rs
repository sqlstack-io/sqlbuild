//! Coalesce source edits and retain the last good publication across isolated compiler failures.

use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::time::{Instant, MissedTickBehavior};

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    location_generation, publish_child, shutdown_signal,
};
use crate::native_commands::_helpers::paths::{location_inputs, location_rules};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::models::LocationServe;

pub(super) async fn run_location(
    options: LocationServe,
    script: &Path,
    bearer: &str,
) -> Result<(), ControlError> {
    let rules = location_rules();
    let mut observed = location_inputs(&options.project_dir, &rules);
    let mut attempted = None;
    let mut changed_at = Instant::now();
    let mut retry_at = None;
    let mut clock = tokio::time::interval(Duration::from_millis(250));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let stopped = shutdown_signal();
    tokio::pin!(stopped);
    drop(write_output(&[(
        Stream::Stderr,
        "Starting code location; watching project inputs...\n",
    )]));
    loop {
        tokio::select! {
            result = &mut stopped => return result,
            _ = clock.tick() => {},
        }
        let current = location_inputs(&options.project_dir, &rules);
        if observed != current {
            observed = current;
            changed_at = Instant::now();
        }
        if attempted.as_ref() == Some(&observed)
            && retry_at.is_none_or(|retry| Instant::now() < retry)
        {
            continue;
        }
        if attempted.is_some() && changed_at.elapsed() < Duration::from_millis(250) {
            continue;
        }
        attempted = Some(observed.clone());
        let generation = tokio::select! {
            result = &mut stopped => return result,
            generation = location_generation(&options, bearer) => generation?,
        };
        let generation = match generation {
            Some(generation) => generation,
            None => {
                retry_at = Some(Instant::now() + Duration::from_secs(5));
                continue;
            }
        };
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Compiling location generation {generation} in an isolated interpreter...\n"),
        )]));
        let mut child = publish_child(&options, script, generation)?;
        let result = tokio::select! {
            result = &mut stopped => {
                child.kill().await?;
                return result;
            },
            result = child.wait() => result?,
        };
        if result.success() {
            retry_at = None;
            drop(write_output(&[(
                Stream::Stderr,
                "Code location ready; watching for changes.\n",
            )]));
        } else {
            retry_at = Some(Instant::now() + Duration::from_secs(30));
            drop(write_output(&[(
                Stream::Stderr,
                "Location reload failed; retaining last good snapshot. Retrying on change or in 30 seconds.\n",
            )]));
        }
    }
}
