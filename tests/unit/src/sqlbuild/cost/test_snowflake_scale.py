"""Snowflake cost collection over runs with tens of thousands of statements."""

from __future__ import annotations

import re
from dataclasses import dataclass, field, replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from snowflake.connector.errors import ProgrammingError as SnowflakeProgrammingError

from sqlbuild._native import (
    CostCommandRequest,
    CostRunRecord,
    collect_snowflake_cost,
    read_cost_run,
    refresh_pending_cost_run,
    write_cost_run,
)
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.cost.models import RunCostSummary
from sqlbuild.cost.types import CostStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from tests.unit.src.sqlbuild.cli.commands._helpers.cost.helpers import write_snowflake_project
from tests.unit.src.sqlbuild.cost._test_types import (
    SnowflakeCostBudgetTestCase,
    SnowflakeCostFailureMessageTestCase,
    SnowflakeLargeRunCostTestCase,
    SnowflakeScaleTestCase,
    SnowflakeTimeoutOutcomeTestCase,
)
from tests.unit.src.sqlbuild.cost.helpers import (
    build_cost_run_record,
    history_row,
    run_history_rows,
    write_ledger,
)

_ORIGIN: datetime = datetime(2026, 8, 23, 1, tzinfo=UTC)
_RUN_ID: str = "run-nightly"


_START_PATTERN: re.Pattern[str] = re.compile(
    r"END_TIME_RANGE_START => TO_TIMESTAMP_LTZ\('([^']+)'\)"
)
_END_PATTERN: re.Pattern[str] = re.compile(r"END_TIME_RANGE_END => TO_TIMESTAMP_LTZ\('([^']+)'\)")
_LIMIT_PATTERN: re.Pattern[str] = re.compile(r"RESULT_LIMIT => (\d+)")
STATEMENT_TIMEOUT_ERRNO: int = 630


class StatementTimeoutError(Exception):
    """The error a Snowflake read raises when it reaches its statement timeout."""

    def __init__(self, *, seconds: str) -> None:
        super().__init__(
            f"000630 (57014): Statement reached its statement or warehouse timeout of "
            f"{seconds} second(s) and was canceled."
        )
        self.errno: int = STATEMENT_TIMEOUT_ERRNO


@dataclass
class HistoryRead:
    """One query-history read: its window, SQL length and statement timeout."""

    start: datetime
    end: datetime | None
    sql_length: int
    timeout_seconds: str


class _Cursor:
    def __init__(self, rows: list[tuple[Any, ...]]) -> None:
        self.rows: list[tuple[Any, ...]] = rows

    def fetchall(self) -> list[tuple[Any, ...]]:
        return self.rows


@dataclass
class _Clock:
    """A monotonic clock that advances only when the fake history answers a read."""

    seconds: float = 0.0

    def monotonic(self) -> float:
        return self.seconds


@dataclass
class _FakeQueryHistory:
    """Answers `INFORMATION_SCHEMA.QUERY_HISTORY` reads from fixed rows, like Snowflake does.

    Rows are filtered by end time, the open-ended read also returns running queries, and at most
    `RESULT_LIMIT` rows are returned. A read wider than `timeout_window` raises Snowflake's
    statement-timeout error, standing in for a read too slow for its statement timeout.
    """

    rows: list[tuple[Any, ...]]
    timeout_window: timedelta | None = None
    reads: list[HistoryRead] = field(default_factory=list)
    clock: _Clock = field(default_factory=_Clock)
    seconds_per_read: float = 0.0

    @property
    def longest_sql(self) -> int:
        """The length of the longest SQL statement read."""

        return max(read.sql_length for read in self.reads)

    def execute(self, sql: str, **kwargs: Any) -> _Cursor:
        assert "INFORMATION_SCHEMA.QUERY_HISTORY" in sql
        start_match: re.Match[str] | None = _START_PATTERN.search(sql)
        limit_match: re.Match[str] | None = _LIMIT_PATTERN.search(sql)
        assert start_match is not None and limit_match is not None
        end_match: re.Match[str] | None = _END_PATTERN.search(sql)
        start: datetime = datetime.fromisoformat(start_match.group(1))
        end: datetime | None = (
            datetime.fromisoformat(end_match.group(1)) if end_match is not None else None
        )
        timeout_seconds: str = kwargs["statement_params"]["STATEMENT_TIMEOUT_IN_SECONDS"]
        self.clock.seconds += self.seconds_per_read
        self.reads.append(
            HistoryRead(start=start, end=end, sql_length=len(sql), timeout_seconds=timeout_seconds)
        )
        latest_end: datetime = end if end is not None else max(row[7] or start for row in self.rows)
        if self.timeout_window is not None and latest_end - start > self.timeout_window:
            raise StatementTimeoutError(seconds=timeout_seconds)
        matched: list[tuple[Any, ...]] = [
            row
            for row in self.rows
            if (row[7] is None and end is None)
            or (row[7] is not None and row[7] >= start and (end is None or row[7] < end))
        ]
        return _Cursor(matched[: int(limit_match.group(1))])


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeScaleTestCase(
            description="an unsaturated 8-hour run is read with one query",
            statement_count=5_000,
            duration_minutes=480,
            timeout_window=None,
            expected_status=CostStatus.COMPLETE,
            expected_query_count=5_000,
            expected_minimum_reads=1,
            expected_maximum_reads=1,
        ),
        SnowflakeScaleTestCase(
            description="a 78-minute run of 20,000 statements splits its saturated window",
            statement_count=20_000,
            duration_minutes=78,
            timeout_window=None,
            expected_status=CostStatus.COMPLETE,
            expected_query_count=20_000,
        ),
        SnowflakeScaleTestCase(
            description="a window that reaches the statement timeout is split and retried",
            statement_count=6_500,
            duration_minutes=42,
            timeout_window=timedelta(minutes=8),
            expected_status=CostStatus.COMPLETE,
            expected_query_count=6_500,
        ),
        SnowflakeScaleTestCase(
            description="history that times out at every width is reported pending with its cause",
            statement_count=6_500,
            duration_minutes=42,
            timeout_window=timedelta(0),
            expected_status=CostStatus.PENDING,
            expected_query_count=0,
            expected_message_fragments=(
                "10-second statement timeout",
                f"Run `sqb cost {_RUN_ID}` to retry.",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_large_run_when_collecting_snowflake_cost_then_sql_stays_bounded(
    test_case: SnowflakeScaleTestCase,
    tmp_path: Path,
) -> None:
    duration: timedelta = timedelta(minutes=test_case.duration_minutes)
    rows: list[tuple[Any, ...]] = run_history_rows(
        origin=_ORIGIN, statement_count=test_case.statement_count, duration=duration
    )
    ledger_path: Path = tmp_path / "statements.jsonl"
    write_ledger(path=ledger_path, run_id=_RUN_ID, rows=rows)
    history: _FakeQueryHistory = _FakeQueryHistory(
        rows=rows, timeout_window=test_case.timeout_window
    )

    summary: RunCostSummary = collect_snowflake_cost(
        connection=history,
        run_id=_RUN_ID,
        started_at=_ORIGIN,
        completed_at=_ORIGIN + duration,
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
        attempts=1,
    )

    assert summary.status == test_case.expected_status
    assert summary.query_count == test_case.expected_query_count
    assert summary.expected_statement_count == test_case.statement_count
    for fragment in test_case.expected_message_fragments:
        assert fragment in (summary.message or "")
    assert history.longest_sql < test_case.expected_max_sql_length
    assert {read.timeout_seconds for read in history.reads} == {"10"}
    assert test_case.expected_minimum_reads <= len(history.reads)
    assert len(history.reads) <= test_case.expected_maximum_reads


class _ObservedHistory(_FakeQueryHistory):
    """Query history read through statement lifecycles, as the Snowflake connection records them."""

    def execute(self, sql: str, **kwargs: Any) -> _Cursor:
        with StatementLifecycle(adapter="snowflake", sql=sql, intent="query") as lifecycle:
            try:
                cursor: _Cursor = super().execute(sql, **kwargs)
            except Exception as error:
                lifecycle.failed(error=error)
                raise
            lifecycle.completed(row_count=len(cursor.rows))
            return cursor


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeTimeoutOutcomeTestCase(
            description="a timed-out read retried in smaller windows is handled",
            timeout_window=timedelta(minutes=8),
            expected_failure_outcomes={"handled"},
        ),
        SnowflakeTimeoutOutcomeTestCase(
            description="a timed-out read too narrow to split stays failed",
            timeout_window=timedelta(0),
            expected_failure_outcomes={"handled", None},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_history_read_timeout_when_collecting_then_only_retried_reads_are_handled(
    test_case: SnowflakeTimeoutOutcomeTestCase, tmp_path: Path
) -> None:
    duration: timedelta = timedelta(minutes=42)
    rows: list[tuple[Any, ...]] = run_history_rows(
        origin=_ORIGIN, statement_count=6_500, duration=duration
    )
    ledger_path: Path = tmp_path / "statements.jsonl"
    write_ledger(path=ledger_path, run_id=_RUN_ID, rows=rows)
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-history-timeout"), dispatcher_scope(dispatcher):
        _ = collect_snowflake_cost(
            connection=_ObservedHistory(rows=rows, timeout_window=test_case.timeout_window),
            run_id=_RUN_ID,
            started_at=_ORIGIN,
            completed_at=_ORIGIN + duration,
            statement_ledger_path=ledger_path,
            usd_per_credit=Decimal("3.00"),
            rate_source="default",
            attempts=1,
        )

    failures: list[LifecycleEvent] = list(
        filter(lambda event: event.event_type == "statement_failed", events)
    )
    assert {event.payload.get("outcome") for event in failures} == (
        test_case.expected_failure_outcomes
    )


class _RefusingCursor:
    """A role that may read neither the account nor its rate sheet: the list price is used."""

    def execute(self, sql: str) -> None:
        raise SnowflakeProgrammingError(msg=f"not authorized: {sql}", errno=2003)

    def close(self) -> None:
        return None


class _ClosingHistory(_FakeQueryHistory):
    def close(self) -> None:
        return None

    def cursor(self) -> _RefusingCursor:
        return _RefusingCursor()


class ProgrammingError(Exception):
    """Stands in for the Snowflake connector's error of the same name."""


class _FailingHistory(_ClosingHistory):
    def execute(self, sql: str, **kwargs: Any) -> Any:
        del sql, kwargs
        raise ProgrammingError(
            "002003 (02000): SQL compilation error:\nDatabase 'ANALYTICS' does not exist."
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeLargeRunCostTestCase(
            description="a large run's history is collected completely",
            timeout_window=None,
            expected_status=CostStatus.COMPLETE,
            expected_message_fragment="",
        ),
        SnowflakeLargeRunCostTestCase(
            description="history too slow to read leaves the run pending, naming the timeout",
            timeout_window=timedelta(0),
            expected_status=CostStatus.PENDING,
            expected_message_fragment="statement timeout",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_large_run_when_adapter_collects_cost_then_failures_stay_non_fatal(
    test_case: SnowflakeLargeRunCostTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    origin: datetime = datetime(2026, 8, 23, 1, tzinfo=UTC)
    rows: list[tuple[Any, ...]] = [
        history_row(query_id=f"q-{index:05d}", ended_at=origin + timedelta(seconds=index // 3))
        for index in range(12_000)
    ]
    ledger_path: Path = tmp_path / "statements.jsonl"
    write_ledger(path=ledger_path, run_id="run-large", rows=rows)
    history: _ClosingHistory = _ClosingHistory(rows=rows, timeout_window=test_case.timeout_window)
    monkeypatch.setattr(adapter, "connect", lambda config: history)

    summary: RunCostSummary = adapter.collect_run_cost(
        connection_config={},
        target_database="ANALYTICS",
        run_id="run-large",
        started_at=origin,
        completed_at=origin + timedelta(seconds=4_000),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
    )

    assert summary.status == test_case.expected_status
    assert test_case.expected_message_fragment in (summary.message or "")
    assert history.longest_sql < test_case.expected_max_sql_length


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCostFailureMessageTestCase(
            description="a multi-line Snowflake error is named on one line",
            expected_status=CostStatus.COLLECTION_FAILED,
            expected_message=(
                "Snowflake cost collection failed (ProgrammingError: 002003 (02000): "
                "SQL compilation error: Database 'ANALYTICS' does not exist.)."
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_query_history_error_when_adapter_collects_cost_then_message_names_cause(
    test_case: SnowflakeCostFailureMessageTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    origin: datetime = datetime(2026, 8, 23, 1, tzinfo=UTC)
    rows: list[tuple[Any, ...]] = [history_row(query_id="q-1", ended_at=origin)]
    ledger_path: Path = tmp_path / "statements.jsonl"
    write_ledger(path=ledger_path, run_id="run-failed", rows=rows)
    monkeypatch.setattr(adapter, "connect", lambda config: _FailingHistory(rows=rows))

    summary: RunCostSummary = adapter.collect_run_cost(
        connection_config={},
        target_database="ANALYTICS",
        run_id="run-failed",
        started_at=origin,
        completed_at=origin + timedelta(seconds=1),
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
    )

    assert summary.status == test_case.expected_status
    assert summary.message == test_case.expected_message


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCostBudgetTestCase(
            description="the at-exit budget leaves a slow eight-hour run pending",
            seconds_per_read=1.0,
            expected_status=CostStatus.PENDING,
            expected_message_fragment="30-second collection deadline exceeded",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slow_history_when_collecting_at_exit_then_run_stays_pending(
    test_case: SnowflakeCostBudgetTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    duration: timedelta = timedelta(hours=8)
    rows: list[tuple[Any, ...]] = run_history_rows(
        origin=_ORIGIN, statement_count=2_000, duration=duration
    )
    ledger_path: Path = tmp_path / "statements.jsonl"
    write_ledger(path=ledger_path, run_id=_RUN_ID, rows=rows)
    history: _ClosingHistory = _ClosingHistory(
        rows=rows,
        timeout_window=timedelta(minutes=10),
        seconds_per_read=test_case.seconds_per_read,
    )
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    monkeypatch.setattr(adapter, "connect", lambda config: history)
    monkeypatch.setattr("time.monotonic", history.clock.monotonic)

    summary: RunCostSummary = adapter.collect_run_cost(
        connection_config={},
        target_database="ANALYTICS",
        run_id=_RUN_ID,
        started_at=_ORIGIN,
        completed_at=_ORIGIN + duration,
        statement_ledger_path=ledger_path,
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
    )

    assert summary.status == test_case.expected_status
    assert test_case.expected_message_fragment in (summary.message or "")


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCostBudgetTestCase(
            description="sqb cost completes a slow eight-hour run left pending at exit",
            seconds_per_read=1.0,
            expected_status=CostStatus.COMPLETE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_pending_slow_run_when_refreshing_with_sqb_cost_then_larger_budget_completes_it(
    test_case: SnowflakeCostBudgetTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    duration: timedelta = timedelta(hours=8)
    rows: list[tuple[Any, ...]] = run_history_rows(
        origin=_ORIGIN, statement_count=2_000, duration=duration
    )
    ledger_dir: Path = tmp_path / "target" / "executions" / _RUN_ID
    ledger_dir.mkdir(parents=True)
    write_ledger(path=ledger_dir / "statements.jsonl", run_id=_RUN_ID, rows=rows)
    record: CostRunRecord = build_cost_run_record(run_id=_RUN_ID, completed_at=_ORIGIN + duration)
    write_cost_run(
        project_dir=tmp_path,
        record=replace(
            record,
            started_at=_ORIGIN,
            cost=replace(record.cost, status=CostStatus.PENDING, resources=(), query_count=0),
        ),
    )
    write_snowflake_project(tmp_path)
    history: _ClosingHistory = _ClosingHistory(
        rows=rows,
        timeout_window=timedelta(minutes=10),
        seconds_per_read=test_case.seconds_per_read,
    )
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    monkeypatch.setattr(adapter, "connect", lambda config: history)
    monkeypatch.setattr("time.monotonic", history.clock.monotonic)
    monkeypatch.setattr(
        "sqlbuild.adapter.discovery.main.resolve_adapter.resolve_adapter",
        lambda **kwargs: adapter,
    )

    refresh_pending_cost_run(
        CostCommandRequest(
            project_dir=tmp_path,
            selector=_RUN_ID,
            no_color=True,
            limit=None,
            no_limit=False,
            sort=None,
            order=None,
            since=None,
            until=None,
            json_output=False,
            json_output_path=None,
        )
    )

    refreshed: CostRunRecord | None = read_cost_run(project_dir=tmp_path, run_id=_RUN_ID)
    assert refreshed is not None
    assert refreshed.cost.status == test_case.expected_status
    assert refreshed.cost.query_count == len(rows)
    assert history.clock.seconds > 30
