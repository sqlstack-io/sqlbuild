"""E2E coverage for reusable model schemas across materializations."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ReusableModelSchemaBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        ReusableModelSchemaBuildE2ETestCase(
            description="view table and incremental consume inherited schemas",
            expected_rows=((1, "feed"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reusable_inherited_schemas_when_building_then_all_materializations_succeed(
    test_case: ReusableModelSchemaBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="reusable_schema_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "reusable_schema_project"
                adapter = "duckdb"

                [connection]
                database = "reusable_schema.duckdb"
                """
            ).strip()
            + "\n",
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "sql/schemas/events/base.sql": dedent(
                """
                SCHEMA (description "Test schema.",
                  name event,
                    columns (
                        event_id (type INTEGER, nullable false, audits [not_null]),
                        observed_at (type TIMESTAMP, nullable false, audits [not_null]),
                  ),
                );
                """
            ).strip()
            + "\n",
            "sql/schemas/events/sourced.sql": dedent(
                """
                SCHEMA (description "Test schema.",
                  name sourced_event,
                  extends event,
                  columns (
                    source (type VARCHAR, nullable false),
                  ),
                );
                """
            ).strip()
            + "\n",
            "sql/models/stg_events.sql": dedent(
                """
                MODEL (
                  materialized view,
                  model_schema event,
                  contract enforced,
                );

                SELECT
                  event_id,
                  observed_at
                FROM __source("raw_events")
                """
            ).strip()
            + "\n",
            "sql/models/event_table.sql": dedent(
                """
                MODEL (
                  materialized table,
                  model_schema sourced_event,
                  contract enforced,
                );

                SELECT
                  event_id,
                  observed_at,
                  'feed'::VARCHAR AS source
                FROM __ref("stg_events")
                """
            ).strip()
            + "\n",
            "sql/models/event_incremental.sql": dedent(
                """
                MODEL (
                  materialized incremental,
                  incremental_strategy append,
                  cursor observed_at,
                  cursor_type timestamp,
                  cursor_grain second,
                  model_schema sourced_event,
                  columns (
                    event_id (audits [unique]),
                    batch_id (type INTEGER, nullable false),
                  ),
                  contract enforced,
                );

                SELECT
                  event_id,
                  observed_at,
                  source,
                  1::INTEGER AS batch_id
                FROM __ref("event_table")
                """
            ).strip()
            + "\n",
        },
    )
    database_path: Path = project_dir / "reusable_schema.duckdb"
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(database_path))
    connection.execute(
        """
        CREATE TABLE raw_events (event_id INTEGER NOT NULL, observed_at TIMESTAMP NOT NULL);
        INSERT INTO raw_events VALUES (1, '2026-01-01 00:00:00');
        """
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    connection = duckdb.connect(str(database_path))
    rows: list[tuple[int, str]] = connection.execute(
        "SELECT event_id, source FROM event_incremental ORDER BY event_id"
    ).fetchall()
    connection.close()
    assert tuple(rows) == test_case.expected_rows


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
