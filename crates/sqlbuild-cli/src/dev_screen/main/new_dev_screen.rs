//! A `sqb dev` screen before its first step.

use std::collections::VecDeque;
use std::time::Instant;

use sqlbuild_core::screen_text::models::PhaseState;

use crate::dev_screen::constants::STEPS;
use crate::dev_screen::models::{DevScreen, ScreenSetup, StepRow};

/// Every step pending, the warehouse unknown and nothing drawn yet.
pub(crate) fn new_dev_screen(setup: ScreenSetup, started: Instant) -> DevScreen {
    DevScreen {
        live: setup.live,
        facts: setup.facts,
        header: setup.header,
        ui_url: String::new(),
        watching: setup.watching,
        started,
        steps: STEPS
            .iter()
            .map(|step| StepRow {
                step: *step,
                state: PhaseState::Pending,
                detail: String::new(),
                started: None,
                elapsed: None,
            })
            .collect(),
        ready: false,
        location: setup.location,
        adapter: String::new(),
        warehouse: None,
        generation_started: None,
        pending_changes: 0,
        serving: None,
        compile_detail: String::new(),
        project_root: setup.project_root,
        client_output: false,
        client_partial: false,
        event_gap: false,
        log: VecDeque::new(),
        show_log: false,
        frame: 0,
        drawn: 0,
    }
}
