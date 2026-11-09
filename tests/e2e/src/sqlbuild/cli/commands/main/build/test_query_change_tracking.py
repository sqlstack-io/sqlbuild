"""E2E regression tests for query-change tracking across repeated CLI runs."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    QueryChangeTrackingBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_OLDER_SCHEME_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "older_scheme_project"
        adapter = "duckdb"

        [connection]
        database = "regression.duckdb"

        [defaults]
        materialized = "table"
        """
    ).strip()
    + "\n",
    "sources/raw.yml": dedent(
        """
        sources:
          - name: raw_orders
            description: Test source raw_orders.
            schema: main
            table: raw_orders
        """
    ).strip()
    + "\n",
    "models/stg_orders.sql": dedent(
        """
        MODEL (description "Test model stg_orders.");

        SELECT id AS order_id, amount_cents FROM __source("raw_orders")
        """
    ).strip()
    + "\n",
    "models/fact_orders.sql": dedent(
        """
        MODEL (description "Test model fact_orders.");

        SELECT order_id, amount_cents AS line_total_cents FROM __ref("stg_orders")
        """
    ).strip()
    + "\n",
}


@pytest.mark.parametrize(
    "test_case",
    [
        QueryChangeTrackingBuildE2ETestCase(
            description="unchanged source and ref models are not query_changed after build",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "query_change_tracking_project"
                adapter = "duckdb"

                [connection]
                database = "regression.duckdb"

                [defaults]
                materialized = "table"
                    """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                    CREATE TABLE IF NOT EXISTS raw_orders (
                      id INTEGER,
                      customer_id INTEGER,
                      ordered_at TIMESTAMP,
                      amount_cents INTEGER
                    );

                    INSERT INTO raw_orders VALUES
                      (1, 10, '2026-01-01 00:30:00', 100),
                      (2, 11, '2026-01-01 01:30:00', 200);
                    """
                ).strip()
                + "\n",
                "sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_orders
                        description: Test source raw_orders.
                        schema: main
                        table: raw_orders
                    """
                ).strip()
                + "\n",
                "models/staging/stg_orders.sql": dedent(
                    """
                    MODEL (description "Test model stg_orders.",
                      materialized view
                    );

                    SELECT
                      id AS order_id,
                      customer_id,
                      ordered_at,
                      amount_cents
                    FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
                "models/marts/fact_orders.sql": dedent(
                    """
                    MODEL (description "Test model fact_orders.",
                      materialized table
                    );

                    SELECT
                      order_id,
                      customer_id,
                      ordered_at,
                      amount_cents AS line_total_cents
                    FROM __ref("stg_orders")
                    """
                ).strip()
                + "\n",
            },
            build_command=("--no-color", "build"),
            plan_command=("plan", "--json"),
            expected_exit_code=0,
            expected_fingerprint_models=("fact_orders", "stg_orders"),
            expected_unchanged_models=("fact_orders", "stg_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unchanged_project_when_planning_after_build_then_models_are_not_query_changed(
    test_case: QueryChangeTrackingBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="query_change_tracking_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute((project_dir / "seed_raw_data.sql").read_text(encoding="utf-8"))
    connection.close()

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command,
        project_dir=project_dir,
    )

    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    assert table_exists(db_path=db_path, table_name="_sqlbuild_fingerprints")

    fingerprint_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT node_type, node_name, target_database, target_schema, target_name, run_id, "
            "definition_hash, version_hash, schema_fingerprint, definition_b64, "
            "metadata_json_b64, ts FROM main._sqlbuild_fingerprints ORDER BY node_name"
        ),
    )
    assert tuple(row[1] for row in fingerprint_rows) == test_case.expected_fingerprint_models
    assert all(isinstance(row[9], str) and row[9] for row in fingerprint_rows)

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        stale_row: tuple[object, ...] = tuple(fingerprint_rows[0])
        connection.execute(
            "INSERT INTO main._sqlbuild_fingerprints "
            "(node_type, node_name, target_database, target_schema, target_name, run_id, "
            "definition_hash, version_hash, schema_fingerprint, definition_b64, "
            "metadata_json_b64, ts) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                stale_row[0],
                stale_row[1],
                stale_row[2],
                stale_row[3],
                stale_row[4],
                "stale_run",
                "stale_definition_hash",
                "stale_version_hash",
                stale_row[8],
                stale_row[9],
                stale_row[10],
                "2000-01-01 00:00:00",
            ),
        )
    finally:
        connection.close()

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.plan_command,
        project_dir=project_dir,
    )

    assert plan_result.returncode == test_case.expected_exit_code, (
        plan_result.stdout + plan_result.stderr
    )

    plan_payload: dict[str, object] = json.loads(plan_result.stdout)
    model_entries: list[dict[str, object]] = plan_payload["models"]
    entries_by_name: dict[str, dict[str, object]] = {
        str(entry["name"]): entry for entry in model_entries
    }
    assert len(entries_by_name) == len(model_entries)
    reasons_by_name: dict[str, str] = {
        model_name: str(entries_by_name[model_name]["reason"])
        for model_name in test_case.expected_unchanged_models
    }

    model_name: str
    for model_name in test_case.expected_unchanged_models:
        assert reasons_by_name[model_name] != "query_changed"


@pytest.mark.parametrize(
    "test_case",
    [
        QueryChangeTrackingBuildE2ETestCase(
            description="hashes recorded under an older fingerprint scheme change only for edited models",
            repo_files=_OLDER_SCHEME_PROJECT_FILES,
            build_command=("--no-color", "build"),
            plan_command=("plan", "--json"),
            expected_exit_code=0,
            expected_fingerprint_models=("fact_orders", "stg_orders"),
            expected_unchanged_models=("stg_orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_hashes_from_older_fingerprint_scheme_when_rebuilding_then_only_edited_models_change(
    test_case: QueryChangeTrackingBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="older_scheme_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "regression.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql="CREATE TABLE raw_orders AS SELECT 1 AS id, 100 AS amount_cents",
    )
    first_build: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command, project_dir=project_dir
    )
    assert first_build.returncode == test_case.expected_exit_code, (
        first_build.stdout + first_build.stderr
    )
    execute_duckdb(
        db_path=db_path,
        sql="UPDATE main._sqlbuild_fingerprints SET definition_hash = 'older-scheme-' || node_name",
    )
    edited_model: Path = project_dir / "models" / "fact_orders.sql"
    edited_model.write_text(
        edited_model.read_text(encoding="utf-8").replace("line_total_cents", "total_cents"),
        encoding="utf-8",
    )

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.plan_command, project_dir=project_dir
    )
    second_build: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command, project_dir=project_dir
    )

    assert plan_result.returncode == test_case.expected_exit_code, (
        plan_result.stdout + plan_result.stderr
    )
    reasons: dict[str, str] = {
        str(entry["name"]): str(entry["reason"])
        for entry in json.loads(plan_result.stdout)["models"]
    }
    assert reasons["fact_orders"] == "query_changed"
    for model_name in test_case.expected_unchanged_models:
        assert reasons[model_name] != "query_changed"
    assert second_build.returncode == test_case.expected_exit_code, (
        second_build.stdout + second_build.stderr
    )
    latest_hashes: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT node_name, arg_max(definition_hash, ts) FROM main._sqlbuild_fingerprints "
            "GROUP BY node_name ORDER BY node_name"
        ),
    )
    assert tuple(name for name, _ in latest_hashes) == test_case.expected_fingerprint_models
    latest: dict[str, str] = dict(latest_hashes)
    for model_name in test_case.expected_unchanged_models:
        assert latest[model_name] == f"older-scheme-{model_name}"
    assert latest["fact_orders"] != "older-scheme-fact_orders"
