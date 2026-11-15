//! The `sqb dev` screen: checklist, framed sign-in, ready card, event lines, plain mode and keys.

use std::time::Instant;

use sqlbuild_core::screen_text::models::WarehouseState;
use sqlbuild_core::text::main::cell_len::cell_len;
use sqlbuild_orchestration::dev_events::models::{
    DevEvent, DevPhase, DevStepState, SourcePosition,
};

use crate::dev_screen::constants::LOG_RING_LINES;
use crate::dev_screen::models::{ScreenAction, ScreenMessage};
use crate::dev_screen::tests::helpers::{connected, play, screen, startup, step};

fn edits() -> Vec<ScreenMessage> {
    vec![
        ScreenMessage::Child(DevEvent::Changed {
            paths: vec!["models/orders/int_orders.sql".to_owned()],
            more: 0,
        }),
        step(DevPhase::Discover, DevStepState::Started, None),
        ScreenMessage::Child(DevEvent::Published {
            generation: 2,
            snapshot_id: "c01d2e3f4a".to_owned(),
            record: serde_json::Value::Null,
        }),
        ScreenMessage::Child(DevEvent::Changed {
            paths: vec!["models/finance/fct_revenue.sql".to_owned()],
            more: 2,
        }),
        step(DevPhase::Discover, DevStepState::Started, None),
        ScreenMessage::Child(DevEvent::Failed {
            generation: 3,
            message: "unknown column net_amount_usd".to_owned(),
            position: Some(SourcePosition {
                path: "models/finance/fct_revenue.sql".to_owned(),
                line: 42,
                column: 7,
            }),
        }),
    ]
}

fn not_connected() -> ScreenMessage {
    ScreenMessage::Warehouse {
        adapter: "snowflake".to_owned(),
        state: WarehouseState::NotConnected {
            reason: "sign-in skipped".to_owned(),
        },
    }
}

fn card(warehouse: &str, ready_in: &str) -> Vec<String> {
    [
        format!("  sqlbuild dev  v0.232.0   ready in {ready_in}"),
        String::new(),
        "  \u{279c}  UI          http://127.0.0.1:8000".to_owned(),
        "  \u{279c}  Location    dev \u{b7} gen 1 \u{b7} 412 python nodes, 2280 models".to_owned(),
        format!("  \u{279c}  Warehouse   {warehouse}"),
        "  \u{279c}  Triggers    41 paused".to_owned(),
        "  \u{279c}  Watching    project/".to_owned(),
    ]
    .to_vec()
}

const HOTKEY_BAR: &str =
    "  o open UI \u{b7} r recompile \u{b7} s retry warehouse \u{b7} l logs \u{b7} q quit";

#[test]
fn given_a_live_screen_when_starting_then_the_checklist_redraws_in_place_and_cuts_rather_than_wraps() {
    let checklist = |control_store: &str, pad: usize| -> Vec<String> {
        let row = |glyph: &str, label: &str, detail: &str, time: &str| -> String {
            format!("  {glyph} {label:<20} {detail:<width$} {time:>8}", width = pad)
        };
        vec![
            "  sqlbuild dev  v0.232.0 \u{b7} shop \u{b7} target dev".to_owned(),
            String::new(),
            row("\u{2713}", "Control store", control_store, "0.1s"),
            row("\u{2713}", "API", "127.0.0.1:8000", "0.1s"),
            row("\u{2713}", "Scheduler", "41 paused", "0.1s"),
            row("\u{2713}", "Discover project", "", "0.1s"),
            row("\u{2713}", "Compile snapshot", "412 python nodes, 2280 models", "0.1s"),
            row("\u{2713}", "Publish", "gen 1 \u{b7} b429729b", "0.1s"),
            row("\u{280b}", "Warehouse", "duckdb \u{b7} connecting", "0.0s"),
        ]
    };
    for (columns, expected) in [
        (
            80,
            checklist("sqlite \u{b7} /home/analyst/projects/shop/.sqlbuil\u{2026}", 46),
        ),
        (
            120,
            checklist(
                "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuild/control.sqlite3",
                62,
            ),
        ),
    ] {
        let start: Instant = Instant::now();
        let mut live = screen(Some(columns), start);
        let transcript = play(&mut live, start, startup());

        assert_eq!(transcript.rows, expected, "{columns} columns");
        assert!(
            transcript.rows.iter().all(|row| cell_len(row) <= columns),
            "{columns} columns"
        );
    }
}

#[test]
fn given_a_live_screen_when_ready_and_edited_then_events_print_between_the_card_and_the_hotkey_bar()
 {
    let mut expected: Vec<String> = card("\u{25cf} duckdb \u{b7} main", "1.5s");
    expected.extend(
        [
            "",
            "  09:41:15  ~ models/orders/int_orders.sql",
            "  09:41:17  \u{2713} gen 2 published                 1 file changed       0.1s",
            "  09:41:18  ~ models/finance/fct_revenue.sql",
            "  09:41:18  ~ 2 more files",
            "  09:41:20  \u{2717} gen 3 failed \u{b7} still serving gen 2",
            "            models/finance/fct_revenue.sql:42:7  unknown column net_amount_usd",
            "            r retry \u{b7} l full log",
            "",
            HOTKEY_BAR,
        ]
        .map(str::to_owned),
    );
    for columns in [80, 120] {
        let start: Instant = Instant::now();
        let mut live = screen(Some(columns), start);
        let mut messages: Vec<ScreenMessage> = startup();
        messages.push(connected());
        messages.extend(edits());
        let transcript = play(&mut live, start, messages);

        assert_eq!(transcript.rows, expected, "{columns} columns");
    }
}

#[test]
fn given_no_warehouse_when_ready_then_the_card_says_not_connected_and_how_to_retry() {
    let start: Instant = Instant::now();
    let mut live = screen(Some(80), start);
    let mut messages: Vec<ScreenMessage> = startup();
    messages.push(not_connected());
    let transcript = play(&mut live, start, messages);

    let mut expected: Vec<String> = card(
        "\u{25cb} not connected \u{b7} sign-in skipped \u{b7} press s to retry",
        "1.5s",
    );
    expected.extend([String::new(), HOTKEY_BAR.to_owned()]);
    assert_eq!(transcript.rows, expected);
}

#[test]
fn given_a_client_prompting_for_sign_in_when_framed_then_its_output_is_verbatim_and_nothing_redraws_over_it()
 {
    let start: Instant = Instant::now();
    let mut live = screen(Some(80), start);
    let mut messages: Vec<ScreenMessage> = startup();
    messages.extend([
        ScreenMessage::ClientOutput(
            "Initiating login request with your identity provider...\n".to_owned(),
        ),
        ScreenMessage::Tick,
        ScreenMessage::ClientOutput("Enter the URL the OAuth flow redirected you to: ".to_owned()),
        ScreenMessage::Tick,
        ScreenMessage::Log("connection check started".to_owned()),
        connected(),
    ]);
    let transcript = play(&mut live, start, messages);

    assert_eq!(
        &transcript.writes[15..19],
        [
            "",
            "Enter the URL the OAuth flow redirected you to: ",
            "",
            ""
        ]
    );
    let rule: String = "\u{2500}".repeat(78);
    assert_eq!(
        &transcript.rows[8..14],
        [
            "  \u{280b} Warehouse            duckdb \u{b7} connecting                                0.1s".to_owned(),
            String::new(),
            format!("  {}", &format!("\u{2500}\u{2500} warehouse {rule}")[..]
                .chars()
                .take(78)
                .collect::<String>()),
            "Initiating login request with your identity provider...".to_owned(),
            "Enter the URL the OAuth flow redirected you to: ".to_owned(),
            format!("  {rule}"),
        ]
    );
    let mut expected: Vec<String> = vec![String::new()];
    expected.extend(card("\u{25cf} duckdb \u{b7} main", "2.0s"));
    expected.extend([String::new(), HOTKEY_BAR.to_owned()]);
    assert_eq!(&transcript.rows[14..], expected);
}

#[test]
fn given_plain_output_when_started_and_edited_then_lines_are_stable_unstyled_and_ignore_keys_and_ticks()
 {
    let start: Instant = Instant::now();
    let mut plain = screen(None, start);
    let mut messages: Vec<ScreenMessage> = startup();
    messages.extend([ScreenMessage::Tick, ScreenMessage::Key('q'), not_connected()]);
    messages.extend(edits());
    let transcript = play(&mut plain, start, messages);

    assert_eq!(
        transcript.rows,
        [
            "sqlbuild dev: location dev \u{b7} target dev",
            "sqlbuild dev: control store ready (0.1s)",
            "sqlbuild dev: discovered 412 python nodes, 2280 models (0.3s)",
            "sqlbuild dev: published gen 1 b429729b (0.1s)",
            "sqlbuild dev: warehouse not connected: sign-in skipped",
            "sqlbuild dev: ready http://127.0.0.1:8000",
            "sqlbuild dev: changed models/orders/int_orders.sql",
            "sqlbuild dev: published gen 2 c01d2e3f (0.1s)",
            "sqlbuild dev: changed models/finance/fct_revenue.sql",
            "sqlbuild dev: changed 2 more files",
            "sqlbuild dev: gen 3 failed \u{b7} still serving gen 2: models/finance/fct_revenue.sql:42:7 unknown column net_amount_usd",
        ]
    );
    assert!(transcript.actions.iter().all(|action| *action == ScreenAction::None));
}

#[test]
fn given_more_log_lines_than_the_ring_when_l_is_pressed_then_the_newest_lines_show_and_keys_map_to_actions()
 {
    let start: Instant = Instant::now();
    let mut live = screen(Some(80), start);
    let mut messages: Vec<ScreenMessage> = startup();
    messages.push(connected());
    messages.extend(
        (0..=LOG_RING_LINES).map(|index| ScreenMessage::Log(format!("log line {index}"))),
    );
    messages.extend("lxlorsq".chars().map(ScreenMessage::Key));
    let transcript = play(&mut live, start, messages);

    let logged: Vec<&String> = transcript
        .rows
        .iter()
        .filter(|row| row.starts_with("  log line "))
        .collect();
    assert_eq!(logged.len(), LOG_RING_LINES);
    assert_eq!(logged[0], "  log line 1");
    assert_eq!(logged[LOG_RING_LINES - 1], &format!("  log line {LOG_RING_LINES}"));
    assert_eq!(
        transcript.actions[transcript.actions.len() - 7..],
        [
            ScreenAction::None,
            ScreenAction::None,
            ScreenAction::None,
            ScreenAction::OpenUi,
            ScreenAction::Recompile,
            ScreenAction::RetryWarehouse,
            ScreenAction::Quit,
        ]
    );
}
