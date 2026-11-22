//! What each step is called while it waits, runs and is done.

use std::time::{Duration, Instant};

use sqlbuild_core::screen_text::models::PhaseState;

use crate::dev_screen::models::DevStep;

/// `step`'s label in `state`: `Compile project`, `Compiling project`, `Compiled project`; a failed or warned step keeps the running form.
pub(crate) fn step_label(step: DevStep, state: PhaseState) -> &'static str {
    let (pending, running, done): (&str, &str, &str) = match step {
        DevStep::Config => (
            "Read configuration",
            "Reading configuration",
            "Read configuration",
        ),
        DevStep::ControlStore => (
            "Open control store",
            "Opening control store",
            "Opened control store",
        ),
        DevStep::Api => ("Start UI server", "Starting UI server", "Started UI server"),
        DevStep::Scheduler => ("Start scheduler", "Starting scheduler", "Started scheduler"),
        DevStep::Discover => (
            "Discover project",
            "Discovering project",
            "Discovered project",
        ),
        DevStep::Compile => ("Compile project", "Compiling project", "Compiled project"),
        DevStep::Publish => (
            "Publish snapshot",
            "Publishing snapshot",
            "Published snapshot",
        ),
        DevStep::Warehouse => (
            "Connect warehouse",
            "Connecting warehouse",
            "Connected warehouse",
        ),
    };
    match state {
        PhaseState::Pending => pending,
        PhaseState::Ok => done,
        PhaseState::Running { .. } | PhaseState::Warn | PhaseState::Fail => {
            if step == DevStep::Config {
                pending
            } else {
                running
            }
        }
    }
}

/// How long ago `started` was, if it was.
pub(crate) fn elapsed_since(started: Option<Instant>, now: Instant) -> Option<Duration> {
    started.map(|started| now.saturating_duration_since(started))
}
