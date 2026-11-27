"""E2E coverage for which resources sqb clone prepares, where they go, and its terminal output."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clone._test_types import (
    ClonePreparationE2ETestCase,
    ClonePreparationFailureE2ETestCase,
    CloneTerminalProgressE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clone.helpers import copy_project
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    SPINNER_GLYPHS,
    assert_fragments_in_order,
    query_duckdb,
    render_terminal_screen,
    run_sqb,
    run_sqb_with_pty,
)

_VIEW_ROWS: str = (
    "SELECT schema_name, view_name, sql FROM duckdb_views() "
    "WHERE NOT internal AND schema_name = '{schema}' ORDER BY view_name"
)
_DEV_VIEWS: tuple[tuple[object, ...], ...] = (
    (
        "dev",
        "orders_summary",
        "CREATE VIEW dev.orders_summary AS SELECT f.*, p.amount_cents AS paid_cents "
        "FROM (dev.fct_orders AS f LEFT JOIN dev.stg_payments AS p "
        "ON ((p.order_id = f.order_id)));",
    ),
    (
        "dev",
        "stg_orders",
        "CREATE VIEW dev.stg_orders AS SELECT order_id, customer_id, amount_cents, "
        "'apac' AS region FROM (SELECT * FROM (SELECT * FROM (VALUES (1, 101, 100), "
        "(2, 102, 200)) AS valueslist) AS raw_orders(order_id, customer_id, amount_cents));",
    ),
    ("dev", "stg_payments", "CREATE VIEW dev.stg_payments AS SELECT * FROM raw_dev.raw_payments;"),
)


_ALL_DEV_ASSETS: tuple[tuple[str, str, str, str | None, str], ...] = (
    ("seed", "customers", "copied", "prod.customers", "dev.customers"),
    ("model", "stg_orders", "recreated_view", "prod.stg_orders", "dev.stg_orders"),
    ("model", "fct_orders", "copied", "prod.fct_orders", "dev.fct_orders"),
    ("model", "fct_order_events", "copied", "prod.fct_order_events", "dev.fct_order_events"),
    (
        "model",
        "orders_backlog",
        "warning_missing_source",
        "prod.orders_backlog",
        "dev.orders_backlog",
    ),
    ("source", "raw_payments", "copied", "raw_prod.raw_payments", "raw_dev.raw_payments"),
    ("model", "stg_payments", "recreated_view", "prod.stg_payments", "dev.stg_payments"),
    ("model", "orders_summary", "recreated_view", "prod.orders_summary", "dev.orders_summary"),
    ("udf", "cents_to_units", "recreated_function", None, "dev.cents_to_units"),
)


@pytest.mark.parametrize(
    "test_case",
    (
        ClonePreparationE2ETestCase(
            description="whole project clones in graph order and reports the missing origin",
            clone_arguments=("--from", "prod", "--to", "dev"),
            expected_assets=_ALL_DEV_ASSETS,
            expected_query_results=(
                (_VIEW_ROWS.format(schema="dev"), _DEV_VIEWS),
                ("SELECT DISTINCT region FROM dev.stg_orders", (("apac",),)),
                (
                    "SELECT order_id, paid_cents FROM dev.orders_summary ORDER BY order_id",
                    ((1, 500), (2, None)),
                ),
            ),
        ),
        ClonePreparationE2ETestCase(
            description="graph selector clones the selected view and its ancestors",
            clone_arguments=("--from", "prod", "--to", "dev", "--select", "+orders_summary"),
            expected_assets=(
                _ALL_DEV_ASSETS[0],
                _ALL_DEV_ASSETS[1],
                _ALL_DEV_ASSETS[2],
                _ALL_DEV_ASSETS[5],
                _ALL_DEV_ASSETS[6],
                _ALL_DEV_ASSETS[7],
            ),
            expected_query_results=((_VIEW_ROWS.format(schema="dev"), _DEV_VIEWS),),
        ),
        ClonePreparationE2ETestCase(
            description="excluded view and seed are left out",
            clone_arguments=(
                "--from",
                "prod",
                "--to",
                "dev",
                "--exclude",
                "orders_summary",
                "customers",
            ),
            expected_assets=(
                _ALL_DEV_ASSETS[1],
                _ALL_DEV_ASSETS[2],
                _ALL_DEV_ASSETS[3],
                _ALL_DEV_ASSETS[4],
                _ALL_DEV_ASSETS[5],
                _ALL_DEV_ASSETS[6],
                _ALL_DEV_ASSETS[8],
            ),
            expected_query_results=(
                (_VIEW_ROWS.format(schema="dev"), (_DEV_VIEWS[1], _DEV_VIEWS[2])),
            ),
        ),
        ClonePreparationE2ETestCase(
            description="destination deferring sources reads origin sources and skips them",
            clone_arguments=("--from", "prod", "--to", "review"),
            expected_assets=(
                ("seed", "customers", "copied", "prod.customers", "review.customers"),
                ("model", "stg_orders", "recreated_view", "prod.stg_orders", "review.stg_orders"),
                ("model", "fct_orders", "copied", "prod.fct_orders", "review.fct_orders"),
                (
                    "model",
                    "fct_order_events",
                    "copied",
                    "prod.fct_order_events",
                    "review.fct_order_events",
                ),
                (
                    "model",
                    "orders_backlog",
                    "warning_missing_source",
                    "prod.orders_backlog",
                    "review.orders_backlog",
                ),
                (
                    "model",
                    "stg_payments",
                    "recreated_view",
                    "prod.stg_payments",
                    "review.stg_payments",
                ),
                (
                    "model",
                    "orders_summary",
                    "recreated_view",
                    "prod.orders_summary",
                    "review.orders_summary",
                ),
                ("udf", "cents_to_units", "recreated_function", None, "review.cents_to_units"),
            ),
            expected_query_results=(
                (
                    "SELECT sql FROM duckdb_views() "
                    "WHERE schema_name = 'review' AND view_name = 'stg_payments'",
                    (
                        (
                            "CREATE VIEW review.stg_payments AS SELECT * FROM "
                            "(SELECT * EXCLUDE (payment_id, order_id, amount_cents), "
                            "CAST(payment_id AS BIGINT) AS payment_id, "
                            "CAST(order_id AS BIGINT) AS order_id, "
                            "CAST(amount_cents AS BIGINT) AS amount_cents "
                            "FROM raw_prod.raw_payments);",
                        ),
                    ),
                ),
                ("SELECT DISTINCT region FROM review.stg_orders", (("amer",),)),
            ),
        ),
        ClonePreparationE2ETestCase(
            description="selected model missing in the origin warns beside a copied one",
            clone_arguments=(
                "--from",
                "prod",
                "--to",
                "dev",
                "--select",
                "orders_backlog",
                "fct_orders",
            ),
            expected_assets=(_ALL_DEV_ASSETS[2], _ALL_DEV_ASSETS[4]),
            expected_query_results=(("SELECT count(*) FROM dev.fct_orders", ((2,),)),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_origin_environment_when_cloning_then_items_and_relations_match_selection(
    test_case: ClonePreparationE2ETestCase, origin_project: Path, tmp_path: Path
) -> None:
    project_dir: Path = copy_project(origin_project=origin_project, tmp_path=tmp_path)
    json_path: Path = tmp_path / "clone.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "clone",
            *test_case.clone_arguments,
            "--json-output",
            str(json_path),
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert re.search(r"TOTAL=\d+  \(\d+\.\d{2}s\)\n", result.stdout), result.stdout
    assets: list[dict[str, object]] = json.loads(json_path.read_text(encoding="utf-8"))["assets"]
    assert (
        tuple(
            (
                asset["kind"],
                asset["name"],
                asset["action"],
                asset.get("origin_relation"),
                asset["target"],
            )
            for asset in assets
        )
        == test_case.expected_assets
    )
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        rows: list[tuple[object, ...]] = query_duckdb(
            db_path=project_dir / "clone.duckdb", sql=query
        )
        assert tuple(tuple(row) for row in rows) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        CloneTerminalProgressE2ETestCase(
            description="finished preparation leaves no spinner over the clone output",
            columns=120,
            expected_screen_fragments=(
                "✓ Warehouse connected  duckdb",
                "Prepared clone plan.",
                "sqb clone  origin=prod destination=dev  (2 resources)",
                "fct_orders",
                "orders_backlog",
                "Completed with warnings",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_terminal_when_cloning_then_preparation_spinner_stops_before_items(
    test_case: CloneTerminalProgressE2ETestCase, origin_project: Path, tmp_path: Path
) -> None:
    project_dir: Path = copy_project(origin_project=origin_project, tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb_with_pty(
        command=(
            "clone",
            "--from",
            "prod",
            "--to",
            "dev",
            "--select",
            "fct_orders",
            "orders_backlog",
        ),
        project_dir=project_dir,
        columns=test_case.columns,
    )

    screen: tuple[str, ...] = render_terminal_screen(
        output=result.stdout, columns=test_case.columns
    )
    rendered: str = "\n".join(screen)
    assert result.returncode == 0, rendered
    assert not any(glyph in rendered for glyph in SPINNER_GLYPHS), rendered
    assert_fragments_in_order(rendered, test_case.expected_screen_fragments)
    after_preparation: str = result.stdout.split("Prepared clone plan.", maxsplit=1)[1]
    assert not any(glyph in after_preparation for glyph in SPINNER_GLYPHS), after_preparation


@pytest.mark.parametrize(
    "test_case",
    (
        ClonePreparationFailureE2ETestCase(
            description="compile error stops the preparation spinner before the error prints",
            columns=120,
            broken_model_path="sql/models/marts/orders_backlog.sql",
            broken_model_sql=(
                'MODEL (description "Test model orders_backlog.", materialized table);\n\n'
                'SELECT order_id FROM __ref("missing_orders")\n'
            ),
            expected_screen_fragments=(
                "✓ Warehouse connected  duckdb",
                "error[P001]: Model file sql/models/marts/orders_backlog.sql references unknown "
                "model 'missing_orders'",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failing_preparation_in_terminal_when_cloning_then_error_has_no_spinner(
    test_case: ClonePreparationFailureE2ETestCase, origin_project: Path, tmp_path: Path
) -> None:
    project_dir: Path = copy_project(origin_project=origin_project, tmp_path=tmp_path)
    (project_dir / test_case.broken_model_path).write_text(
        test_case.broken_model_sql, encoding="utf-8"
    )

    result: subprocess.CompletedProcess[str] = run_sqb_with_pty(
        command=("clone", "--from", "prod", "--to", "dev"),
        project_dir=project_dir,
        columns=test_case.columns,
    )

    screen: tuple[str, ...] = render_terminal_screen(
        output=result.stdout, columns=test_case.columns
    )
    rendered: str = "\n".join(screen)
    assert result.returncode == 1, rendered
    assert not any(glyph in rendered for glyph in SPINNER_GLYPHS), rendered
    assert_fragments_in_order(rendered, test_case.expected_screen_fragments)
    before_error: str = result.stdout.split("error[P001]", maxsplit=1)[0]
    assert before_error.rsplit("\x1b[?25", maxsplit=1)[1].startswith("h"), repr(before_error[-80:])


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
