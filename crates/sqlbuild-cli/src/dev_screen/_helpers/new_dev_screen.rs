//! A `sqb dev` screen before its first step.

use std::collections::VecDeque;

use crate::dev_screen::_helpers::transitions::pending;
use crate::dev_screen::constants::STEPS;
use crate::dev_screen::models::{DevScreen, ScreenSetup};

/// Every step pending, the warehouse unknown and nothing drawn yet.
pub(crate) fn new_dev_screen(setup: ScreenSetup) -> DevScreen {
    DevScreen {
        live: setup.live,
        facts: setup.facts,
        header: setup.header,
        header_printed: false,
        ui_url: String::new(),
        location: String::new(),
        env_file: setup.env_file,
        triggers: String::new(),
        steps: STEPS.into_iter().map(pending).collect(),
        ready: false,
        adapter: setup.adapter,
        warehouse: None,
        generation_started: None,
        pending_changes: 0,
        serving: None,
        compile_detail: String::new(),
        project_root: setup.project_root,
        prompt: None,
        client_partial: false,
        stopping: None,
        log: VecDeque::new(),
        show_log: false,
        frame: 0,
        drawn: Vec::new(),
    }
}
