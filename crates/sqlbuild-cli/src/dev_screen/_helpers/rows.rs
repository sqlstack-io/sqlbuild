//! The live rows of the `sqb dev` screen: a prompt, the running and waiting steps, and the key line, never taller than the terminal.

use std::time::Instant;

use sqlbuild_core::screen_text::main::render_key_line::render_key_line;
use sqlbuild_core::screen_text::main::render_prompt::render_prompt;
use sqlbuild_core::screen_text::main::render_step_line::render_step_line;
use sqlbuild_core::screen_text::models::{Hotkey, PhaseState, PromptBlock, StepLine};

use crate::dev_screen::_helpers::labels::{elapsed_since, step_label};
use crate::dev_screen::_helpers::secrets::secrets_rows;
use crate::dev_screen::constants::{READY_KEYS, STARTING_KEYS};
use crate::dev_screen::models::{DevScreen, StepRow};

/// The rows redrawn in place below everything printed for good.
pub(crate) fn live_rows(screen: &DevScreen, now: Instant) -> Vec<String> {
    if !screen.live {
        return Vec::new();
    }
    let facts = &screen.facts;
    if let Some(since) = screen.stopping {
        let line: StepLine = StepLine {
            state: PhaseState::Running {
                frame: screen.frame,
            },
            label: "Stopping sqb dev".to_owned(),
            elapsed: Some(now.saturating_duration_since(since)),
            detail: String::new(),
        };
        return render_step_line(&line, facts, false);
    }
    let mut prompt: Vec<String> = match &screen.prompt {
        Some(prompt) => render_prompt(
            &PromptBlock {
                title: sign_in_title(&screen.adapter),
                text: prompt.text.clone(),
                waited: now.saturating_duration_since(prompt.since),
            },
            facts,
        ),
        None => Vec::new(),
    };
    let (mut running, waiting): (Vec<String>, Vec<String>) = if screen.ready {
        (Vec::new(), Vec::new())
    } else {
        let rows = |wanted: fn(PhaseState) -> bool| -> Vec<String> {
            screen
                .steps
                .iter()
                .filter(|row| wanted(row.state))
                .flat_map(|row| render_step_line(&step_line(row, screen.frame, now), facts, false))
                .collect()
        };
        (
            rows(|state| matches!(state, PhaseState::Running { .. })),
            rows(|state| state == PhaseState::Pending),
        )
    };
    let secrets: Vec<String> = secrets_rows(screen);
    let key: String = render_key_line(&hotkeys(screen), facts);
    let budget: usize = facts.rows.saturating_sub(1).max(1);
    let fixed: usize = secrets.len() + 1;
    let room: usize = budget.saturating_sub(prompt.len() + running.len() + fixed);
    let waiting: Vec<String> = fit_waiting(waiting, room, facts);
    let over: usize = (prompt.len() + running.len() + waiting.len() + fixed).saturating_sub(budget);
    if over > 0 {
        let keep: usize = budget.saturating_sub(prompt.len() + fixed).max(1);
        running.truncate(keep);
        let over: usize = (prompt.len() + running.len() + fixed).saturating_sub(budget);
        prompt.drain(..over.min(prompt.len()));
    }
    let mut rows: Vec<String> = prompt;
    rows.extend(running);
    rows.extend(waiting);
    rows.extend(secrets);
    rows.push(key);
    let extra: usize = rows.len().saturating_sub(budget);
    rows.drain(..extra);
    rows
}

/// The waiting steps in `room` rows; when they do not fit, the first few and `· N more waiting`.
fn fit_waiting(
    waiting: Vec<String>,
    room: usize,
    facts: &sqlbuild_core::screen_text::models::ScreenFacts,
) -> Vec<String> {
    if waiting.len() <= room {
        return waiting;
    }
    if room == 0 {
        return Vec::new();
    }
    let shown: usize = room - 1;
    let mut rows: Vec<String> = waiting[..shown].to_vec();
    rows.push(
        facts
            .style
            .muted(&format!("\u{b7} {} more waiting", waiting.len() - shown)),
    );
    rows
}

/// One step as a live row: a running step shows the current spinner frame and its live timer.
pub(crate) fn step_line(row: &StepRow, frame: usize, now: Instant) -> StepLine {
    let state: PhaseState = match row.state {
        PhaseState::Running { .. } => PhaseState::Running { frame },
        state => state,
    };
    StepLine {
        state,
        label: step_label(row.step, state).to_owned(),
        elapsed: row.elapsed.or_else(|| match state {
            PhaseState::Pending => None,
            _ => elapsed_since(row.started, now),
        }),
        detail: if state == PhaseState::Pending && row.detail.is_empty() {
            "waiting".to_owned()
        } else {
            row.detail.clone()
        },
    }
}

/// `snowflake needs you to sign in`, or a general title before the adapter is known.
pub(crate) fn sign_in_title(adapter: &str) -> String {
    if adapter.is_empty() {
        return "the warehouse client is waiting for you".to_owned();
    }
    format!("{adapter} needs you to sign in")
}

/// The keys for where the session stands.
pub(crate) fn hotkeys(screen: &DevScreen) -> Vec<Hotkey> {
    let keys: Vec<(char, &str)> = if screen.ready {
        READY_KEYS.to_vec()
    } else {
        STARTING_KEYS.to_vec()
    };
    keys.into_iter()
        .map(|(key, label)| Hotkey {
            key,
            label: label.to_owned(),
        })
        .collect()
}
