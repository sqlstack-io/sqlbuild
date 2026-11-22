//! The warning `sqb dev` shows while providers miss secrets: their variables and how to set them, until a health read shows none.

use sqlbuild_core::screen_text::main::render_step_line::render_step_line;
use sqlbuild_core::screen_text::models::{PhaseState, StepLine};
use sqlbuild_orchestration::dev_events::models::UnsetProviderSecrets;

use crate::dev_screen::constants::{SECRETS_FIX, SECRETS_MISSING, SECRETS_SET};
use crate::dev_screen::models::DevScreen;

/// Each provider and the variables it misses, then how to set them; empty when none is missing.
pub(crate) fn secrets_rows(screen: &DevScreen) -> Vec<String> {
    if screen.missing_secrets.is_empty() {
        return Vec::new();
    }
    let providers: Vec<String> = screen
        .missing_secrets
        .iter()
        .map(|provider| format!("{} {}", provider.provider, provider.variables.join(", ")))
        .collect();
    let line: StepLine = StepLine {
        state: PhaseState::Warn,
        label: SECRETS_MISSING.to_owned(),
        elapsed: None,
        detail: format!("{}; {SECRETS_FIX}", providers.join(" \u{b7} ")),
    };
    render_step_line(&line, &screen.facts, true)
}

/// Take the latest health read's providers missing secrets. A live screen redraws its warning row; a plain one prints the warning when it changes and a line when it clears.
pub(crate) fn secrets_reported(
    mut screen: DevScreen,
    providers: Vec<UnsetProviderSecrets>,
) -> (DevScreen, Vec<String>) {
    if providers == screen.missing_secrets {
        return (screen, Vec::new());
    }
    let had_missing: bool = !screen.missing_secrets.is_empty();
    screen.missing_secrets = providers;
    if screen.live {
        return (screen, Vec::new());
    }
    if !screen.missing_secrets.is_empty() {
        let lines: Vec<String> = secrets_rows(&screen);
        return (screen, lines);
    }
    if !had_missing {
        return (screen, Vec::new());
    }
    let line: StepLine = StepLine {
        state: PhaseState::Ok,
        label: SECRETS_SET.to_owned(),
        elapsed: None,
        detail: "no provider is missing a secret".to_owned(),
    };
    let lines: Vec<String> = render_step_line(&line, &screen.facts, true);
    (screen, lines)
}
