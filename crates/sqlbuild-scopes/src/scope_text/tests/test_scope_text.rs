use sqlbuild_core::cli_text::models::CliStyle;

use crate::scope_text::main::render_scope_text::render_scope_text;
use crate::scope_text::main::scope_result_failed::scope_result_failed;
use crate::scope_text::models::ScopeTextRequest;
use crate::scope_text::tests::helpers::{browse_result, list_result, orders_request};
use crate::scope_text::tests::test_types::ScopeTextTestCase;

#[test]
fn given_scope_result_when_rendering_text_then_text_and_exit_match() {
    let test_cases = [
        ScopeTextTestCase {
            description: "a browse quotes follow-up paths and reports partial results",
            result: browse_result(false),
            request: orders_request(),
            expected_text: "Scope folders\n  Path: models\n\n  \u{2514}\u{2500} my marts/  \
                1 declaration, 1 used, 0 children; macro 1\n     \
                sqb scope model:orders --browse 'sql/models/my marts'\n     \
                sqb scope model:orders --list 'sql/models/my marts'\n\nCompleteness: partial\n",
            expected_failed: true,
        },
        ScopeTextTestCase {
            description: "a list page offers the command that continues it",
            result: list_result(),
            request: ScopeTextRequest {
                page_size: 1,
                verbose: true,
                ..orders_request()
            },
            expected_text: "Scope declarations\n  Path: .\n  Showing: 0 of 2\n\n  (none)\n\n\
                Continue: sqb scope model:orders --list . --page-size 1 --after c1 --verbose\n\n\
                Diagnostics (0)\n  (none)\nCompleteness: complete\n",
            expected_failed: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (
                render_scope_text(
                    &test_case.result,
                    &test_case.request,
                    CliStyle { use_color: false }
                ),
                scope_result_failed(&test_case.result)
            ),
            (
                test_case.expected_text.to_owned(),
                test_case.expected_failed
            ),
            "{}",
            test_case.description
        );
    }
}
