use sqlbuild_core::text::main::cell_len::cell_len;

use crate::dev_screen::models::ScreenAction;
use crate::dev_screen::tests::helpers::{
    connecting, failed_compile, interrupted, interrupted_end, not_started_end, play, ready,
    removed_key, signing_in,
};
use crate::dev_screen::tests::test_types::{KeyTestCase, ScreenTestCase};

const SHORT: Option<(usize, usize)> = Some((80, 10));
const TALL: Option<(usize, usize)> = Some((80, 24));

#[test]
fn given_a_session_when_drawn_then_the_viewport_matches_the_snapshot() {
    let test_cases = [
        ScreenTestCase {
            description: "start-up at 80x24",
            size: TALL,
            messages: connecting(),
            end: None,
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "\u{2839} Connecting warehouse    0.1s   connecting",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "start-up at 80x10",
            size: SHORT,
            messages: connecting(),
            end: None,
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "\u{2839} Connecting warehouse    0.1s   connecting",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "a failed step with a long error at 80x24",
            size: TALL,
            messages: failed_compile(),
            end: None,
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2717} Discovering project     0.2s   1 error",
                "\u{2717} error  models/marts/fct_orders.sql                                       D001",
                "  at     models/marts/fct_orders.sql:42:7",
                "  why    column net_amount_usd does not exist in stg_payments; the column was",
                "         renamed to amount_usd_net when the payments source moved to the v2",
                "         export, and fct_orders still selects the old name",
                "  fix    select amount_usd_net instead, or alias it as net_amount_usd",
                "\u{b7} Discover project               waiting for a fix (r recompiles)",
                "\u{b7} Compile project                waiting",
                "\u{b7} Publish snapshot               waiting",
                "\u{b7} Connect warehouse              waiting",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "a failed step with a long error at 80x10",
            size: SHORT,
            messages: failed_compile(),
            end: None,
            expected_viewport: vec![
                "  why    column net_amount_usd does not exist in stg_payments; the column was",
                "         renamed to amount_usd_net when the payments source moved to the v2",
                "         export, and fct_orders still selects the old name",
                "  fix    select amount_usd_net instead, or alias it as net_amount_usd",
                "\u{b7} Discover project               waiting for a fix (r recompiles)",
                "\u{b7} Compile project                waiting",
                "\u{b7} Publish snapshot               waiting",
                "\u{b7} Connect warehouse              waiting",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "the sign-in prompt at 80x24",
            size: TALL,
            messages: signing_in(),
            end: None,
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "\u{25cc} sign in  snowflake needs you to sign in",
                "  Go to https://xy12345.snowflakecomputing.com/oauth/authorize?client_id=SQLBUI",
                "  LD&state=8f3a21c0&redirect_uri=http%3A%2F%2Flocalhost%3A8001 to sign in",
                "  Enter the URL the OAuth flow redirected you to: http://localhost:8001/?code=a",
                "  b",
                "  wait   0:00 \u{b7} Ctrl+C cancels and stops sqb dev",
                "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                "\u{2838} Connecting warehouse    0.5s   waiting for the sign-in above",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "the sign-in prompt at 80x10",
            size: SHORT,
            messages: signing_in(),
            end: None,
            expected_viewport: vec![
                "\u{25cc} sign in  snowflake needs you to sign in",
                "  Go to https://xy12345.snowflakecomputing.com/oauth/authorize?client_id=SQLBUI",
                "  LD&state=8f3a21c0&redirect_uri=http%3A%2F%2Flocalhost%3A8001 to sign in",
                "  Enter the URL the OAuth flow redirected you to: http://localhost:8001/?code=a",
                "  b",
                "  wait   0:00 \u{b7} Ctrl+C cancels and stops sqb dev",
                "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                "\u{2838} Connecting warehouse    0.5s   waiting for the sign-in above",
                "keys  l full log \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "ready at 80x24",
            size: TALL,
            messages: ready(),
            end: None,
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "\u{2713} Connected warehouse     0.6s   snowflake \u{b7} browser sign-in (OAuth)",
                "\u{256d}\u{2500} ready \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
                "\u{2502} UI         http://127.0.0.1:8640                                           \u{2502}",
                "\u{2502} Project    shop \u{b7} /home/analyst/projects/shop \u{b7} target dev                 \u{2502}",
                "\u{2502} Warehouse  snowflake \u{b7} account xy12345 \u{b7} auth browser sign-in (OAuth)      \u{2502}",
                "\u{2502}            role TRANSFORMER \u{b7} warehouse TRANSFORM_XS                       \u{2502}",
                "\u{2502} Triggers   41 paused                                                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
                "keys  o open UI \u{b7} l full log \u{b7} r recompile \u{b7} q quit",
            ],
        },
        ScreenTestCase {
            description: "Ctrl+C at 80x24",
            size: TALL,
            messages: interrupted(),
            end: Some(interrupted_end()),
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "\u{2713} Connected warehouse     0.6s   snowflake \u{b7} browser sign-in (OAuth)",
                "\u{256d}\u{2500} ready \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
                "\u{2502} UI         http://127.0.0.1:8640                                           \u{2502}",
                "\u{2502} Project    shop \u{b7} /home/analyst/projects/shop \u{b7} target dev                 \u{2502}",
                "\u{2502} Warehouse  snowflake \u{b7} account xy12345 \u{b7} auth browser sign-in (OAuth)      \u{2502}",
                "\u{2502}            role TRANSFORMER \u{b7} warehouse TRANSFORM_XS                       \u{2502}",
                "\u{2502} Triggers   41 paused                                                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
                "\u{b7} Stopped code location   0.3s",
                "\u{b7} Stopped scheduler       0.4s",
                "\u{b7} Stopped UI server       0.5s",
                "\u{b7} sqb dev stopped (interrupted)  ran 42m 10s   1.1s",
            ],
        },
        ScreenTestCase {
            description: "Ctrl+C at 80x10",
            size: SHORT,
            messages: interrupted(),
            end: Some(interrupted_end()),
            expected_viewport: vec![
                "\u{2502} Project    shop \u{b7} /home/analyst/projects/shop \u{b7} target dev                 \u{2502}",
                "\u{2502} Warehouse  snowflake \u{b7} account xy12345 \u{b7} auth browser sign-in (OAuth)      \u{2502}",
                "\u{2502}            role TRANSFORMER \u{b7} warehouse TRANSFORM_XS                       \u{2502}",
                "\u{2502} Triggers   41 paused                                                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
                "\u{b7} Stopped code location   0.3s",
                "\u{b7} Stopped scheduler       0.4s",
                "\u{b7} Stopped UI server       0.5s",
                "\u{b7} sqb dev stopped (interrupted)  ran 42m 10s   1.1s",
            ],
        },
        ScreenTestCase {
            description: "a configuration error at 80x10",
            size: SHORT,
            messages: removed_key(),
            end: Some(not_started_end()),
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2717} Read configuration      0.0s   1 error",
                "\u{2717} error  sqlbuild_project.toml                                             D001",
                "  why    [dbt] option(s) were removed: defer_clone_from; use dbt-native",
                "         --state/--defer",
                "  fix    remove defer_clone_from from [dbt]",
                "\u{2717} sqb dev did not start  configuration error in sqlbuild_project.toml   0.1s",
                "Next",
                "  sqb debug                     check the configuration on its own",
            ],
        },
        ScreenTestCase {
            description: "--plain appends finished lines only",
            size: None,
            messages: interrupted(),
            end: Some(interrupted_end()),
            expected_viewport: vec![
                "sqb dev  shop \u{b7} target dev \u{b7} sqb 0.256.0",
                "\u{2713} Opened control store    0.1s   sqlite",
                "\u{2713} Started UI server       0.1s   http://127.0.0.1:8640",
                "\u{2713} Started scheduler       0.1s   triggers start paused",
                "\u{2713} Discovered project      0.2s   118 models \u{b7} 24 seeds \u{b7} 2 Python nodes",
                "\u{2713} Compiled project        0.1s   0 errors \u{b7} 1 warning",
                "\u{2713} Published snapshot      0.1s   snapshot b429729b",
                "Go to https://xy12345.snowflakecomputing.com/oauth/authorize?client_id=SQLBUILD&state=8f3a21c0&redirect_uri=http%3A%2F%2Flocalhost%3A8001 to sign in",
                "Enter the URL the OAuth flow redirected you to: http://localhost:8001/?code=ab",
                "\u{2713} Connected warehouse     0.6s   snowflake \u{b7} browser sign-in (OAuth)",
                "\u{2713} ready  http://127.0.0.1:8640",
                "  warehouse  snowflake \u{b7} account xy12345 \u{b7} auth browser sign-in (OAuth)",
                "             role TRANSFORMER \u{b7} warehouse TRANSFORM_XS",
                "  triggers   41 paused",
                "\u{b7} Stopped code location   0.3s",
                "\u{b7} Stopped scheduler       0.4s",
                "\u{b7} Stopped UI server       0.5s",
                "\u{b7} sqb dev stopped (interrupted)  ran 42m 10s   1.1s",
            ],
        },
    ];

    for test_case in test_cases {
        let transcript = play(test_case.size, test_case.messages, test_case.end);

        assert_eq!(
            transcript.viewport, test_case.expected_viewport,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_session_when_drawn_then_the_header_is_printed_once_and_no_row_overflows() {
    let sessions = [connecting(), failed_compile(), signing_in(), ready(), interrupted()];
    let mut problems: Vec<String> = Vec::new();
    for size in [SHORT, TALL, Some((40, 6)), Some((120, 30))] {
        for messages in sessions.clone() {
            let transcript = play(size, messages, Some(interrupted_end()));
            let (columns, rows) = size.unwrap_or((80, 24));
            let headers: usize = transcript
                .scrollback
                .iter()
                .filter(|line| line.starts_with("sqb dev  shop"))
                .count();
            if headers != 1 {
                problems.push(format!("{columns}x{rows}: header printed {headers} times"));
            }
            if transcript.viewport.len() > rows {
                problems.push(format!("{columns}x{rows}: {} rows shown", transcript.viewport.len()));
            }
            for line in &transcript.scrollback {
                if cell_len(line) >= columns && columns >= 60 {
                    problems.push(format!("{columns}x{rows}: too wide: {line}"));
                }
            }
        }
    }

    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn given_keys_after_ready_when_pressed_then_they_ask_for_actions() {
    let test_cases = [KeyTestCase {
        description: "o, r and s act; l toggles the log; other keys do nothing",
        keys: "olrsx",
        expected_actions: vec![
            ScreenAction::OpenUi,
            ScreenAction::None,
            ScreenAction::Recompile,
            ScreenAction::RetryWarehouse,
            ScreenAction::None,
        ],
    }];

    for test_case in test_cases {
        let keys: usize = test_case.keys.len();
        let transcript = play(
            TALL,
            [
                ready(),
                test_case
                    .keys
                    .chars()
                    .map(crate::dev_screen::models::ScreenMessage::Key)
                    .collect(),
            ]
            .concat(),
            None,
        );

        assert_eq!(
            transcript.actions[transcript.actions.len() - keys..],
            test_case.expected_actions,
            "{}",
            test_case.description
        );
    }
}
