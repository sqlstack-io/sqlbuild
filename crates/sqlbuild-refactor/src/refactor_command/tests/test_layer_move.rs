use sqlbuild_core::cli_text::models::CliStyle;

use crate::refactor_command::main::layer_move_suggestion::layer_move_suggestion;
use crate::refactor_command::main::render_refactor_text::render_refactor_text;
use crate::refactor_command::models::{
    BaselineComparison, LayerMove, RefactorDiagnostic, RefactorStatus,
};
use crate::refactor_command::tests::helpers::{rename_plan, rule_diagnostics};
use crate::refactor_command::tests::test_types::{LayerMoveTestCase, RenderTestCase};
use crate::refactoring::models::RefactorPlan;

const LAYER_CODES: &[&str] = &["SQBRPROJECT102", "SQBRPROJECT105"];
const ENRICHED_COMMAND: &str = "sqb mv orders__int_clean__lines__web \
    sql/models/orders/intermediate/enriched/web/orders__int_enriched__lines__web.sql";

#[test]
fn given_rename_compile_errors_when_suggesting_layer_move_then_one_step_mv_is_exact() {
    let test_cases = [
        LayerMoveTestCase {
            description: "int_clean to int_enriched keeps domain and sub-folders",
            old_path: "sql/models/orders/intermediate/clean/web/orders__int_clean__lines__web.sql",
            new_name: "orders__int_enriched__lines__web",
            codes: LAYER_CODES,
            project_dir: None,
            windows: false,
            expected_command: Some(ENRICHED_COMMAND),
            expected_folder: Some("sql/models/orders/intermediate/enriched"),
        },
        LayerMoveTestCase {
            description: "a bare intermediate folder becomes the int_clean folder",
            old_path: "sql/models/orders/intermediate/orders__int_v__totals.sql",
            new_name: "orders__int_clean__totals",
            codes: &["SQBRPROJECT102"],
            project_dir: None,
            windows: false,
            expected_command: Some(
                "sqb mv orders__int_v__totals \
                 sql/models/orders/intermediate/clean/orders__int_clean__totals.sql",
            ),
            expected_folder: Some("sql/models/orders/intermediate/clean"),
        },
        LayerMoveTestCase {
            description: "only the current layer's own folder is replaced",
            old_path: "sql/models/mart/orders/staging/orders__stg__order_lines_x.sql",
            new_name: "orders__int_clean__order_lines_x",
            codes: &["SQBRPROJECT102"],
            project_dir: None,
            windows: false,
            expected_command: Some(
                "sqb mv orders__stg__order_lines_x \
                 sql/models/mart/orders/intermediate/clean/orders__int_clean__order_lines_x.sql",
            ),
            expected_folder: Some("sql/models/mart/orders/intermediate/clean"),
        },
        LayerMoveTestCase {
            description: "no layer folder in the current path uses a placeholder",
            old_path: "sql/models/orders/web/order_lines.sql",
            new_name: "orders__stg__order_lines",
            codes: LAYER_CODES,
            project_dir: None,
            windows: false,
            expected_command: Some(
                "sqb mv order_lines 'sql/models/<folder>/orders__stg__order_lines.sql'",
            ),
            expected_folder: None,
        },
        LayerMoveTestCase {
            description: "a layer without a folder mapping uses a placeholder",
            old_path: "sql/models/orders/intermediate/clean/orders__int_clean__totals.sql",
            new_name: "orders__int_archive__totals",
            codes: LAYER_CODES,
            project_dir: None,
            windows: false,
            expected_command: Some(
                "sqb mv orders__int_clean__totals \
                 'sql/models/<folder>/orders__int_archive__totals.sql'",
            ),
            expected_folder: None,
        },
        LayerMoveTestCase {
            description: "another rule failing as well gets no suggestion",
            old_path: "sql/models/orders/intermediate/clean/orders__int_clean__totals.sql",
            new_name: "orders__int_enriched__totals",
            codes: &["SQBRPROJECT105", "SQBRMODEL103"],
            project_dir: None,
            windows: false,
            expected_command: None,
            expected_folder: None,
        },
        LayerMoveTestCase {
            description: "posix shells get POSIX quoting",
            old_path: "sql/models/orders/intermediate/clean/web/orders__int_clean__lines__web.sql",
            new_name: "orders__int_enriched__lines__web",
            codes: LAYER_CODES,
            project_dir: Some("/srv/order projects/orders"),
            windows: false,
            expected_command: Some(
                "sqb mv orders__int_clean__lines__web \
                 sql/models/orders/intermediate/enriched/web/orders__int_enriched__lines__web.sql \
                 --project-dir '/srv/order projects/orders'",
            ),
            expected_folder: Some("sql/models/orders/intermediate/enriched"),
        },
        LayerMoveTestCase {
            description: "windows gets cmd.exe quoting",
            old_path: "sql/models/orders/intermediate/clean/web/orders__int_clean__lines__web.sql",
            new_name: "orders__int_enriched__lines__web",
            codes: LAYER_CODES,
            project_dir: Some("C:\\order projects\\orders"),
            windows: true,
            expected_command: Some(
                "sqb mv orders__int_clean__lines__web \
                 sql/models/orders/intermediate/enriched/web/orders__int_enriched__lines__web.sql \
                 --project-dir \"C:\\order projects\\orders\"",
            ),
            expected_folder: Some("sql/models/orders/intermediate/enriched"),
        },
    ];

    for test_case in test_cases {
        let plan: RefactorPlan = rename_plan(test_case.old_path, test_case.new_name);
        let diagnostics: Vec<RefactorDiagnostic> =
            rule_diagnostics(test_case.codes, &plan.changes[0].path);
        let suggestion: Option<LayerMove> = layer_move_suggestion(
            &plan,
            &diagnostics,
            (test_case.project_dir, test_case.windows),
        );
        assert_eq!(
            (
                suggestion.as_ref().map(|item| item.command.as_str()),
                suggestion.as_ref().and_then(|item| item.folder.as_deref()),
            ),
            (test_case.expected_command, test_case.expected_folder),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_layer_failure_when_rendering_text_then_suggestion_follows_errors() {
    let test_cases = [RenderTestCase {
        description: "a refused rename lists errors, the one-step move and the status",
        old_path: "sql/models/orders/staging/orders__stg__lines.sql",
        new_name: "orders__mart__lines",
        codes: &["SQBRPROJECT105"],
        expected_text: "Rename model  orders__stg__lines -> orders__mart__lines\n\
            \u{2514}\u{2500}\u{2500} sql/models/orders/staging/orders__mart__lines.sql  \
            (moved from sql/models/orders/staging/orders__stg__lines.sql)\n\
            Compile errors in the edited project\n\
            \u{2514}\u{2500}\u{2500} SQBRPROJECT105  \
            sql/models/orders/staging/orders__mart__lines.sql:1  SQBRPROJECT105 failed\n\
            orders__mart__lines belongs under sql/models/orders/mart/\n  \
            Rename and move in one step:\n    \
            sqb mv orders__stg__lines sql/models/orders/mart/orders__mart__lines.sql\n  \
            (change the folder if you want it somewhere else)\n\
            Compiled: failed; no files changed\n",
    }];

    for test_case in test_cases {
        let plan: RefactorPlan = rename_plan(test_case.old_path, test_case.new_name);
        let diagnostics: Vec<RefactorDiagnostic> =
            rule_diagnostics(test_case.codes, &plan.changes[0].path);
        let suggestion: Option<LayerMove> =
            layer_move_suggestion(&plan, &diagnostics, (None, false));
        assert_eq!(
            render_refactor_text(
                &plan,
                (
                    RefactorStatus::CompileFailed,
                    &diagnostics,
                    &BaselineComparison::default(),
                ),
                suggestion.as_ref(),
                CliStyle { use_color: false },
            ),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
