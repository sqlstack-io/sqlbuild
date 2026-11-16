//! The `sqb dev` screen: checklist, framed sign-in, ready card, event lines, plain mode and keys.

use sqlbuild_core::text::main::cell_len::cell_len;

use crate::dev_screen::constants::LOG_RING_LINES;
use crate::dev_screen::models::{ScreenAction, ScreenMessage};
use crate::dev_screen::tests::helpers::{
    after_startup, card, checklist, connected, edits, framed_sign_in, lines_of, log_rows,
    not_connected, play, shown_log, startup,
};
use crate::dev_screen::tests::test_types::{KeyTestCase, ScreenTestCase};

const HOTKEY_BAR: &str =
    "  o open UI \u{b7} r recompile \u{b7} s retry warehouse \u{b7} l logs \u{b7} q quit";

#[test]
fn given_a_session_when_each_message_is_applied_then_the_terminal_shows_stable_uncut_rows() {
    let edited: Vec<String> = [
        card("\u{25cf} duckdb \u{b7} main", "1.7s"),
        lines_of([
            "",
            "  09:41:17  ~ models/orders/int_orders.sql",
            "  09:41:19  \u{2713} gen 2 published                 1 file changed       0.1s",
            "  09:41:20  ~ models/finance/fct_revenue.sql",
            "  09:41:20  ~ 2 more files",
            "  09:41:22  \u{2717} gen 3 failed \u{b7} still serving gen 2",
            "            models/finance/fct_revenue.sql:42:7  unknown column net_amount_usd",
            "            r retry \u{b7} l full log",
            "  09:41:23  \u{26a0} warehouse sync stopped",
            "            5 failed reads in a row: ProgrammingError: insufficient privileges",
            "",
            HOTKEY_BAR,
        ]),
    ]
    .concat();
    let rule: String = "\u{2500}".repeat(78);
    let test_cases = [
        ScreenTestCase {
            description: "starting at 80 columns cuts the long control store path with an ellipsis",
            columns: Some(80),
            messages: startup(),
            expected_rows: checklist(
                "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuil\u{2026}",
                46,
            ),
        },
        ScreenTestCase {
            description: "starting at 120 columns shows the whole path",
            columns: Some(120),
            messages: startup(),
            expected_rows: checklist(
                "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuild/control.sqlite3",
                62,
            ),
        },
        ScreenTestCase {
            description: "at 80 columns the checklist collapses into the card and events print above the hotkeys",
            columns: Some(80),
            messages: after_startup([vec![connected()], edits()].concat()),
            expected_rows: edited.clone(),
        },
        ScreenTestCase {
            description: "at 120 columns event lines keep their columns",
            columns: Some(120),
            messages: after_startup([vec![connected()], edits()].concat()),
            expected_rows: edited,
        },
        ScreenTestCase {
            description: "an unavailable warehouse says not connected and how to retry",
            columns: Some(80),
            messages: after_startup(vec![not_connected()]),
            expected_rows: [
                card(
                    "\u{25cb} not connected \u{b7} sign-in skipped \u{b7} press s to retry",
                    "1.7s",
                ),
                lines_of(["", HOTKEY_BAR]),
            ]
            .concat(),
        },
        ScreenTestCase {
            description: "a client prompting for sign-in is framed verbatim and ticks never redraw over it",
            columns: Some(80),
            messages: after_startup(framed_sign_in()),
            expected_rows: [
                checklist(
                    "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuil\u{2026}",
                    46,
                )[..8]
                    .to_vec(),
                lines_of([
                    "  \u{280b} Warehouse            duckdb \u{b7} connecting                                0.1s",
                    "",
                ]),
                vec![format!(
                    "  \u{2500}\u{2500} warehouse {}",
                    "\u{2500}".repeat(65)
                )],
                lines_of([
                    "Initiating login request with your identity provider...",
                    "Enter the URL the OAuth flow redirected you to: ",
                ]),
                vec![format!("  {rule}"), String::new()],
                card("\u{25cf} duckdb \u{b7} main", "2.2s"),
                lines_of(["", HOTKEY_BAR]),
            ]
            .concat(),
        },
        ScreenTestCase {
            description: "plain output is stable unstyled lines that ignore ticks and keys",
            columns: None,
            messages: after_startup(
                [
                    vec![ScreenMessage::Tick, ScreenMessage::Key('q'), not_connected()],
                    edits(),
                ]
                .concat(),
            ),
            expected_rows: lines_of([
                "sqlbuild dev: control store ready (0.1s)",
                "sqlbuild dev: location 01a1d7ff-0b3a-7631-ba35-e44d44d072f2 \u{b7} target dev",
                "sqlbuild dev: discovered 412 python nodes, 2280 models (0.3s)",
                "sqlbuild dev: published gen 1 b429729b (0.1s)",
                "sqlbuild dev: warehouse not connected: sign-in skipped",
                "sqlbuild dev: ready http://127.0.0.1:8000",
                "sqlbuild dev: changed models/orders/int_orders.sql",
                "sqlbuild dev: published gen 2 c01d2e3f (0.1s)",
                "sqlbuild dev: changed models/finance/fct_revenue.sql",
                "sqlbuild dev: changed 2 more files",
                "sqlbuild dev: gen 3 failed \u{b7} still serving gen 2: models/finance/fct_revenue.sql:42:7 unknown column net_amount_usd",
                "sqlbuild dev: warehouse sync stopped: 5 failed reads in a row: ProgrammingError: insufficient privileges",
            ]),
        },
    ];

    for test_case in test_cases {
        let transcript = play(test_case.columns, test_case.messages);

        assert_eq!(
            transcript.rows, test_case.expected_rows,
            "{}",
            test_case.description
        );
        assert!(
            transcript
                .rows
                .iter()
                .all(|row| cell_len(row) <= test_case.columns.unwrap_or(usize::MAX)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_keys_after_ready_when_pressed_then_they_ask_for_actions_and_l_shows_the_newest_log_lines()
{
    let flood: Vec<ScreenMessage> = (0..=LOG_RING_LINES)
        .map(|index| ScreenMessage::Log(format!("log line {index}")))
        .collect();
    let test_cases = [
        KeyTestCase {
            description: "l opens the ring's newest lines, x does nothing, l closes, then o r s q act",
            columns: Some(80),
            messages: after_startup([vec![connected()], flood.clone()].concat()),
            keys: "lxlorsq",
            expected_actions: vec![
                ScreenAction::None,
                ScreenAction::None,
                ScreenAction::None,
                ScreenAction::OpenUi,
                ScreenAction::Recompile,
                ScreenAction::RetryWarehouse,
                ScreenAction::Quit,
            ],
            expected_log: log_rows(1, LOG_RING_LINES),
        },
        KeyTestCase {
            description: "plain output has no hotkeys",
            columns: None,
            messages: after_startup([vec![connected()], flood].concat()),
            keys: "lq",
            expected_actions: vec![ScreenAction::None, ScreenAction::None],
            expected_log: Vec::new(),
        },
    ];

    for test_case in test_cases {
        let keys: usize = test_case.keys.len();
        let transcript = play(
            test_case.columns,
            [
                test_case.messages,
                test_case.keys.chars().map(ScreenMessage::Key).collect(),
            ]
            .concat(),
        );

        assert_eq!(
            transcript.actions[transcript.actions.len() - keys..],
            test_case.expected_actions,
            "{}",
            test_case.description
        );
        assert_eq!(
            shown_log(&transcript.rows),
            test_case.expected_log,
            "{}",
            test_case.description
        );
    }
}
