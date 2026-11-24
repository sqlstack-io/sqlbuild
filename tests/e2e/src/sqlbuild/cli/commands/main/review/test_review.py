"""`sqb review` builds a branch's changes in a run-owned schema, compares and reports."""

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.review._test_types import (
    ReviewCleanupFailureE2ETestCase,
    ReviewDatabaseE2ETestCase,
    ReviewDryRunE2ETestCase,
    ReviewIncrementalStartE2ETestCase,
    ReviewMarkdownFailureE2ETestCase,
    ReviewOutcomeE2ETestCase,
    ReviewSelectedAncestorE2ETestCase,
    ReviewSelectErrorE2ETestCase,
    ReviewSelectionE2ETestCase,
    ReviewSetupFailureE2ETestCase,
    ReviewTextOutputE2ETestCase,
    ReviewUsageErrorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.review.helpers import (
    CUSTOMER_ORDERS,
    DATABASE_NAME,
    ORDER_TOTALS,
    STAGING,
    customer_orders_model,
    edit_project,
    edited_order_totals,
    edited_staging,
    execute_review_database,
    export_task_module,
    node_statuses,
    order_statuses_seed,
    own_database_model,
    prepare_review_repository,
    review_document,
    review_schemas,
    run_review,
    write_drop_failure_plugin,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_SAME_VALUES_STAGING: str = edited_staging(
    before="SELECT order_id, customer_id, status,",
    after="SELECT order_id, customer_id, LOWER(status) AS status,",
)
_CHANGED_VALUES_TOTALS: str = edited_order_totals(before="/ 100.0", after="/ 10.0")
_NEW_COLUMN_TOTALS: str = edited_order_totals(
    before="AS amount\n", after="AS amount, amount_cents\n"
)
_DOUBLED_AMOUNTS_STAGING: str = edited_staging(
    before="status, amount_cents FROM", after="status, amount_cents * 2 AS amount_cents FROM"
)
_HOOKED_TOTALS: str = edited_order_totals(
    before="critical true);", after='critical true, post_hooks [inline_sql("SELECT 1")]);'
)
_EXPORT_TASK: str = "python/nodes/tasks/exports.py"
_OWN_DATABASE_ORDERS: str = "sql/models/own_database_orders.sql"


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewOutcomeE2ETestCase(
            description="the default policy builds the change and its critical downstream only",
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=(),
            expected_exit_code=0,
            expected_status="passed",
            expected_validated=("stg_orders", "order_totals"),
            expected_not_validated=("open_orders", "order_ledger"),
            expected_node_statuses={
                "stg_orders": "ok",
                "order_totals": "ok",
                "open_orders": "not_validated",
                "order_ledger": "not_validated",
            },
        ),
        ReviewOutcomeE2ETestCase(
            description="--downstream none builds only the changed model",
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=("--downstream", "none"),
            expected_exit_code=0,
            expected_status="passed",
            expected_validated=("stg_orders",),
            expected_not_validated=("order_totals", "open_orders", "order_ledger"),
            expected_node_statuses={"stg_orders": "ok", "order_totals": "not_validated"},
        ),
        ReviewOutcomeE2ETestCase(
            description="--downstream all builds every downstream model, the incremental included",
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=("--downstream", "all"),
            expected_exit_code=0,
            expected_status="passed",
            expected_validated=("stg_orders", "open_orders", "order_ledger", "order_totals"),
            expected_node_statuses={
                "stg_orders": "ok",
                "order_totals": "ok",
                "open_orders": "ok",
                "order_ledger": "ok",
            },
        ),
        ReviewOutcomeE2ETestCase(
            description="a changed value in a critical model is a finding",
            edits={ORDER_TOTALS: _CHANGED_VALUES_TOTALS},
            arguments=(),
            expected_exit_code=1,
            expected_status="findings",
            expected_validated=("order_totals",),
            expected_node_statuses={"order_totals": "finding"},
            expected_finding_kinds=("values", "test"),
        ),
        ReviewOutcomeE2ETestCase(
            description="a new column is reported as a schema change",
            edits={ORDER_TOTALS: _NEW_COLUMN_TOTALS},
            arguments=(),
            expected_exit_code=1,
            expected_status="findings",
            expected_validated=("order_totals",),
            expected_node_statuses={"order_totals": "finding"},
            expected_finding_kinds=("schema",),
        ),
        ReviewOutcomeE2ETestCase(
            description="a Python task downstream of the change is listed as not validated",
            extra_files={_EXPORT_TASK: export_task_module()},
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=(),
            expected_exit_code=0,
            expected_status="passed",
            expected_validated=("stg_orders", "order_totals"),
            expected_not_validated=("open_orders", "order_ledger", "asset:export_totals"),
            expected_node_statuses={"asset:export_totals": "not_validated"},
        ),
        ReviewOutcomeE2ETestCase(
            description="a changed seed is built but never passes uncompared",
            extra_files=order_statuses_seed(rows="open,Open\n"),
            edits=order_statuses_seed(rows="open,Open\nshipped,Shipped\n"),
            arguments=(),
            expected_exit_code=2,
            expected_status="incomplete",
            expected_validated=("order_statuses",),
            expected_node_statuses={"order_statuses": "built"},
            expected_incomplete_nodes=("order_statuses",),
        ),
        ReviewOutcomeE2ETestCase(
            description="a critical model without a unique key leaves the review incomplete",
            extra_files={CUSTOMER_ORDERS: customer_orders_model(tags="critical")},
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=(),
            expected_exit_code=2,
            expected_status="incomplete",
            expected_validated=("stg_orders", "customer_orders", "order_totals"),
            expected_not_validated=("open_orders", "order_ledger"),
            expected_node_statuses={"customer_orders": "built"},
            expected_incomplete_nodes=("customer_orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_branch_edit_when_reviewing_then_report_shows_outcome(
    test_case: ReviewOutcomeE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(
        tmp_path=tmp_path, extra_files=test_case.extra_files
    )
    edit_project(project_dir=project_dir, files=test_case.edits)

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", *test_case.arguments)
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["status"] == test_case.expected_status, result.stdout
    assert document["exit_code"] == test_case.expected_exit_code
    assert sorted(document["validated"]) == sorted(test_case.expected_validated)
    assert sorted(item["name"] for item in document["not_validated"]) == sorted(
        test_case.expected_not_validated
    )
    statuses: dict[str, str] = node_statuses(document)
    for name, status in test_case.expected_node_statuses.items():
        assert statuses.get(name) == status, (name, result.stdout)
    finding_kinds: set[str] = {finding["kind"] for finding in document["findings"]}
    assert finding_kinds == set(test_case.expected_finding_kinds), result.stdout
    incomplete_nodes: set[str] = {check["node"] for check in document["incomplete"]}
    assert set(test_case.expected_incomplete_nodes) <= incomplete_nodes, result.stdout
    assert document["review_schema"]["created"] is True
    assert document["review_schema"]["dropped"] is True
    assert document["recall"]["command"].startswith("sqb recall output ")
    assert review_schemas(project_dir=project_dir) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewIncrementalStartE2ETestCase(
            description="DuckDB has no zero-copy clone, so the incremental is fully refreshed",
            arguments=("--downstream", "all"),
            expected_starts=(("order_ledger", "full_refresh"),),
        ),
        ReviewIncrementalStartE2ETestCase(
            description="--no-clone fully refreshes the incremental",
            arguments=("--downstream", "all", "--no-clone"),
            expected_starts=(("order_ledger", "full_refresh"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_incremental_downstream_when_reviewing_then_its_start_is_reported(
    test_case: ReviewIncrementalStartE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files={STAGING: _SAME_VALUES_STAGING})

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", *test_case.arguments)
    )

    assert result.returncode == 0, result.stdout + result.stderr
    starts: list[dict[str, Any]] = review_document(result)["baseline"]["incremental_starts"]
    assert tuple((start["node"], start["method"]) for start in starts) == (
        test_case.expected_starts
    ), result.stdout
    assert all(start["reason"] for start in starts), result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewSelectErrorE2ETestCase(
            description="an unknown name",
            edits={ORDER_TOTALS: _CHANGED_VALUES_TOTALS},
            select="no_such_model",
            expected_code="C761",
            expected_message_fragment="no_such_model",
        ),
        ReviewSelectErrorE2ETestCase(
            description="a model the branch did not change or affect",
            edits={ORDER_TOTALS: _CHANGED_VALUES_TOTALS},
            select="open_orders",
            expected_code="C762",
            expected_message_fragment="open_orders",
        ),
        ReviewSelectErrorE2ETestCase(
            description="an affected model the downstream policy leaves out",
            edits={STAGING: _SAME_VALUES_STAGING},
            select="open_orders",
            expected_code="C763",
            expected_message_fragment="open_orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unusable_select_when_reviewing_then_it_fails_with_json_error(
    test_case: ReviewSelectErrorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files=test_case.edits)

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", "--select", test_case.select)
    )

    assert result.returncode == 3, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["status"] == "failed"
    assert document["error"]["code"] == test_case.expected_code, result.stdout
    assert test_case.expected_message_fragment in document["error"]["message"]
    assert review_schemas(project_dir=project_dir) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewSelectionE2ETestCase(
            description="selecting the changed model leaves its critical downstream unvalidated",
            select="stg_orders",
            expected_validated=("stg_orders",),
            expected_not_validated_member="order_totals",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_select_inside_changed_set_when_reviewing_then_only_selection_is_validated(
    test_case: ReviewSelectionE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files={STAGING: _SAME_VALUES_STAGING})

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", "--select", test_case.select)
    )

    assert result.returncode == 0, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert tuple(document["validated"]) == test_case.expected_validated
    assert test_case.expected_not_validated_member in {
        item["name"] for item in document["not_validated"]
    }


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewSelectedAncestorE2ETestCase(
            description="selecting a model below a change builds and compares the change too",
            edits={STAGING: _DOUBLED_AMOUNTS_STAGING},
            select="order_totals",
            expected_exit_code=1,
            expected_validated=("stg_orders", "order_totals"),
            expected_added=(("stg_orders", "order_totals"),),
            expected_finding_nodes=frozenset({"stg_orders", "order_totals"}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_select_below_change_when_reviewing_then_changed_ancestor_is_built_and_compared(
    test_case: ReviewSelectedAncestorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files=test_case.edits)

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", "--select", test_case.select)
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert tuple(document["validated"]) == test_case.expected_validated
    assert (
        tuple((item["name"], item["required_by"]) for item in document["added"])
        == test_case.expected_added
    )
    assert {finding["node"] for finding in document["findings"]} == (
        test_case.expected_finding_nodes
    ), result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewDatabaseE2ETestCase(
            description="a target with a database builds the node in the review schema there",
            edits={_OWN_DATABASE_ORDERS: own_database_model(amount="amount_cents + 0")},
            arguments=("--target", "dev_database"),
            expected_exit_code=0,
            expected_database=DATABASE_NAME,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_with_own_database_when_reviewing_then_it_builds_in_review_database(
    test_case: ReviewDatabaseE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(
        tmp_path=tmp_path,
        extra_files={_OWN_DATABASE_ORDERS: own_database_model(amount="amount_cents")},
    )
    edit_project(project_dir=project_dir, files=test_case.edits)

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", *test_case.arguments)
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["review_schema"]["database"] == test_case.expected_database
    assert document["review_schema"]["dropped"] is True
    assert review_schemas(project_dir=project_dir) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewUsageErrorE2ETestCase(
            description="an unknown downstream policy",
            arguments=("--downstream", "some"),
            expected_message_fragment="invalid choice: 'some'",
        ),
        ReviewUsageErrorE2ETestCase(
            description="an unknown option",
            arguments=("--no-such-option",),
            expected_message_fragment="unrecognized arguments: --no-such-option",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_arguments_when_reviewing_then_it_exits_3_with_json_error(
    test_case: ReviewUsageErrorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})

    machine: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", *test_case.arguments)
    )
    human: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=test_case.arguments
    )

    assert machine.returncode == 3, machine.stdout + machine.stderr
    document: dict[str, Any] = review_document(machine)
    assert document["status"] == "failed"
    assert document["exit_code"] == 3
    assert document["error"]["code"] == "C900"
    assert test_case.expected_message_fragment in document["error"]["message"]
    assert human.returncode == 3, human.stdout + human.stderr
    assert test_case.expected_message_fragment in human.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewSetupFailureE2ETestCase(
            description="a target without a database refuses a node that sets its own",
            extra_files={_OWN_DATABASE_ORDERS: own_database_model(amount="amount_cents")},
            edits={_OWN_DATABASE_ORDERS: own_database_model(amount="amount_cents + 0")},
            arguments=(),
            expected_code="C776",
            expected_message_fragment="own_database_orders",
            expected_node_statuses=frozenset({"planned"}),
        ),
        ReviewSetupFailureE2ETestCase(
            description="an --against target that holds none of the compared tables",
            edits={STAGING: _SAME_VALUES_STAGING},
            arguments=("--against", "empty"),
            expected_code="C774",
            expected_node_statuses=frozenset({"planned", "not_validated"}),
            expected_message_fragment="--against has none of these relations; check the target",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unusable_setup_when_reviewing_then_it_fails_before_creating_schema(
    test_case: ReviewSetupFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(
        tmp_path=tmp_path, extra_files=test_case.extra_files
    )
    edit_project(project_dir=project_dir, files=test_case.edits)

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", *test_case.arguments)
    )

    assert result.returncode == 3, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["error"]["code"] == test_case.expected_code, result.stdout
    assert test_case.expected_message_fragment in document["error"]["message"]
    assert not (document["review_schema"] or {}).get("created")
    assert set(node_statuses(document).values()) <= test_case.expected_node_statuses, result.stdout
    assert all(node["build_status"] is None for node in document["nodes"]), result.stdout
    assert review_schemas(project_dir=project_dir) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewMarkdownFailureE2ETestCase(
            description="a directory as the Markdown path of a review with findings",
            edits={ORDER_TOTALS: _CHANGED_VALUES_TOTALS},
            expected_stdout_fragment="Review failed",
            expected_code="C775",
            expected_status="failed",
            expected_finding_kinds=frozenset({"values", "test"}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unwritable_markdown_path_when_reviewing_then_result_prints_and_it_exits_3(
    test_case: ReviewMarkdownFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files=test_case.edits)

    human: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--markdown", str(tmp_path))
    )
    machine: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", "--markdown", str(tmp_path))
    )

    assert human.returncode == 3, human.stdout + human.stderr
    assert test_case.expected_stdout_fragment in human.stdout
    assert test_case.expected_code in human.stderr
    assert "Traceback" not in human.stderr
    assert machine.returncode == 3, machine.stdout + machine.stderr
    document: dict[str, Any] = review_document(machine)
    assert document["exit_code"] == machine.returncode
    assert document["status"] == test_case.expected_status
    assert document["error"]["code"] == test_case.expected_code
    assert {finding["kind"] for finding in document["findings"]} == (
        test_case.expected_finding_kinds
    ), machine.stdout
    assert "Traceback" not in machine.stderr
    assert review_schemas(project_dir=project_dir) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewDryRunE2ETestCase(
            description="a dry run plans the review without creating a schema or building",
            expected_status="planned",
            expected_validated=("order_totals", "stg_orders"),
            expected_node_statuses=frozenset({"planned", "not_validated"}),
        ),
        ReviewDryRunE2ETestCase(
            description="a dry run lists the nodes whose hooks would run",
            edits={STAGING: _SAME_VALUES_STAGING, ORDER_TOTALS: _HOOKED_TOTALS},
            expected_status="planned",
            expected_validated=("order_totals", "stg_orders"),
            expected_node_statuses=frozenset({"planned", "not_validated"}),
            expected_hooks=("order_totals",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dry_run_when_reviewing_then_nothing_is_built_or_created(
    test_case: ReviewDryRunE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files=test_case.edits or {STAGING: _SAME_VALUES_STAGING})

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json", "--dry-run")
    )

    assert result.returncode == 0, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["dry_run"] is True
    assert document["status"] == test_case.expected_status
    assert document["review_schema"] is None
    assert tuple(sorted(document["validated"])) == test_case.expected_validated
    assert set(node_statuses(document).values()) <= test_case.expected_node_statuses
    assert tuple(document["hooks"]) == test_case.expected_hooks
    assert review_schemas(project_dir=project_dir) == ()
    assert not (project_dir / "target" / "review").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewTextOutputE2ETestCase(
            description="a passing review lists what it left out and how to recall the run",
            edits={STAGING: _SAME_VALUES_STAGING},
            expected_stdout_fragments=(
                "Review passed",
                "What changed\n  stg_orders (model): model SQL changed",
                "open_orders [not validated: not on a path to a critical model",
                "sqb recall output ",
            ),
            expected_markdown_fragments=(
                "# Review passed\n",
                "## What changed\n\n- `stg_orders` (model): model SQL changed",
                "open_orders [not validated: not on a path to a critical model",
                "Recall: `sqb recall output ",
            ),
        ),
        ReviewTextOutputE2ETestCase(
            description="a review with findings names the changed column",
            edits={ORDER_TOTALS: _CHANGED_VALUES_TOTALS},
            expected_exit_code=1,
            expected_stdout_fragments=("Review needs a fix", "order_totals", "amount"),
            expected_markdown_fragments=("## Findings", "order_totals", "amount"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_markdown_path_when_reviewing_then_text_and_markdown_are_written(
    test_case: ReviewTextOutputE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files=test_case.edits)
    markdown_path: Path = tmp_path / "out" / "review.md"

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--markdown", str(markdown_path))
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout
    markdown: str = markdown_path.read_text(encoding="utf-8")
    for fragment in test_case.expected_markdown_fragments:
        assert fragment in markdown, markdown


@pytest.mark.parametrize(
    "test_case",
    (
        ReviewCleanupFailureE2ETestCase(
            description="a failed drop exits 3 and the janitor removes only the expired leftover",
            expired_schema="review_20200101t000000z_0000000_000000",
            expected_exit_code=3,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failing_schema_drop_when_reviewing_then_it_exits_3_and_janitor_cleans_old_schema(
    test_case: ReviewCleanupFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_review_repository(tmp_path=tmp_path, extra_files={})
    edit_project(project_dir=project_dir, files={STAGING: _SAME_VALUES_STAGING})
    plugin_env: dict[str, str] = write_drop_failure_plugin(directory=tmp_path / "plugin")

    result: subprocess.CompletedProcess[str] = run_review(
        project_dir=project_dir, arguments=("--json",), env=plugin_env
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    document: dict[str, Any] = review_document(result)
    assert document["status"] == "failed"
    schema: dict[str, Any] = document["review_schema"]
    assert schema["created"] is True
    assert schema["dropped"] is False
    assert schema["error"]
    leftover: str = schema["name"]
    assert review_schemas(project_dir=project_dir) == (leftover,)

    expired: str = test_case.expired_schema
    execute_review_database(
        project_dir=project_dir,
        sql=(
            f"CREATE SCHEMA {expired};"
            "INSERT INTO dev._sqlbuild_fingerprints "
            f"SELECT * REPLACE ('{expired}' AS node_name, '{expired}' AS target_schema) "
            "FROM dev._sqlbuild_fingerprints "
            f"WHERE node_type = 'review_schema' AND node_name = '{leftover}'"
        ),
    )
    janitor: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "janitor", "--json", "--auto-approve"), project_dir=project_dir
    )

    assert janitor.returncode == 0, janitor.stdout + janitor.stderr
    janitor_document: dict[str, Any] = json.loads(janitor.stdout)
    assert janitor_document["plan"]["review_schemas"] == [expired], janitor.stdout
    assert review_schemas(project_dir=project_dir) == (leftover,)
