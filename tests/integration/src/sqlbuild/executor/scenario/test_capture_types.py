from __future__ import annotations

from datetime import date
from decimal import Decimal
from pathlib import Path
from typing import Any
from uuid import UUID

import duckdb
import pytest

from sqlbuild._native import CompiledRelationLocation, ScenarioArtifactKind
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.scenario._helpers.snapshots.core import read_scenario_snapshot_manifest
from sqlbuild.executor.scenario.main._capture import execute_scenario_snapshot_capture
from sqlbuild.executor.scenario.models import (
    ScenarioSnapshotCapturePlan,
    ScenarioSnapshotCaptureRelationPlan,
    ScenarioSnapshotCaptureResult,
    ScenarioSnapshotManifest,
    ScenarioSnapshotRelation,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.scenario._test_types import (
    ScenarioSnapshotCaptureTypesIntegrationTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotCaptureTypesIntegrationTestCase(
            description="typed duckdb capture jsonl reloads with manifest schema",
            expected_local_types={
                "decimal_scale_col": "DECIMAL(12,2)",
                "date_col": "DATE",
                "timestamp_col": "TIMESTAMP",
                "json_col": "JSON",
                "interval_col": "INTERVAL",
                "blob_col": "VARCHAR",
                "geometry_col": "VARCHAR",
            },
            expected_replay_summary_rows=(
                (
                    2,
                    Decimal("123.45"),
                    date(2026, 1, 2),
                    UUID("550e8400-e29b-41d4-a716-446655440000"),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_duckdb_relation_when_capturing_then_jsonl_reloads_with_manifest_schema(
    test_case: ScenarioSnapshotCaptureTypesIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    tmp_path: Path,
) -> None:
    adapter.execute(
        connection=connection,
        sql="""
        CREATE TABLE typed_capture_source AS
        SELECT
          TRUE AS bool_col,
          CAST(1 AS TINYINT) AS tinyint_col,
          CAST(2 AS SMALLINT) AS smallint_col,
          CAST(3 AS INTEGER) AS int_col,
          CAST(4 AS BIGINT) AS bigint_col,
          CAST(5 AS HUGEINT) AS hugeint_col,
          CAST(6 AS UTINYINT) AS utinyint_col,
          CAST(7 AS USMALLINT) AS usmallint_col,
          CAST(8 AS UINTEGER) AS uinteger_col,
          CAST(9 AS UBIGINT) AS ubigint_col,
          CAST(10 AS UHUGEINT) AS uhugeint_col,
          CAST(123 AS DECIMAL(12,0)) AS decimal_zero_col,
          CAST(123.45 AS DECIMAL(12,2)) AS decimal_scale_col,
          CAST(1.25 AS REAL) AS real_col,
          CAST(2.5 AS DOUBLE) AS double_col,
          CAST('hello' AS TEXT) AS text_col,
          DATE '2026-01-02' AS date_col,
          TIME '03:04:05' AS time_col,
          TIMESTAMP '2026-01-02 03:04:05' AS timestamp_col,
          TIMESTAMPTZ '2026-01-02 03:04:05+00' AS timestamptz_col,
          CAST('{"x":1}' AS JSON) AS json_col,
          CAST('abc' AS BLOB) AS blob_col,
          CAST('550e8400-e29b-41d4-a716-446655440000' AS UUID) AS uuid_col,
          INTERVAL '2 days' AS interval_col,
          [1, 2, 3] AS list_col,
          {'a': 1, 'b': 'x'} AS struct_col,
          map(['a', 'b'], [1, 2]) AS map_col,
          {'variant': 1} AS variant_col,
          CAST('POINT (1 2)' AS GEOMETRY) AS geometry_col
        UNION ALL
        SELECT
          NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
          NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
          NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
        """,
    )
    capture_plan: ScenarioSnapshotCapturePlan = ScenarioSnapshotCapturePlan(
        scenario_name="typed_capture",
        snapshot_root=tmp_path / "tests" / "_scenario_snapshots" / "typed_capture",
        manifest_path=tmp_path
        / "tests"
        / "_scenario_snapshots"
        / "typed_capture"
        / "scenario.json",
        input_fingerprint="typed-inputs",
        relations=(
            ScenarioSnapshotCaptureRelationPlan(
                kind=ScenarioArtifactKind.SOURCE,
                logical_name="typed_source",
                source_target=CompiledRelationLocation(
                    database=None,
                    schema=None,
                    name="typed_capture_source",
                    qualified_name="typed_capture_source",
                ),
                file_path=Path("sources/typed_source.jsonl"),
                capture_sql="SELECT * FROM typed_capture_source",
            ),
        ),
    )
    manifest: ScenarioSnapshotManifest = ScenarioSnapshotManifest(
        version=1,
        scenario_name="typed_capture",
        captured_at="2026-05-10T00:00:00Z",
        capture_adapter="duckdb",
        capture_dialect="duckdb",
        sqlbuild_version="0.2.1",
        input_fingerprint="typed-inputs",
        total_rows=0,
        total_bytes=0,
    )

    result: ScenarioSnapshotCaptureResult = execute_scenario_snapshot_capture(
        capture_plan=capture_plan,
        manifest=manifest,
        adapter=adapter,
        connection=connection,
    )

    assert result.status == ExecutionStatus.SUCCESS
    stored_manifest: ScenarioSnapshotManifest = read_scenario_snapshot_manifest(
        manifest_path=capture_plan.manifest_path
    )
    assert stored_manifest.total_rows == 2
    relation: ScenarioSnapshotRelation = stored_manifest.relations[0]
    local_types_by_name: dict[str, str] = {
        column.name: column.local_type for column in relation.columns
    }
    assert local_types_by_name | test_case.expected_local_types == local_types_by_name

    replay_connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    column_defs: str = ", ".join(
        f'"{column.name}" {column.local_type}' for column in relation.columns
    )
    replay_connection.execute(f"CREATE TABLE replay ({column_defs})")
    column_select: str = ", ".join(f'"{column.name}"' for column in relation.columns)
    replay_connection.execute(
        f"INSERT INTO replay SELECT {column_select} FROM read_json_auto(?)",
        [str(capture_plan.snapshot_root / relation.file_path)],
    )
    rows: list[tuple[object, ...]] = replay_connection.execute(
        "SELECT COUNT(*), MAX(decimal_scale_col), MAX(date_col), MAX(uuid_col) FROM replay"
    ).fetchall()
    assert tuple(rows) == test_case.expected_replay_summary_rows
