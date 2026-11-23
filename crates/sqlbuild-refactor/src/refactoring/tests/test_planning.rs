use crate::refactoring::_helpers::planning::model_planning::decide_model_migration;
use crate::refactoring::errors::{RefactorError, RefactorErrorKind};
use crate::refactoring::main::with_model_migration::with_model_migration;
use crate::refactoring::models::{DeclarationPlacement, RefactorOperation, RefactorPlan};
use crate::refactoring::tests::helpers::{
    blocking_reasons, change, commit_failure, facts, files_on_disk, followed_test_changes,
    followed_unit_tests, model, model_in_database, move_request, owned_pairs, placed,
    planned_batch, planned_declaration_moves, refused_target, relocated, replace_all, request,
    write_files,
};
use crate::refactoring::tests::test_types::{
    BatchPlanTestCase, CollisionTestCase, CommitTestCase, DeclarationMovesTestCase,
    FollowedUnitTestTestCase, MigrationDecisionTestCase, PendingMigrationTestCase,
    RefusedTargetTestCase,
};

#[test]
fn given_taken_name_or_destination_when_planning_then_collisions_block() -> Result<(), RefactorError>
{
    let project = tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
    write_files(project.path(), &[("sql/models/finance/orders.sql", "SELECT 1")])?;
    let project_facts = facts(
        &project.path().to_string_lossy(),
        vec![
            model("orders", Some("table"), "analytics"),
            model("Order_Lines", Some("table"), "analytics"),
        ],
    );
    let test_cases = [
        CollisionTestCase {
            description: "a rename onto another model's name, ignoring case",
            request: request(RefactorOperation::RenameModel, "order_lines", None),
            expected_reasons: &["sql/models/Order_Lines.sql: model:order_lines already exists"],
        },
        CollisionTestCase {
            description: "a move onto an existing file",
            request: request(RefactorOperation::MoveModel, "", Some("sql/models/finance/")),
            expected_reasons: &["sql/models/finance/orders.sql: destination file already exists"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            blocking_reasons(&project_facts, &test_case.request)?,
            test_case.expected_reasons,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_destination_outside_project_when_moving_then_request_is_refused() {
    let project_facts = facts(
        "/nonexistent/project",
        vec![model("orders", Some("table"), "analytics")],
    );
    let test_cases = [RefusedTargetTestCase {
        description: "a destination above the project directory",
        request: request(
            RefactorOperation::MoveModel,
            "",
            Some("../elsewhere/orders.sql"),
        ),
        expected_error: (
            "C955",
            "destination '../elsewhere/orders.sql' is outside the project",
        ),
    }];
    for test_case in test_cases {
        assert_eq!(
            refused_target(&project_facts, &test_case.request),
            Some((
                test_case.expected_error.0.to_owned(),
                test_case.expected_error.1.to_owned()
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relation_change_when_deciding_migration_then_python_decision_is_made() {
    let before = vec![
        model("orders", Some("table"), "analytics"),
        model("customers", Some("view"), "analytics"),
    ];
    let test_cases = [
        MigrationDecisionTestCase {
            description: "a renamed table keeps its history",
            before: before.clone(),
            after: Some(vec![
                model("order_lines", Some("table"), "analytics"),
                model("customers", Some("view"), "analytics"),
            ]),
            expected_decision: (
                true,
                false,
                "keeps the relation's history and its old name working",
            ),
        },
        MigrationDecisionTestCase {
            description: "an unknown materialization keeps no data",
            before: vec![model("orders", None, "analytics")],
            after: None,
            expected_decision: (false, false, "'None' models keep no warehouse data"),
        },
        MigrationDecisionTestCase {
            description: "an edited project that does not compile still gets migrate_from",
            before: before.clone(),
            after: None,
            expected_decision: (
                true,
                false,
                "keeps the relation's history; the destination could not be checked because the edited project does not compile",
            ),
        },
        MigrationDecisionTestCase {
            description: "a move across databases is blocked",
            before: before.clone(),
            after: Some(vec![model_in_database("order_lines", "archive")]),
            expected_decision: (
                false,
                true,
                "moves from database warehouse to archive; migrations cannot cross databases",
            ),
        },
        MigrationDecisionTestCase {
            description: "an emptied schema is blocked",
            before: before.clone(),
            after: Some(vec![model("order_lines", Some("table"), "marts")]),
            expected_decision: (
                false,
                true,
                "no model is left in schema analytics, so migrate_from orders cannot find the old relation; add a schema-qualified migrate_from for each target",
            ),
        },
    ];
    for test_case in test_cases {
        let decision = decide_model_migration(
            &test_case.before,
            test_case.after.as_deref(),
            ("orders", "order_lines"),
        );
        assert_eq!(
            (decision.needed, decision.blocked, decision.reason.as_str()),
            test_case.expected_decision,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_failing_commit_when_committing_then_project_files_are_unchanged()
-> Result<(), RefactorError> {
    let test_cases = [
        CommitTestCase {
            description: "a write failing after an earlier write restores every file",
            on_disk: &[
                ("sql/models/orders.sql", "SELECT 1"),
                ("sql/models/customers.sql", "SELECT 2"),
            ],
            originals: &[
                ("sql/models/orders.sql", "SELECT 1"),
                ("sql/models/customers.sql", "SELECT 2"),
            ],
            changes: vec![
                change(
                    "sql/models/orders.sql",
                    "sql/models/orders.sql",
                    vec![replace_all("SELECT 1", "SELECT 10")],
                ),
                change(
                    "sql/models/orders.sql/customers.sql",
                    "sql/models/customers.sql",
                    Vec::new(),
                ),
            ],
            expected_error: (RefactorErrorKind::Io, "", ""),
            expected_files: &[
                ("sql/models/orders.sql", "SELECT 1"),
                ("sql/models/customers.sql", "SELECT 2"),
            ],
        },
        CommitTestCase {
            description: "a file edited since planning writes nothing",
            on_disk: &[("sql/models/orders.sql", "SELECT 3")],
            originals: &[("sql/models/orders.sql", "SELECT 1")],
            changes: vec![change(
                "sql/models/orders.sql",
                "sql/models/orders.sql",
                vec![replace_all("SELECT 1", "SELECT 10")],
            )],
            expected_error: (
                RefactorErrorKind::Write,
                "C958",
                "files changed while the refactoring ran: sql/models/orders.sql",
            ),
            expected_files: &[("sql/models/orders.sql", "SELECT 3")],
        },
    ];
    for test_case in test_cases {
        let project =
            tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
        write_files(project.path(), test_case.on_disk)?;
        let failure = commit_failure(
            project.path(),
            test_case.originals,
            &test_case.changes,
            test_case.expected_error.2,
        );
        assert_eq!(
            failure,
            Some((
                test_case.expected_error.0,
                test_case.expected_error.1.to_owned(),
                true
            )),
            "{}",
            test_case.description
        );
        assert_eq!(
            files_on_disk(project.path(), test_case.on_disk),
            owned_pairs(test_case.expected_files),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_scope_placement_when_moving_across_folders_then_declaration_files_follow_or_block()
-> Result<(), RefactorError> {
    let test_cases = [
        DeclarationMovesTestCase {
            description: "a move within one folder asks no placement",
            on_disk: &[],
            moves: &[("sql/models/orders.sql", "sql/models/orders_v2.sql")],
            placement: DeclarationPlacement::default(),
            expected_moves: &[],
            expected_blocking: &[],
        },
        DeclarationMovesTestCase {
            description: "an unsettled placement blocks with the preview's reasons",
            on_disk: &[],
            moves: &[("sql/models/orders.sql", "sql/models/marts/orders.sql")],
            placement: DeclarationPlacement {
                relocated: None,
                diagnostics: vec!["Move source model:orders is unknown".to_owned()],
                declarations: Vec::new(),
            },
            expected_moves: &[],
            expected_blocking: &["sql/models/marts/orders.sql:0 Move source model:orders is unknown"],
        },
        DeclarationMovesTestCase {
            description: "an unsettled placement without reasons blocks with the default reason",
            on_disk: &[],
            moves: &[("sql/models/orders.sql", "sql/models/marts/orders.sql")],
            placement: DeclarationPlacement::default(),
            expected_moves: &[],
            expected_blocking: &[
                "sql/models/marts/orders.sql:0 declaration placement at the destination could not be worked out",
            ],
        },
        DeclarationMovesTestCase {
            description: "files move whole; split files, shared files, generated files and taken paths block",
            on_disk: &[
                ("sql/models/_sqlbuild/_macros/cents.py", "x"),
                ("sql/models/_sqlbuild/_enums/both.sql", "x"),
                ("sql/models/marts/_sqlbuild/_enums/both.sql", "x"),
            ],
            moves: &[("sql/models/orders.sql", "sql/models/marts/orders.sql")],
            placement: DeclarationPlacement {
                relocated: Some(vec![
                    relocated("enum:a", "sql/models/marts/_sqlbuild/_enums/both.sql"),
                    relocated("enum:b", "sql/models/other/_sqlbuild/_enums/both.sql"),
                    relocated("macro:cents", "sql/models/marts/_sqlbuild/_macros/cents.py"),
                    relocated("macro:generated", "sql/models/marts/_sqlbuild/_macros/gen.py"),
                ]),
                diagnostics: Vec::new(),
                declarations: vec![
                    placed("enum:a", "sql/models/_sqlbuild/_enums/both.sql"),
                    placed("enum:b", "sql/models/_sqlbuild/_enums/both.sql"),
                    placed("macro:cents", "sql/models/_sqlbuild/_macros/cents.py"),
                    placed("macro:stays", "sql/models/_sqlbuild/_macros/cents.py"),
                    placed("macro:generated", "sql/models/_sqlbuild/_macros/gen.py"),
                ],
            },
            expected_moves: &[
                (
                    "sql/models/_sqlbuild/_enums/both.sql",
                    "sql/models/marts/_sqlbuild/_enums/both.sql",
                ),
                (
                    "sql/models/_sqlbuild/_macros/cents.py",
                    "sql/models/marts/_sqlbuild/_macros/cents.py",
                ),
            ],
            expected_blocking: &[
                "sql/models/_sqlbuild/_enums/both.sql:1 sql/models/_sqlbuild/_enums/both.sql holds declarations that must move to different folders (sql/models/marts/_sqlbuild/_enums/both.sql, sql/models/other/_sqlbuild/_enums/both.sql); split the file first",
                "sql/models/_sqlbuild/_macros/gen.py:1 macro:generated must move to sql/models/marts/_sqlbuild/_macros/gen.py, but it is not an authored project file",
                "sql/models/_sqlbuild/_macros/cents.py:1 sql/models/marts/_sqlbuild/_macros/cents.py would take macro:stays along, which must stay in sql/models/_sqlbuild/_macros/cents.py; split the file first",
                "sql/models/marts/_sqlbuild/_enums/both.sql:0 a declaration must move here, but the file already exists",
            ],
        },
        DeclarationMovesTestCase {
            description: "two grouped files flattened into one direct file block",
            on_disk: &[
                ("sql/models/_sqlbuild/_macros/money/helpers.sql", "x"),
                ("sql/models/_sqlbuild/_macros/text/helpers.sql", "x"),
            ],
            moves: &[("sql/models/orders.sql", "sql/models/marts/orders.sql")],
            placement: DeclarationPlacement {
                relocated: Some(vec![
                    relocated("macro:cents", "sql/macros/helpers.sql"),
                    relocated("macro:label", "sql/macros/helpers.sql"),
                ]),
                diagnostics: Vec::new(),
                declarations: vec![
                    placed("macro:cents", "sql/models/_sqlbuild/_macros/money/helpers.sql"),
                    placed("macro:label", "sql/models/_sqlbuild/_macros/text/helpers.sql"),
                ],
            },
            expected_moves: &[
                (
                    "sql/models/_sqlbuild/_macros/money/helpers.sql",
                    "sql/macros/helpers.sql",
                ),
                (
                    "sql/models/_sqlbuild/_macros/text/helpers.sql",
                    "sql/macros/helpers.sql",
                ),
            ],
            expected_blocking: &[
                "sql/macros/helpers.sql:0 sql/models/_sqlbuild/_macros/money/helpers.sql and \
                 sql/models/_sqlbuild/_macros/text/helpers.sql would both move to sql/macros/helpers.sql; \
                 rename one of them first",
            ],
        },
        DeclarationMovesTestCase {
            description: "two moved models' same-named local files sharing one target block",
            on_disk: &[
                ("sql/models/sales/_sqlbuild/_macros/helpers.sql", "x"),
                ("sql/models/crm/_sqlbuild/_macros/helpers.sql", "x"),
            ],
            moves: &[
                ("sql/models/sales/orders.sql", "sql/models/marts/orders.sql"),
                ("sql/models/crm/customers.sql", "sql/models/marts/customers.sql"),
            ],
            placement: DeclarationPlacement {
                relocated: Some(vec![
                    relocated("macro:cents", "sql/models/marts/_sqlbuild/_macros/helpers.sql"),
                    relocated("macro:label", "sql/models/marts/_sqlbuild/_macros/helpers.sql"),
                ]),
                diagnostics: Vec::new(),
                declarations: vec![
                    placed("macro:cents", "sql/models/sales/_sqlbuild/_macros/helpers.sql"),
                    placed("macro:label", "sql/models/crm/_sqlbuild/_macros/helpers.sql"),
                ],
            },
            expected_moves: &[
                (
                    "sql/models/crm/_sqlbuild/_macros/helpers.sql",
                    "sql/models/marts/_sqlbuild/_macros/helpers.sql",
                ),
                (
                    "sql/models/sales/_sqlbuild/_macros/helpers.sql",
                    "sql/models/marts/_sqlbuild/_macros/helpers.sql",
                ),
            ],
            expected_blocking: &["sql/models/marts/_sqlbuild/_macros/helpers.sql:0 \
                 sql/models/crm/_sqlbuild/_macros/helpers.sql and \
                 sql/models/sales/_sqlbuild/_macros/helpers.sql would both move to \
                 sql/models/marts/_sqlbuild/_macros/helpers.sql; rename one of them first"],
        },
    ];
    for test_case in test_cases {
        let project =
            tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
        write_files(project.path(), test_case.on_disk)?;
        let (moves, blocking) =
            planned_declaration_moves(project.path(), test_case.moves, &test_case.placement)?;
        assert_eq!(
            moves,
            owned_pairs(test_case.expected_moves),
            "{}",
            test_case.description
        );
        assert_eq!(
            blocking, test_case.expected_blocking,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_unit_tests_of_model_when_renaming_or_moving_then_tests_follow() -> Result<(), RefactorError>
{
    let tests: &[(&str, &str, &[&str])] = &[
        (
            "tests/unit/marts/test_orders__keeps_rows.sql",
            "orders__keeps_rows",
            &["orders"],
        ),
        (
            "tests/unit/test_orders.sql",
            "orders__flat_layout",
            &["orders"],
        ),
        (
            "tests/unit/marts/test_marts__totals.sql",
            "marts__totals",
            &["orders", "customers"],
        ),
    ];
    let test_cases = [
        FollowedUnitTestTestCase {
            description: "a rename renames owned files and TEST names in place",
            request: request(RefactorOperation::RenameModel, "order_lines", None),
            rebound: &["macro:mock_orders"],
            expected_changes: followed_test_changes(&[
                (
                    "tests/unit/marts/test_orders__keeps_rows.sql -> tests/unit/marts/test_order_lines__keeps_rows.sql",
                    &["\"order_lines__keeps_rows\""] as &[&str],
                ),
                (
                    "tests/unit/test_orders.sql -> tests/unit/test_order_lines.sql",
                    &["\"order_lines__flat_layout\""],
                ),
            ]),
            expected_notes: &[],
        },
        FollowedUnitTestTestCase {
            description: "a move takes mirrored tests to the mirrored folder and leaves others",
            request: request(RefactorOperation::MoveModel, "", Some("sql/models/finance/")),
            rebound: &[],
            expected_changes: followed_test_changes(&[(
                "tests/unit/marts/test_orders__keeps_rows.sql -> tests/unit/finance/test_orders__keeps_rows.sql",
                &[] as &[&str],
            )]),
            expected_notes: &[],
        },
        FollowedUnitTestTestCase {
            description: "a move leaves a test whose declarations would rebind in its folder",
            request: request(RefactorOperation::MoveModel, "", Some("sql/models/finance/")),
            rebound: &["macro:mock_orders"],
            expected_changes: Vec::new(),
            expected_notes: &[
                "tests/unit/marts/test_orders__keeps_rows.sql: kept in its folder: macro:mock_orders would not resolve to the same declaration from tests/unit/finance",
            ],
        },
        FollowedUnitTestTestCase {
            description: "a move with a new name renames a kept test in its folder",
            request: request(
                RefactorOperation::MoveModel,
                "",
                Some("sql/models/finance/order_lines.sql"),
            ),
            rebound: &["enum:order_status", "macro:mock_orders"],
            expected_changes: followed_test_changes(&[
                (
                    "tests/unit/marts/test_orders__keeps_rows.sql -> tests/unit/marts/test_order_lines__keeps_rows.sql",
                    &["\"order_lines__keeps_rows\""] as &[&str],
                ),
                (
                    "tests/unit/test_orders.sql -> tests/unit/test_order_lines.sql",
                    &["\"order_lines__flat_layout\""],
                ),
            ]),
            expected_notes: &[
                "tests/unit/marts/test_order_lines__keeps_rows.sql: kept in its folder: enum:order_status, macro:mock_orders would not resolve to the same declaration from tests/unit/finance",
            ],
        },
    ];
    for test_case in test_cases {
        let (changes, notes) = followed_unit_tests(&test_case.request, tests, test_case.rebound)?;
        assert_eq!(
            changes, test_case.expected_changes,
            "{}",
            test_case.description
        );
        assert_eq!(notes, test_case.expected_notes, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_several_moves_when_planning_together_then_chains_pass_and_conflicts_block()
-> Result<(), RefactorError> {
    let project = tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
    write_files(
        project.path(),
        &[
            ("sql/models/orders.sql", "SELECT 1"),
            ("sql/models/customers.sql", "SELECT 1"),
        ],
    )?;
    let project_facts = facts(
        &project.path().to_string_lossy(),
        vec![
            model("orders", Some("view"), "analytics"),
            model("customers", Some("view"), "analytics"),
        ],
    );
    let test_cases = [
        BatchPlanTestCase {
            description: "a model takes the name another move gives up",
            requests: vec![
                move_request("orders", "sql/models/customers.sql"),
                move_request("customers", "sql/models/clients.sql"),
            ],
            expected_blocking: &["sql/models/customers.sql: model:orders would take the name \
                                  model:customers gives up in this batch; the old name must \
                                  stay free for its history, so move them in separate runs"],
            expected_moves: &[
                "sql/models/customers.sql -> sql/models/clients.sql",
                "sql/models/orders.sql -> sql/models/customers.sql",
            ],
        },
        BatchPlanTestCase {
            description: "two models moved to the same file",
            requests: vec![
                move_request("orders", "sql/models/finance/totals.sql"),
                move_request("customers", "sql/models/finance/totals.sql"),
            ],
            expected_blocking: &[
                "sql/models/finance/totals.sql: model:orders and model:customers both move to \
                 sql/models/finance/totals.sql",
            ],
            expected_moves: &[
                "sql/models/orders.sql -> sql/models/finance/totals.sql",
                "sql/models/customers.sql -> sql/models/finance/totals.sql",
            ],
        },
        BatchPlanTestCase {
            description: "one model moved twice",
            requests: vec![
                move_request("orders", "sql/models/finance/"),
                move_request("orders", "sql/models/sales/"),
            ],
            expected_blocking: &[
                "sql/models/orders.sql: model:orders is moved more than once in \
                                  this batch",
            ],
            expected_moves: &["sql/models/orders.sql -> sql/models/sales/orders.sql"],
        },
        BatchPlanTestCase {
            description: "two models given one name in different folders",
            requests: vec![
                move_request("orders", "sql/models/finance/totals.sql"),
                move_request("customers", "sql/models/sales/totals.sql"),
            ],
            expected_blocking: &[
                "sql/models/sales/totals.sql: model:orders and model:customers would both be named \
                 totals",
            ],
            expected_moves: &[
                "sql/models/orders.sql -> sql/models/finance/totals.sql",
                "sql/models/customers.sql -> sql/models/sales/totals.sql",
            ],
        },
    ];
    for test_case in test_cases {
        let (blocking, moves) = planned_batch(&project_facts, &test_case.requests)?;

        assert_eq!(
            (blocking, moves),
            (
                test_case
                    .expected_blocking
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect::<Vec<_>>(),
                test_case
                    .expected_moves
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect::<Vec<_>>()
            ),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_pending_migration_when_moving_under_same_name_then_only_relation_moves_block()
-> Result<(), RefactorError> {
    let project = tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
    write_files(
        project.path(),
        &[(
            "sql/models/orders.sql",
            "MODEL (materialized table, migrate_from old_orders);\nSELECT 1",
        )],
    )?;
    let mut pending = model("orders", Some("table"), "analytics");
    pending.declares_migrate_from = true;
    pending.migrate_from_set = true;
    let project_facts = facts(&project.path().to_string_lossy(), vec![pending.clone()]);
    let plan = RefactorPlan {
        request: request(
            RefactorOperation::MoveModel,
            "orders",
            Some("sql/models/finance/orders.sql"),
        ),
        batch: Vec::new(),
        changes: vec![change(
            "sql/models/finance/orders.sql",
            "sql/models/orders.sql",
            Vec::new(),
        )],
        manual: Vec::new(),
        blocking: Vec::new(),
        migrations: Vec::new(),
        renamed_columns: Vec::new(),
        help: None,
        notes: Vec::new(),
    };
    let test_cases = [
        PendingMigrationTestCase {
            description: "a same-name move passes the pending-migration refusal",
            request: request(RefactorOperation::MoveModel, "", Some("sql/models/finance/")),
            after: vec![pending.clone()],
            expected_refusals: vec![],
            expected_blocking: vec![],
            expected_migration_count: 0,
        },
        PendingMigrationTestCase {
            description: "a rename is refused while migrate_from is pending",
            request: request(RefactorOperation::RenameModel, "order_lines", None),
            after: vec![pending.clone()],
            expected_refusals: vec![
                "sql/models/orders.sql: model:orders still declares migrate_from; build it on every target and remove migrate_from before renaming it again",
            ],
            expected_blocking: vec![],
            expected_migration_count: 0,
        },
        PendingMigrationTestCase {
            description: "a same-name move that changes the relation is blocked",
            request: request(RefactorOperation::MoveModel, "", Some("sql/models/finance/")),
            after: vec![
                model("orders", Some("table"), "finance"),
                model("customers", Some("table"), "analytics"),
            ],
            expected_refusals: vec![],
            expected_blocking: vec![
                "sql/models/finance/orders.sql: model:orders still declares migrate_from and this move changes its relation; build it on every target and remove migrate_from before moving it again",
            ],
            expected_migration_count: 0,
        },
    ];
    for test_case in test_cases {
        let refusals = blocking_reasons(&project_facts, &test_case.request)?;
        let planned = with_model_migration(
            &plan,
            std::slice::from_ref(&pending),
            Some(&test_case.after),
            &Vec::new(),
        );

        assert_eq!(
            (
                refusals,
                planned
                    .blocking
                    .iter()
                    .map(|item| format!("{}: {}", item.path, item.reason))
                    .collect::<Vec<_>>(),
                planned.migrations.len(),
            ),
            (
                test_case
                    .expected_refusals
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect::<Vec<_>>(),
                test_case
                    .expected_blocking
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect::<Vec<_>>(),
                test_case.expected_migration_count,
            ),
            "{}",
            test_case.description
        );
    }
    Ok(())
}
