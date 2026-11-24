from __future__ import annotations

from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from snowflake.connector.errors import ProgrammingError

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
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeCostCollectionTestCase,
    SnowflakeCostPurposeTestCase,
)


class _RefusingCursor:
    """A role that may read neither the account nor its rate sheet: the list price is used."""

    def execute(self, sql: str) -> None:
        raise ProgrammingError(msg=f"not authorized: {sql}", errno=2003)

    def close(self) -> None:
        return None


class _CostConnection:
    def __init__(self, *, close_error: bool) -> None:
        self.close_error: bool = close_error

    def cursor(self) -> _RefusingCursor:
        return _RefusingCursor()

    def close(self) -> None:
        if self.close_error:
            raise OSError("close failed")


class _Connect:
    def __init__(self, *, connection: _CostConnection, error: bool) -> None:
        self.connection: _CostConnection = connection
        self.error: bool = error
        self.config: dict[str, object] = {}

    def __call__(self, config: dict[str, object]) -> _CostConnection:
        self.config = config
        if self.error:
            raise ConnectionError("connect failed")
        return self.connection


class _QueryHistoryCollect:
    """Runs one query-history statement, as the real collector does."""

    def __call__(self, **kwargs: Any) -> RunCostSummary:
        with StatementLifecycle(
            adapter="snowflake", sql="SELECT 1 FROM query_history", intent="query"
        ) as lifecycle:
            lifecycle.completed(row_count=0)
        return RunCostSummary(
            status=CostStatus.COMPLETE,
            usd_per_credit=kwargs["usd_per_credit"],
            rate_source=kwargs["rate_source"],
        )


class _Collect:
    def __init__(self, *, error: bool) -> None:
        self.error: bool = error

    def __call__(self, **kwargs: Any) -> RunCostSummary:
        if self.error:
            raise PermissionError("query history denied")
        return RunCostSummary(
            status=CostStatus.COMPLETE,
            usd_per_credit=kwargs["usd_per_credit"],
            rate_source=kwargs["rate_source"],
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCostCollectionTestCase(
            description="connection failure returns collection-failed status",
            connect_error=True,
            collection_error=False,
            close_error=False,
            expected_status=CostStatus.COLLECTION_FAILED,
            expected_message_fragment="failed to connect (ConnectionError)",
        ),
        SnowflakeCostCollectionTestCase(
            description="query-history failure returns collection-failed status",
            connect_error=False,
            collection_error=True,
            close_error=False,
            expected_status=CostStatus.COLLECTION_FAILED,
            expected_message_fragment="collection failed (PermissionError: query history denied)",
        ),
        SnowflakeCostCollectionTestCase(
            description="close failure downgrades successful collection to partial",
            connect_error=False,
            collection_error=False,
            close_error=True,
            expected_status=CostStatus.PARTIAL,
            expected_limitation_fragment="connection close failed (OSError)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_telemetry_stage_failure_when_collecting_then_safe_status_is_returned(
    test_case: SnowflakeCostCollectionTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: _CostConnection = _CostConnection(close_error=test_case.close_error)
    connect: _Connect = _Connect(connection=connection, error=test_case.connect_error)
    monkeypatch.setattr(
        adapter,
        "connect",
        connect,
    )
    monkeypatch.setattr(
        "sqlbuild.adapters.snowflake.classes.snowflake_adapter.collect_snowflake_cost",
        _Collect(error=test_case.collection_error),
    )
    now: datetime = datetime(2026, 8, 23, tzinfo=UTC)

    summary: RunCostSummary = adapter.collect_run_cost(
        connection_config={"login_timeout": 99, "network_timeout": 99},
        target_database="ANALYTICS",
        run_id="run-1",
        started_at=now,
        completed_at=now,
        statement_ledger_path=tmp_path / "statements.jsonl",
        usd_per_credit=Decimal("3.00"),
        rate_source="default",
    )

    assert summary.status == test_case.expected_status
    assert connect.config["login_timeout"] == 5
    assert connect.config["network_timeout"] == 15
    assert (test_case.expected_message_fragment or "") in (summary.message or "")
    assert (test_case.expected_limitation_fragment or "") in " ".join(summary.limitations)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCostPurposeTestCase(
            description="query-history statements are recorded as cost telemetry",
            expected_purposes=["cost_telemetry", "cost_telemetry"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cost_collection_when_it_queries_history_then_its_statements_name_their_purpose(
    test_case: SnowflakeCostPurposeTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    monkeypatch.setattr(
        adapter, "connect", _Connect(connection=_CostConnection(close_error=False), error=False)
    )
    monkeypatch.setattr(
        "sqlbuild.adapters.snowflake.classes.snowflake_adapter.collect_snowflake_cost",
        _QueryHistoryCollect(),
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    now: datetime = datetime(2026, 8, 23, tzinfo=UTC)

    with invocation_scope("inv-cost-purpose"), dispatcher_scope(dispatcher):
        _ = adapter.collect_run_cost(
            connection_config={},
            target_database="ANALYTICS",
            run_id="run-1",
            started_at=now,
            completed_at=now,
            statement_ledger_path=tmp_path / "statements.jsonl",
            usd_per_credit=Decimal("3.00"),
            rate_source="default",
        )

    assert [event.payload.get("purpose") for event in events] == test_case.expected_purposes
