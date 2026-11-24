"""Helpers for `sqb review` E2E tests."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Mapping
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb

from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    commit_project_on_main,
    write_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

DATABASE_FILE: str = "review_orders.duckdb"
DATABASE_NAME: str = "review_orders"
REVIEW_SCHEMA_PREFIX: str = "review_"
STAGING: str = "sql/models/stg_orders.sql"
ORDER_TOTALS: str = "sql/models/order_totals.sql"
CUSTOMER_ORDERS: str = "sql/models/customer_orders.sql"
DROP_FAILURE_PLUGIN: str = "sitecustomize.py"


def build_review_project_files() -> dict[str, str]:
    """A DuckDB orders project with a production target, a dev target and one critical model."""

    return {
        "sqlbuild_project.toml": dedent(
            f"""
            name = "review_orders"
            adapter = "duckdb"
            default_target = "dev"

            [connection]
            database = "{DATABASE_FILE}"

            [targets.prod]
            schema = "prod"

            [targets.dev]
            schema = "dev"

            [targets.empty]
            schema = "empty"

            [targets.dev_database]
            database = "{DATABASE_NAME}"
            schema = "dev"

            [defaults]
            materialized = "table"

            [git]
            base_ref = "main"

            [review]
            against = "prod"

            [janitor]
            enabled = true
            """
        ).lstrip(),
        ".gitignore": "target/\nlogs/\n*.duckdb\n*.duckdb.wal\n",
        "sql/sources/raw.yml": dedent(
            """
            sources:
              - name: raw_orders
                description: Raw orders.
                expression: |
                  SELECT * FROM (VALUES
                    (1, 10, 'open', 250),
                    (2, 10, 'shipped', 1200),
                    (3, 20, 'open', 900)
                  ) AS raw_orders(order_id, customer_id, status, amount_cents)
            """
        ).lstrip(),
        STAGING: (
            "MODEL (description 'Cleaned orders.', unique_key order_id);\n\n"
            'SELECT order_id, customer_id, status, amount_cents FROM __source("raw_orders")\n'
        ),
        ORDER_TOTALS: (
            "MODEL (description 'Order totals in dollars.', unique_key order_id,\n"
            "  critical true);\n\n"
            "SELECT order_id, amount_cents / 100.0 AS amount\n"
            'FROM __ref("stg_orders")\n'
        ),
        "sql/models/open_orders.sql": (
            "MODEL (description 'Open orders.', unique_key order_id);\n\n"
            "SELECT order_id FROM __ref(\"stg_orders\") WHERE status = 'open'\n"
        ),
        "sql/models/order_ledger.sql": (
            "MODEL (\n"
            "  description 'Every order once.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key order_id,\n"
            ");\n\n"
            'SELECT order_id, status FROM __ref("stg_orders")\n'
        ),
        "tests/unit/test_order_totals.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (\n"
            "  SELECT 1 AS order_id, 10 AS customer_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__order_totals AS (SELECT 1 AS order_id, 2.5 AS amount)\nSELECT 1\n"
        ),
    }


def own_database_model(*, amount: str) -> str:
    """A model that sets its own database."""

    return (
        "MODEL (description 'Orders with their own database.', unique_key order_id,\n"
        f"  database {DATABASE_NAME});\n\n"
        f'SELECT order_id, {amount} AS amount_cents FROM __ref("stg_orders")\n'
    )


def export_task_module() -> str:
    """A Python task that depends on the critical model."""

    return dedent(
        """
        from sqlbuild.refs import model
        from sqlbuild.assets import AssetContext, asset


        @asset(depends_on=model("order_totals"))
        def export_totals(ctx: AssetContext):
            \"\"\"Export order totals.\"\"\"
            return ctx.result(metadata={"rows": 0})
        """
    ).lstrip()


def order_statuses_seed(*, rows: str) -> dict[str, str]:
    """A seed of order statuses and its column types."""

    return {
        "sql/seeds/order_statuses.csv": "status,label\n" + rows,
        "sql/seeds/order_statuses.yml": dedent(
            """
            seeds:
              - name: order_statuses
                description: Order status labels.
                columns:
                  - name: status
                    type: VARCHAR
                  - name: label
                    type: VARCHAR
            """
        ).lstrip(),
    }


def customer_orders_model(*, tags: str) -> str:
    """A model without a unique key, so its row-level check cannot run."""

    return (
        f"MODEL (description 'Orders per customer.', tags [{tags}]);\n\n"
        "SELECT customer_id, COUNT(*) AS order_count\n"
        'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
    )


def prepare_review_repository(*, tmp_path: Path, extra_files: Mapping[str, str]) -> Path:
    """Commit the review project on `main`, build production there, then branch to `feature`."""

    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="review_orders",
        repo_files={**build_review_project_files(), **extra_files},
    )
    commit_project_on_main(repository=project)
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"), project_dir=project
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return project


def edit_project(*, project_dir: Path, files: Mapping[str, str]) -> None:
    """Write uncommitted branch edits."""

    write_project_files(project_dir=project_dir, files=files)


def edited_staging(*, before: str, after: str) -> str:
    """The staging model with one SQL fragment replaced."""

    staging: str = build_review_project_files()[STAGING]
    assert before in staging
    return staging.replace(before, after)


def edited_order_totals(*, before: str, after: str) -> str:
    """The critical model with one SQL fragment replaced."""

    totals: str = build_review_project_files()[ORDER_TOTALS]
    assert before in totals
    return totals.replace(before, after)


def write_drop_failure_plugin(*, directory: Path) -> dict[str, str]:
    """A startup hook that makes every schema drop fail, and the environment that loads it."""

    directory.mkdir(parents=True, exist_ok=True)
    (directory / DROP_FAILURE_PLUGIN).write_text(
        dedent(
            """
            from sqlbuild.adapter.contract.classes.duckdb_backed_adapter import (
                DuckDbBackedAdapter,
            )


            def _failing_drop(self, *, database, schema):
                return (f"DROP SCHEMA __review_drop_is_broken__.{schema} CASCADE",)


            DuckDbBackedAdapter.render_drop_schema = _failing_drop
            """
        ).lstrip(),
        encoding="utf-8",
    )
    return {"PYTHONPATH": str(directory)}


def run_review(
    *, project_dir: Path, arguments: tuple[str, ...], env: Mapping[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run `sqb review` with colour disabled."""

    return run_sqb(command=("--no-color", "review", *arguments), project_dir=project_dir, env=env)


def review_document(result: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    """Parse the JSON document `sqb review --json` printed to stdout."""

    document: object = json.loads(result.stdout)
    assert isinstance(document, dict), result.stdout
    return document


def review_schemas(*, project_dir: Path) -> tuple[str, ...]:
    """Every review schema present in the project database."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        rows: list[tuple[Any, ...]] = connection.execute(
            "SELECT schema_name FROM information_schema.schemata "
            "WHERE starts_with(schema_name, ?) ORDER BY schema_name",
            [REVIEW_SCHEMA_PREFIX],
        ).fetchall()
    finally:
        connection.close()
    return tuple(str(row[0]) for row in rows)


def execute_review_database(*, project_dir: Path, sql: str) -> None:
    """Run mutating SQL against the project database."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        connection.execute(sql)
    finally:
        connection.close()


def node_statuses(document: Mapping[str, Any]) -> dict[str, str]:
    """Each reported node's status."""

    return {str(node["name"]): str(node["status"]) for node in document["nodes"]}
