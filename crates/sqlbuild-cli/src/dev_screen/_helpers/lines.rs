//! The lines the `sqb dev` screen prints for good, and the redraw that places them above the live rows.

use std::time::Instant;

use sqlbuild_core::screen_text::main::redraw_rows::redraw_rows;
use sqlbuild_core::screen_text::main::render_dev_header::render_dev_header;
use sqlbuild_core::screen_text::main::render_labelled_parts::render_labelled_parts;
use sqlbuild_core::screen_text::main::render_ready_box::render_ready_box;
use sqlbuild_core::screen_text::main::render_step_line::render_step_line;
use sqlbuild_core::screen_text::main::visible_cells::visible_cells;
use sqlbuild_core::screen_text::models::{BoxRow, StepLine, WarehouseState};

use crate::dev_screen::_helpers::rows::live_rows;
use crate::dev_screen::models::DevScreen;

/// A step line printed for good, its detail wrapped in full.
pub(crate) fn lasting_step(screen: &DevScreen, line: &StepLine) -> Vec<String> {
    render_step_line(line, &screen.facts, true)
}

/// The Warehouse line's parts: the adapter, then its labelled settings, or why it is not connected.
pub(crate) fn warehouse_parts(adapter: &str, state: &WarehouseState) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if !adapter.is_empty() {
        parts.push(adapter.to_owned());
    }
    match state {
        WarehouseState::Connected { facts } => parts.extend(facts.iter().cloned()),
        WarehouseState::NotConnected { reason } => {
            parts.push(format!("not connected: {reason}"));
        }
        WarehouseState::Failed { message } => parts.push(format!("failed: {message}")),
    }
    parts
}

/// The ready box, or plain `✓ ready` lines, once the first snapshot serves and the warehouse settled.
pub(crate) fn ready_lines(mut screen: DevScreen) -> (DevScreen, Vec<String>) {
    let (Some(_), Some(warehouse), false) =
        (&screen.serving, screen.warehouse.clone(), screen.ready)
    else {
        return (screen, Vec::new());
    };
    screen.ready = true;
    let parts: Vec<String> = warehouse_parts(&screen.adapter, &warehouse);
    if !screen.live {
        let mut lines: Vec<String> = vec![format!("\u{2713} ready  {}", screen.ui_url)];
        if !screen.exposure.is_empty() {
            lines.extend(render_labelled_parts(
                "access   ",
                std::slice::from_ref(&screen.exposure),
                screen.facts.columns,
            ));
        }
        lines.extend(render_labelled_parts(
            "warehouse",
            &parts,
            screen.facts.columns,
        ));
        if !screen.location.is_empty() {
            lines.extend(render_labelled_parts(
                "location ",
                std::slice::from_ref(&screen.location),
                screen.facts.columns,
            ));
        }
        if !screen.triggers.is_empty() {
            lines.extend(render_labelled_parts(
                "triggers ",
                std::slice::from_ref(&screen.triggers),
                screen.facts.columns,
            ));
        }
        if let Some(env_file) = &screen.env_file {
            lines.extend(render_labelled_parts(
                "env file ",
                std::slice::from_ref(env_file),
                screen.facts.columns,
            ));
        }
        return (screen, lines);
    }
    let mut project: Vec<String> = vec![
        screen.header.project.clone(),
        home_relative(&screen.project_root),
    ];
    if !screen.header.target.is_empty() {
        project.push(format!("target {}", screen.header.target));
    }
    let mut rows: Vec<BoxRow> = vec![
        BoxRow {
            label: "UI".to_owned(),
            parts: vec![screen.ui_url.clone()],
            link: Some(screen.ui_url.clone()),
        },
        BoxRow {
            label: "Project".to_owned(),
            parts: project,
            link: None,
        },
        BoxRow {
            label: "Warehouse".to_owned(),
            parts,
            link: None,
        },
    ];
    if !screen.exposure.is_empty() {
        let access: BoxRow = BoxRow {
            label: "Access".to_owned(),
            parts: vec![screen.exposure.clone()],
            link: None,
        };
        rows.insert(1, access);
    }
    if !screen.triggers.is_empty() {
        rows.push(BoxRow {
            label: "Triggers".to_owned(),
            parts: vec![screen.triggers.clone()],
            link: None,
        });
    }
    if let Some(env_file) = &screen.env_file {
        rows.push(BoxRow {
            label: "Env file".to_owned(),
            parts: vec![env_file.clone()],
            link: None,
        });
    }
    let lines: Vec<String> = render_ready_box(&rows, &screen.facts);
    (screen, lines)
}

/// `path` with the home directory written as `~`.
fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_owned(),
    }
}

/// The header once, then `lines` printed for good, then the live rows redrawn below them.
pub(crate) fn compose(
    mut screen: DevScreen,
    lines: Vec<String>,
    now: Instant,
) -> (DevScreen, String) {
    let mut lasting: Vec<String> = Vec::new();
    if !screen.header_printed {
        screen.header_printed = true;
        lasting.extend(render_dev_header(&screen.header, &screen.facts));
    }
    lasting.extend(lines);
    if !screen.live {
        let mut text: String = String::new();
        if screen.client_partial && !lasting.is_empty() {
            screen.client_partial = false;
            text.push('\n');
        }
        for line in lasting {
            text.push_str(&line);
            text.push('\n');
        }
        return (screen, text);
    }
    let live: Vec<String> = live_rows(&screen, now);
    if lasting.is_empty() && live == screen.drawn {
        return (screen, String::new());
    }
    let previous: usize = physical_rows(&screen.drawn, screen.facts.columns);
    lasting.extend(live.iter().cloned());
    let text: String = redraw_rows(previous, &lasting);
    screen.drawn = live;
    (screen, text)
}

/// The terminal rows `rows` cover at `columns`, after a resize may have wrapped them.
pub(crate) fn physical_rows(rows: &[String], columns: usize) -> usize {
    let columns: usize = columns.max(1);
    rows.iter()
        .map(|row| visible_cells(row).div_ceil(columns).max(1))
        .sum()
}
