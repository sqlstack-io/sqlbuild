from __future__ import annotations

import json
from collections.abc import Iterator
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    COST_TELEMETRY_HEALTH,
    collect_snowflake_cost,
    parse_sqlbuild_query_tag,
    render_snowflake_query_history_sql,
    snowflake_query_observations,
    snowflake_statement_is_metadata_only,
)
from sqlbuild.cost.models import RunCostSummary
from sqlbuild.cost.types import CostStatus
from tests.unit.src.sqlbuild.cost._test_types import (
    CollectSnowflakeCostTestCase,
    HistoryWindowTestCase,
    ParseQueryTagTestCase,
    RenderQueryHistorySqlTestCase,
    SnowflakeClockSkewTestCase,
    SnowflakeObservationTestCase,
    SnowflakeSaturationTestCase,
    SnowflakeStatementClassificationTestCase,
)


class _Cursor:
    def __init__(self, rows: tuple[tuple[Any, ...], ...]) -> None:
        self.rows = rows

    def fetchall(self) -> tuple[tuple[Any, ...], ...]:
        return self.rows


class _Connection:
    def __init__(self, rows: tuple[tuple[Any, ...], ...]) -> None:
        self.rows = rows
        self.sql = ""

    def execute(self, sql: str, **kwargs: Any) -> _Cursor:
        del kwargs
        assert "INFORMATION_SCHEMA.QUERY_HISTORY" in sql
        self.sql = sql
        return _Cursor(self.rows)


class _SequencedConnection:
    def __init__(self, rows_by_call: tuple[tuple[tuple[Any, ...], ...], ...]) -> None:
        self.rows_by_call = rows_by_call
        self.call_count = 0

    def execute(self, sql: str, **kwargs: Any) -> _Cursor:
        del kwargs
        assert "INFORMATION_SCHEMA.QUERY_HISTORY" in sql
        rows: tuple[tuple[Any, ...], ...] = self.rows_by_call[self.call_count]
        self.call_count += 1
        return _Cursor(rows)


@pytest.mark.parametrize(
    "test_case",
    [
        RenderQueryHistorySqlTestCase(
            description="bounded query history uses valid table function",
            expected_fragments=(
                'TABLE("ANALYTICS".INFORMATION_SCHEMA.QUERY_HISTORY(',
                "END_TIME_RANGE_START => TO_TIMESTAMP_LTZ('2026-08-23T10:00:00+00:00')",
                "END_TIME_RANGE_END => TO_TIMESTAMP_LTZ('2026-08-23T10:05:00+00:00')",
                "RESULT_LIMIT => 10000",
                "EXECUTION_STATUS",
                "QUERY_TYPE",
                "CASE WHEN (QUERY_TYPE = 'CREATE' OR QUERY_TYPE LIKE 'CREATE_%') "
                'AND QUERY_TAG LIKE \'%"run_id":"run-1"%\' '
                'AND QUERY_TAG NOT LIKE \'%"resource_type":"hook"%\' '
                "THEN QUERY_TEXT END AS QUERY_TEXT",
                "CURRENT_TIMESTAMP()",
            ),
            expected_excluded_fragment="WHERE EXECUTION_STATUS = 'SUCCESS'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_time_range_when_rendering_query_history_then_uses_valid_bounded_table_function(
    test_case: RenderQueryHistorySqlTestCase,
) -> None:
    sql: str = render_snowflake_query_history_sql(
        database="analytics",
        started_at=datetime(2026, 8, 23, 10, 0, tzinfo=UTC),
        completed_at=datetime(2026, 8, 23, 10, 5, tzinfo=UTC),
        run_id="run-1",
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql
    assert test_case.expected_excluded_fragment not in sql


@pytest.mark.parametrize(
    "test_case",
    [
        RenderQueryHistorySqlTestCase(
            description="open query history includes visible running queries",
            expected_fragments=(
                'TABLE("ANALYTICS".INFORMATION_SCHEMA.QUERY_HISTORY(',
                "END_TIME_RANGE_START => TO_TIMESTAMP_LTZ('2026-08-23T10:00:00+00:00')",
                "RESULT_LIMIT => 10000",
                "EXECUTION_STATUS",
                "QUERY_TYPE",
                "CASE WHEN (QUERY_TYPE = 'CREATE' OR QUERY_TYPE LIKE 'CREATE_%') "
                'AND QUERY_TAG LIKE \'%"run_id":"run-1"%\' '
                'AND QUERY_TAG NOT LIKE \'%"resource_type":"hook"%\' '
                "THEN QUERY_TEXT END AS QUERY_TEXT",
                "CURRENT_TIMESTAMP()",
            ),
            expected_excluded_fragment="END_TIME_RANGE_END",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_start_time_when_rendering_open_history_then_running_queries_are_requested(
    test_case: RenderQueryHistorySqlTestCase,
) -> None:
    sql: str = render_snowflake_query_history_sql(
        database="analytics",
        started_at=datetime(2026, 8, 23, 10, 0, tzinfo=UTC),
        run_id="run-1",
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql
    assert test_case.expected_excluded_fragment not in sql


@pytest.mark.parametrize(
    "test_case",
    [
        RenderQueryHistorySqlTestCase(
            description="hook and non-create query text remains masked",
            expected_fragments=("NULL AS QUERY_TEXT",),
            expected_excluded_fragment="THEN QUERY_TEXT",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_no_classifiable_create_ids_when_rendering_history_then_query_text_is_masked(
    test_case: RenderQueryHistorySqlTestCase,
) -> None:
    sql: str = render_snowflake_query_history_sql(
        started_at=datetime(2026, 8, 23, 10, 0, tzinfo=UTC),
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql
    assert test_case.expected_excluded_fragment not in sql


@pytest.mark.parametrize(
    "test_case",
    [
        RenderQueryHistorySqlTestCase(
            description="a run of only hooks requests no query text",
            expected_fragments=("NULL AS QUERY_TEXT",),
            expected_excluded_fragment="THEN QUERY_TEXT",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_exact_create_hook_when_collecting_then_hook_query_text_is_not_requested(
    test_case: RenderQueryHistorySqlTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-hook",
                "run_id": "run-hook",
                "resource_type": "hook",
                "resource_name": "pre_build",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-hook",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    connection: _Connection = _Connection(())

    collect_snowflake_cost(
        connection=connection,
        run_id="run-hook",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in connection.sql
    assert test_case.expected_excluded_fragment not in connection.sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSaturationTestCase(
            description="complete bounded fallback clears open-query saturation",
            expected_saturated=False,
            expected_observed_query_ids=frozenset({"q-bounded"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_saturated_open_history_when_bounded_fallback_is_complete_then_not_saturated(
    test_case: SnowflakeSaturationTestCase,
) -> None:
    origin: datetime = datetime.now(UTC) - timedelta(minutes=1)
    row: tuple[Any, ...] = (
        "q-open",
        "",
        "DEV_WH",
        "X-Small",
        "STANDARD",
        1,
        origin,
        origin + timedelta(seconds=1),
        1000,
        100,
        "SUCCESS",
        "SELECT",
        None,
        origin + timedelta(seconds=1),
    )
    bounded_row: tuple[Any, ...] = ("q-bounded", *row[1:])
    connection: _SequencedConnection = _SequencedConnection(
        ((row, ("q-open-2", *row[1:])), (bounded_row,))
    )

    _, observed_query_ids, _, saturated = snowflake_query_observations(
        result_limit=2,
        connection=connection,
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        expected_query_ids=frozenset({"q-bounded"}),
        classification_query_ids=frozenset(),
        deadline=float("inf"),
    )

    assert saturated is test_case.expected_saturated
    assert observed_query_ids == test_case.expected_observed_query_ids


@pytest.mark.parametrize(
    "test_case",
    [
        ParseQueryTagTestCase(
            description="sqlbuild query tag returns context",
            query_tag=json.dumps(
                {
                    "app": "sqlbuild",
                    "v": 1,
                    "run_id": "run-1",
                    "resource_type": "model",
                    "resource_name": "orders",
                }
            ),
            expected_payload={
                "run_id": "run-1",
                "resource_type": "model",
                "resource_name": "orders",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_query_tag_when_parsing_then_context_is_returned(
    test_case: ParseQueryTagTestCase,
) -> None:
    parsed: dict[str, Any] | None = parse_sqlbuild_query_tag(test_case.query_tag)

    assert parsed == test_case.expected_payload


@pytest.mark.parametrize(
    "test_case",
    [
        ParseQueryTagTestCase(
            description="invalid JSON query tag returns none",
            query_tag="not-json",
            expected_payload=None,
        ),
        ParseQueryTagTestCase(
            description="unrelated query tag returns none",
            query_tag='{"app":"other","v":1}',
            expected_payload=None,
        ),
        ParseQueryTagTestCase(
            description="missing query tag returns none",
            query_tag=None,
            expected_payload=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_or_invalid_query_tag_when_parsing_then_none_is_returned(
    test_case: ParseQueryTagTestCase,
) -> None:
    assert parse_sqlbuild_query_tag(test_case.query_tag) == test_case.expected_payload


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStatementClassificationTestCase(
            description="use statement is metadata only",
            query_type="USE",
            query_text="USE SCHEMA analytics",
            expected_metadata_only=True,
        ),
        SnowflakeStatementClassificationTestCase(
            description="drop statement is metadata only",
            query_type="DROP",
            query_text="DROP TABLE analytics.old_orders",
            expected_metadata_only=True,
        ),
        SnowflakeStatementClassificationTestCase(
            description="simple create table is metadata only",
            query_type="CREATE_TABLE",
            query_text="CREATE OR REPLACE TRANSIENT TABLE orders (id NUMBER)",
            expected_metadata_only=True,
        ),
        SnowflakeStatementClassificationTestCase(
            description="create table as select remains compute capable",
            query_type="CREATE_TABLE_AS_SELECT",
            query_text="CREATE TABLE orders AS SELECT * FROM raw_orders",
            expected_metadata_only=False,
        ),
        SnowflakeStatementClassificationTestCase(
            description="clone remains compute capable",
            query_type="CREATE_TABLE",
            query_text="CREATE TABLE orders CLONE source_orders",
            expected_metadata_only=False,
        ),
        SnowflakeStatementClassificationTestCase(
            description="dynamic table remains compute capable",
            query_type="CREATE_DYNAMIC_TABLE",
            query_text=(
                "CREATE DYNAMIC TABLE orders TARGET_LAG = DOWNSTREAM WAREHOUSE = DEV_WH "
                "AS SELECT * FROM raw_orders"
            ),
            expected_metadata_only=False,
        ),
        SnowflakeStatementClassificationTestCase(
            description="materialized view remains compute capable",
            query_type="CREATE_MATERIALIZED_VIEW",
            query_text="CREATE MATERIALIZED VIEW orders AS SELECT * FROM raw_orders",
            expected_metadata_only=False,
        ),
        SnowflakeStatementClassificationTestCase(
            description="comments and literals do not hide ctas",
            query_type="CREATE_TABLE",
            query_text=(
                "/* CREATE TABLE decoy CLONE source */ CREATE TABLE orders AS "
                "SELECT 'CLONE source' AS note"
            ),
            expected_metadata_only=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_statement_when_classifying_then_compute_capability_is_conservative(
    test_case: SnowflakeStatementClassificationTestCase,
) -> None:
    assert (
        snowflake_statement_is_metadata_only(
            query_type=test_case.query_type,
            query_text=test_case.query_text,
        )
        is test_case.expected_metadata_only
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="failed ledger query ID overrides forged query tag",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_resource_name="orders",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failed_ledger_query_and_forged_tag_when_collecting_then_exact_id_is_authoritative(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    tag: str = json.dumps(
        {
            "app": "sqlbuild",
            "v": 1,
            "run_id": "run-1",
            "resource_type": "model",
            "resource_name": "forged",
        }
    )
    rows: tuple[tuple[Any, ...], ...] = (
        (
            "q-run",
            "",
            "DEV_WH",
            "X-Small",
            "STANDARD",
            1,
            origin,
            origin + timedelta(seconds=2),
            2000,
            100,
            "FAIL_WITH_ERROR",
            "SELECT",
            "SELECT * FROM missing_table",
            origin + timedelta(seconds=2),
        ),
        (
            "q-forged",
            tag,
            "DEV_WH",
            "X-Small",
            "STANDARD",
            1,
            origin,
            origin + timedelta(seconds=2),
            2000,
            100,
            "SUCCESS",
            "SELECT",
            "SELECT 1",
            origin + timedelta(seconds=2),
        ),
    )
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-1",
                "run_id": "run-1",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-run",
                "status": "failed",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=2)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(rows),
        run_id="run-1",
        started_at=origin,
        completed_at=origin + timedelta(seconds=2),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.resources[0].resource_name == test_case.expected_resource_name
    assert summary.resources[0].estimated_compute_credits == Decimal(1) / Decimal(3600)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="ledger persistence failure prevents false complete zero",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_resource_name="",
            expected_limitation_fragment="Statement-ledger persistence failed (OSError)",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_ledger_persistence_failure_when_collecting_then_summary_is_partial(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    COST_TELEMETRY_HEALTH.mark_ledger_failure(
        run_id="ledger-failure-run", error=OSError("disk unavailable")
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(()),
        run_id="ledger-failure-run",
        started_at=origin,
        completed_at=origin,
        statement_ledger_path=tmp_path / "statements.jsonl",
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert test_case.expected_resource_name == ""
    assert any(
        test_case.expected_limitation_fragment in limitation for limitation in summary.limitations
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeObservationTestCase(
            description="missing warehouse size is explicit partial coverage",
            warehouse_name="DEV_WH",
            warehouse_size=None,
            execution_ms=1000,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
        ),
        SnowflakeObservationTestCase(
            description="zero execution on compute-capable statement is partial coverage",
            warehouse_name="DEV_WH",
            warehouse_size="X-Small",
            execution_ms=0,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
        ),
        SnowflakeObservationTestCase(
            description="missing warehouse name is explicit partial coverage",
            warehouse_name=None,
            warehouse_size="X-Small",
            execution_ms=1000,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
        ),
        SnowflakeObservationTestCase(
            description="a settled query without warehouse size used no warehouse",
            warehouse_name="DEV_WH",
            warehouse_size=None,
            execution_ms=2,
            execution_status="SUCCESS",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=0,
            observed_after_seconds=121,
        ),
        SnowflakeObservationTestCase(
            description="a settled query with zero execution used no warehouse",
            warehouse_name="DEV_WH",
            warehouse_size="X-Small",
            execution_ms=0,
            execution_status="SUCCESS",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=0,
            observed_after_seconds=121,
        ),
        SnowflakeObservationTestCase(
            description="a query read just before it settles still lacks metadata",
            warehouse_name="DEV_WH",
            warehouse_size=None,
            execution_ms=1000,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
            observed_after_seconds=119,
        ),
        SnowflakeObservationTestCase(
            description="cancelled cost-bearing statement remains attributable",
            warehouse_name="DEV_WH",
            warehouse_size="X-Small",
            execution_ms=1000,
            execution_status="CANCELED",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
        ),
        SnowflakeObservationTestCase(
            description="dynamic table with missing size is compute-capable partial coverage",
            warehouse_name="DEV_WH",
            warehouse_size=None,
            execution_ms=1000,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
            query_type="CREATE_DYNAMIC_TABLE",
            query_text=(
                "CREATE DYNAMIC TABLE orders TARGET_LAG = DOWNSTREAM WAREHOUSE = DEV_WH "
                "AS SELECT * FROM raw_orders"
            ),
        ),
        SnowflakeObservationTestCase(
            description="materialized view with missing size is compute-capable partial coverage",
            warehouse_name="DEV_WH",
            warehouse_size=None,
            execution_ms=1000,
            execution_status="SUCCESS",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_limitation_fragment="missing Snowflake warehouse metadata",
            query_type="CREATE_MATERIALIZED_VIEW",
            query_text="CREATE MATERIALIZED VIEW orders AS SELECT * FROM raw_orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_special_history_row_when_collecting_then_coverage_is_explicit(
    test_case: SnowflakeObservationTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-special",
                "run_id": "run-special",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-special",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    rows: tuple[tuple[Any, ...], ...] = (
        (
            "q-special",
            "",
            test_case.warehouse_name,
            test_case.warehouse_size,
            "STANDARD",
            1,
            origin,
            origin + timedelta(seconds=1),
            test_case.execution_ms,
            100,
            test_case.execution_status,
            test_case.query_type,
            test_case.query_text,
            origin + timedelta(seconds=1 + test_case.observed_after_seconds),
        ),
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(rows),
        run_id="run-special",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.expected_statement_count == 1
    assert summary.observed_statement_count == 1
    assert summary.skipped_statement_count == (1 - test_case.expected_query_count)
    assert test_case.expected_limitation_fragment in " ".join(summary.limitations)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="metadata-only create has complete zero cost",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=0,
            expected_resource_name="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_metadata_only_create_when_warehouse_size_is_absent_then_cost_is_complete_zero(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-create",
                "run_id": "run-create",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-create",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(milliseconds=10)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    rows: tuple[tuple[Any, ...], ...] = (
        (
            "q-create",
            "",
            "DEV_WH",
            None,
            None,
            None,
            origin,
            origin + timedelta(milliseconds=10),
            10,
            0,
            "SUCCESS",
            "CREATE_TABLE",
            "CREATE TABLE orders (id NUMBER)",
            origin + timedelta(milliseconds=10),
        ),
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(rows),
        run_id="run-create",
        started_at=origin,
        completed_at=origin + timedelta(milliseconds=10),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert test_case.expected_resource_name == ""
    assert summary.skipped_statement_count == 1


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="delayed insert metadata is left for a later build's re-read",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_resource_name="",
            expected_limitation_fragment="missing Snowflake warehouse metadata",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_compute_query_with_delayed_metadata_when_collecting_then_it_is_not_retried_at_run_end(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-insert",
                "run_id": "run-delayed",
                "resource_type": "seed",
                "resource_name": "countries",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-insert",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    incomplete: tuple[Any, ...] = (
        "q-insert",
        "",
        "DEV_WH",
        None,
        "STANDARD",
        1,
        origin,
        origin + timedelta(seconds=1),
        1000,
        100,
        "SUCCESS",
        "INSERT",
        "INSERT INTO countries SELECT 1",
        origin + timedelta(seconds=1),
    )
    complete: tuple[Any, ...] = (
        *incomplete[:3],
        "X-Small",
        *incomplete[4:],
    )
    connection: _SequencedConnection = _SequencedConnection(((incomplete,), (complete,)))

    summary: RunCostSummary = collect_snowflake_cost(
        connection=connection,
        run_id="run-delayed",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=2,
        retry_delay_seconds=0,
    )

    assert connection.call_count == 1
    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert test_case.expected_limitation_fragment in " ".join(summary.limitations)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="near-deadline retry returns current partial",
            expected_status=CostStatus.PARTIAL,
            expected_query_count=0,
            expected_resource_name="",
            expected_limitation_fragment="missing Snowflake warehouse metadata",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_retry_delay_exceeds_remaining_deadline_when_collecting_then_partial_is_returned(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-deadline",
                "run_id": "run-deadline",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-deadline",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    row: tuple[Any, ...] = (
        "q-deadline",
        "",
        "DEV_WH",
        None,
        "STANDARD",
        1,
        origin,
        origin + timedelta(seconds=1),
        1000,
        100,
        "SUCCESS",
        "INSERT",
        None,
        origin + timedelta(seconds=1),
    )
    monotonic_values: Iterator[float] = iter((0.0, 0.0, 0.0, 0.0, 29.5))
    monkeypatch.setattr(
        "time.monotonic",
        lambda: next(monotonic_values),
    )
    monkeypatch.setattr(
        "time.sleep",
        lambda seconds: pytest.fail(f"unexpected sleep for {seconds} seconds"),
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection((row,)),
        run_id="run-deadline",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=2,
        retry_delay_seconds=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert test_case.expected_resource_name == ""
    assert test_case.expected_limitation_fragment in " ".join(summary.limitations)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="running insert with empty history is retried after enrichment",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_resource_name="countries",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_running_insert_with_empty_fields_when_history_enriches_then_query_is_attributed(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-running-insert",
                "run_id": "run-running-insert",
                "resource_type": "seed",
                "resource_name": "countries",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-running-insert",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    running: tuple[Any, ...] = (
        "q-running-insert",
        "",
        None,
        None,
        None,
        None,
        origin,
        None,
        0,
        0,
        "RUNNING",
        "INSERT",
        "INSERT INTO countries SELECT 1",
        origin + timedelta(milliseconds=100),
    )
    enriched: tuple[Any, ...] = (
        "q-running-insert",
        "",
        "DEV_WH",
        "X-Small",
        "STANDARD",
        1,
        origin,
        origin + timedelta(seconds=1),
        1000,
        100,
        "SUCCESS",
        "INSERT",
        "INSERT INTO countries SELECT 1",
        origin + timedelta(seconds=1),
    )
    connection: _SequencedConnection = _SequencedConnection(((running,), (enriched,)))

    summary: RunCostSummary = collect_snowflake_cost(
        connection=connection,
        run_id="run-running-insert",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=2,
        retry_delay_seconds=0,
    )

    assert connection.call_count == 2
    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.resources[0].resource_name == test_case.expected_resource_name


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeClockSkewTestCase(
            description="Snowflake clock is ahead of host",
            snowflake_offset_seconds=300,
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_attributed_seconds="1.0",
        ),
        SnowflakeClockSkewTestCase(
            description="Snowflake clock is behind host",
            snowflake_offset_seconds=-300,
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_attributed_seconds="1.0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_host_clock_skew_when_collecting_then_snowflake_execution_interval_is_attributed(
    test_case: SnowflakeClockSkewTestCase,
    tmp_path: Path,
) -> None:
    host_origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    snowflake_origin: datetime = host_origin + timedelta(seconds=test_case.snowflake_offset_seconds)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-skew",
                "run_id": "run-skew",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-skew",
                "status": "success",
                "started_at": host_origin.isoformat(),
                "completed_at": (host_origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    rows: tuple[tuple[Any, ...], ...] = (
        (
            "q-skew",
            "",
            "DEV_WH",
            "X-Small",
            "STANDARD",
            1,
            snowflake_origin,
            snowflake_origin + timedelta(seconds=1),
            1000,
            100,
            "SUCCESS",
            "INSERT",
            "INSERT INTO orders SELECT 1",
            snowflake_origin + timedelta(seconds=1),
        ),
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(rows),
        run_id="run-skew",
        started_at=host_origin,
        completed_at=host_origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.attributed_seconds == Decimal(test_case.expected_attributed_seconds)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="visible running competitor shares overlapping warehouse busy time",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_resource_name="orders",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_visible_running_competitor_when_collecting_then_busy_time_is_shared(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime(2026, 8, 23, 10, tzinfo=UTC)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-run",
                "run_id": "run-shared",
                "resource_type": "model",
                "resource_name": test_case.expected_resource_name,
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-run",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    rows: tuple[tuple[Any, ...], ...] = (
        (
            "q-run",
            "",
            "DEV_WH",
            "X-Small",
            "STANDARD",
            1,
            origin,
            origin + timedelta(seconds=1),
            1000,
            100,
            "SUCCESS",
            "INSERT",
            "INSERT INTO orders SELECT 1",
            origin + timedelta(seconds=1),
        ),
        (
            "q-running-competitor",
            "",
            "DEV_WH",
            "X-Small",
            "STANDARD",
            1,
            origin,
            None,
            1000,
            0,
            "RUNNING",
            "SELECT",
            "SELECT * FROM competitor",
            origin + timedelta(seconds=1),
        ),
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(rows),
        run_id="run-shared",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.resources[0].resource_name == test_case.expected_resource_name
    assert summary.resources[0].attributed_seconds == Decimal("0.5")


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="bounded fallback retains open running competitor share",
            expected_status=CostStatus.COMPLETE,
            expected_query_count=1,
            expected_resource_name="orders",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_saturated_open_history_when_bounded_fallback_completes_then_running_share_is_retained(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    origin: datetime = datetime.now(UTC) - timedelta(minutes=1)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-saturated-share",
                "run_id": "run-saturated-share",
                "resource_type": "model",
                "resource_name": test_case.expected_resource_name,
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-run",
                "status": "success",
                "started_at": origin.isoformat(),
                "completed_at": (origin + timedelta(seconds=1)).isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    completed: tuple[Any, ...] = (
        "q-run",
        "",
        "DEV_WH",
        "X-Small",
        "STANDARD",
        1,
        origin,
        origin + timedelta(seconds=1),
        1000,
        100,
        "SUCCESS",
        "INSERT",
        None,
        origin + timedelta(seconds=1),
    )
    running_competitor: tuple[Any, ...] = (
        "q-running-competitor",
        "",
        "DEV_WH",
        "X-Small",
        "STANDARD",
        1,
        origin,
        None,
        1000,
        0,
        "RUNNING",
        "SELECT",
        None,
        origin + timedelta(seconds=1),
    )
    connection: _SequencedConnection = _SequencedConnection(
        ((completed, running_competitor), (completed,))
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=connection,
        result_limit=2,
        run_id="run-saturated-share",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert connection.call_count == 2
    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.resources[0].resource_name == test_case.expected_resource_name
    assert summary.resources[0].attributed_seconds == Decimal("0.5")


@pytest.mark.parametrize(
    "test_case",
    [
        CollectSnowflakeCostTestCase(
            description="aged out missing history becomes unavailable",
            expected_status=CostStatus.UNAVAILABLE,
            expected_query_count=0,
            expected_resource_name="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_run_older_than_retention_when_history_is_missing_then_status_is_unavailable(
    test_case: CollectSnowflakeCostTestCase,
    tmp_path: Path,
) -> None:
    completed_at: datetime = datetime.now(UTC) - timedelta(days=8)
    started_at: datetime = completed_at - timedelta(seconds=1)
    ledger_path: Path = tmp_path / "statements.jsonl"
    ledger_path.write_text(
        json.dumps(
            {
                "statement_id": "statement-expired",
                "run_id": "run-expired",
                "resource_type": "model",
                "resource_name": "orders",
                "phase": "execute",
                "attempt": 1,
                "query_id": "q-expired",
                "status": "success",
                "started_at": started_at.isoformat(),
                "completed_at": completed_at.isoformat(),
            }
        )
        + "\n",
        encoding="utf-8",
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=_Connection(()),
        run_id="run-expired",
        started_at=started_at,
        completed_at=completed_at,
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert "no longer available" in (summary.message or "")


@pytest.mark.parametrize(
    "test_case",
    [
        HistoryWindowTestCase(
            description="a run that just finished reads open-ended history for running queries",
            completed_minutes_ago=0,
            expected_bounded=False,
        ),
        HistoryWindowTestCase(
            description="an older run reads only its own window, not the hours since",
            completed_minutes_ago=30,
            expected_bounded=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_run_age_when_reading_history_then_older_runs_read_a_bounded_window(
    test_case: HistoryWindowTestCase,
) -> None:
    completed_at: datetime = datetime.now(UTC) - timedelta(minutes=test_case.completed_minutes_ago)
    connection: _Connection = _Connection(())

    snowflake_query_observations(
        connection=connection,
        started_at=completed_at - timedelta(minutes=1),
        completed_at=completed_at,
        expected_query_ids=frozenset(),
        classification_query_ids=frozenset(),
        deadline=float("inf"),
    )

    assert ("END_TIME_RANGE_END" in connection.sql) is test_case.expected_bounded
