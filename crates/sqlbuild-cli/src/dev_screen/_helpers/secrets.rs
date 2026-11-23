//! What `sqb dev` shows while providers miss secrets: a warning block when that changes, and a row until a health read shows none.

use std::collections::BTreeSet;

use sqlbuild_core::screen_text::main::render_problem::render_problem;
use sqlbuild_core::screen_text::main::render_step_line::render_step_line;
use sqlbuild_core::screen_text::models::{
    PhaseState, Problem, ProblemLevel, Segment, StepLine, Tone,
};
use sqlbuild_core::text::main::cell_len::cell_len;
use sqlbuild_orchestration::dev_events::models::UnsetProviderSecrets;

use crate::dev_screen::constants::{
    SECRETS_CODE, SECRETS_MISSING, SECRETS_NODES_SHOWN, SECRETS_PLACE_MARGIN, SECRETS_SET,
    SECRETS_WIDE_COLUMNS,
};
use crate::dev_screen::models::DevScreen;

/// The live row while secrets are missing: each provider and the variables it misses; empty when none is.
pub(crate) fn secrets_rows(screen: &DevScreen) -> Vec<String> {
    if screen.missing_secrets.is_empty() {
        return Vec::new();
    }
    let providers: Vec<String> = screen
        .missing_secrets
        .iter()
        .map(|provider| format!("{} {}", provider.provider, variables(provider).join(", ")))
        .collect();
    let line: StepLine = StepLine {
        state: PhaseState::Warn,
        label: SECRETS_MISSING.to_owned(),
        elapsed: None,
        detail: providers.join(" \u{b7} "),
    };
    render_step_line(&line, &screen.facts, true)
}

/// Take the latest health read's providers missing secrets: the block when which are missing or the file changes, and one line once none is.
pub(crate) fn secrets_reported(
    mut screen: DevScreen,
    providers: Vec<UnsetProviderSecrets>,
    env_file: Option<String>,
) -> (DevScreen, Vec<String>) {
    let unchanged: bool =
        unset(&providers) == unset(&screen.missing_secrets) && env_file == screen.secrets_env_file;
    let had_missing: bool = !screen.missing_secrets.is_empty();
    screen.missing_secrets = providers;
    screen.secrets_env_file = env_file;
    if !screen.missing_secrets.is_empty() {
        let lines: Vec<String> = if unchanged {
            Vec::new()
        } else {
            render_problem(&secrets_problem(&screen), &screen.facts)
        };
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

fn variables(provider: &UnsetProviderSecrets) -> Vec<&str> {
    provider
        .settings
        .iter()
        .map(|setting| setting.variable.as_str())
        .collect()
}

/// Which variables which providers miss, which is what a new block is printed for.
fn unset(providers: &[UnsetProviderSecrets]) -> Vec<(&str, Vec<&str>)> {
    providers
        .iter()
        .map(|provider| (provider.provider.as_str(), variables(provider)))
        .collect()
}

/// The warning block: each field's file and line, why it matters with the nodes it stops, and the fix.
fn secrets_problem(screen: &DevScreen) -> Problem {
    let providers: &[UnsetProviderSecrets] = &screen.missing_secrets;
    let title: String = match providers.len() {
        1 => "1 provider has missing secrets".to_owned(),
        count => format!("{count} providers have missing secrets"),
    };
    let settings: usize = providers
        .iter()
        .map(|provider| provider.settings.len())
        .sum();
    Problem {
        level: ProblemLevel::Warning,
        title,
        code: SECRETS_CODE.to_owned(),
        at: places(providers, screen.facts.columns),
        why: reason(providers),
        fix: secrets_fix(settings, screen.secrets_env_file.as_deref()),
    }
}

/// `providers/payments.py:14  api_token  \u{2190} PAYMENTS_API_TOKEN` per setting, in columns, or on two rows where that doesn't fit.
fn places(providers: &[UnsetProviderSecrets], columns: usize) -> Vec<Segment> {
    let label: &str = if columns >= SECRETS_WIDE_COLUMNS {
        "field "
    } else {
        ""
    };
    let mut places: Vec<(String, String, &str)> = Vec::new();
    for provider in providers {
        for setting in &provider.settings {
            let file: String = match setting.line {
                Some(line) => format!("{}:{line}", provider.path),
                None => provider.path.clone(),
            };
            places.push((file, format!("{label}{}", setting.field), &setting.variable));
        }
    }
    let file_width: usize = places
        .iter()
        .map(|place| cell_len(&place.0))
        .max()
        .unwrap_or(0);
    let field_width: usize = places
        .iter()
        .map(|place| cell_len(&place.1))
        .max()
        .unwrap_or(0);
    let width: usize = columns.saturating_sub(SECRETS_PLACE_MARGIN);
    let mut rows: Vec<String> = Vec::new();
    for (file, field, variable) in places {
        let row: String =
            format!("{file:<file_width$}  {field:<field_width$}  \u{2190} {variable}");
        if cell_len(&row) <= width {
            rows.push(row);
        } else {
            rows.extend([file, format!("  {field} \u{2190} {variable}")]);
        }
    }
    rows.into_iter()
        .map(|text| Segment {
            text,
            tone: Tone::Plain,
            link: None,
        })
        .collect()
}

/// Why it matters, naming the nodes that need the providers, the first few when there are many.
fn reason(providers: &[UnsetProviderSecrets]) -> String {
    let them: &str = if providers.len() == 1 { "it" } else { "them" };
    let reason: String =
        format!("runs that use {them} stop at the provider check, before the warehouse");
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut names: Vec<&str> = Vec::new();
    for key in providers.iter().flat_map(|provider| &provider.needed_by) {
        if seen.insert(key) {
            names.push(key.split_once(':').map_or(key.as_str(), |(_, name)| name));
        }
    }
    let total: usize = names.len();
    if total == 0 {
        return reason;
    }
    names.truncate(SECRETS_NODES_SHOWN);
    let mut listed: String = names.join(", ");
    if total > names.len() {
        listed.push_str(&format!(" and {} more", total - names.len()));
    }
    let needing: String = if total == 1 {
        "1 node needs".to_owned()
    } else {
        format!("{total} nodes need")
    };
    format!("{reason}; {needing} {them} ({listed})")
}

/// `r` re-reads an edited env file; only a new `sqb dev` sees a changed environment.
fn secrets_fix(settings: usize, env_file: Option<&str>) -> String {
    let them: &str = if settings == 1 { "it" } else { "them" };
    let environment: String = format!(
        "set {them} in the environment you start sqb dev from, then quit and run sqb dev again"
    );
    match env_file {
        Some(file) => format!("set {them} in {file} and press r, or {environment}"),
        None => environment,
    }
}
