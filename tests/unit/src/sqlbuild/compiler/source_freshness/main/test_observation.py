from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

import pytest

from sqlbuild._native import (
    Dialect,
    SourceEntry,
    SourceFreshnessConfig,
    SourceFreshnessObservation,
    SourceFreshnessStrategy,
    SourceFreshnessValueKind,
    live_reads,
    observe_configured_source_freshness,
)
from sqlbuild.adapter.contract.models import QueryResult, TableFreshnessMetadata
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.compiler.source_freshness.main._test_types import (
    SharedSourceFreshnessColumnSqlTestCase,
    SharedSourceFreshnessExpressionSubqueryTestCase,
    SharedSourceFreshnessObservationTestCase,
)


class CapturingFreshnessMetadataDuckDbAdapter(DuckDbAdapter):
    def __init__(self, *, metadata_observed_at: datetime | None) -> None:
        super().__init__()
        self.metadata_observed_at: datetime | None = metadata_observed_at

    def supports_table_freshness_metadata(self) -> bool:
        return True

    def get_table_freshness_metadata(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        return TableFreshnessMetadata(
            data_version=datetime(2026, 1, 15, 12, 0, 0),
            value_kind="timestamp",
            observed_at=self.metadata_observed_at,
        )


class CapturingQueryDuckDbAdapter(DuckDbAdapter):
    captured_sql: str | None

    def __init__(self) -> None:
        super().__init__()
        self.captured_sql = None
        self.calls: list[str] = []

    def query(self, connection: Any, sql: str, *, limit: int | None = 1000) -> QueryResult:
        self.calls.append("query")
        self.captured_sql = sql
        return QueryResult(columns=("data_version",), rows=((1,),))


@pytest.mark.parametrize(
    "test_case",
    [
        SharedSourceFreshnessObservationTestCase(
            description="uses adapter metadata observed_at when present",
            adapter_observed_at=datetime(2026, 1, 15, 12, 1, 0, tzinfo=UTC),
            fallback_observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            expected_observed_at=datetime(2026, 1, 15, 12, 1, 0, tzinfo=UTC),
        ),
        SharedSourceFreshnessObservationTestCase(
            description="falls back to caller observed_at when adapter omits observed_at",
            adapter_observed_at=None,
            fallback_observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            expected_observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_freshness_metadata_when_observing_then_uses_expected_observed_at(
    test_case: SharedSourceFreshnessObservationTestCase,
) -> None:
    adapter: CapturingFreshnessMetadataDuckDbAdapter = CapturingFreshnessMetadataDuckDbAdapter(
        metadata_observed_at=test_case.adapter_observed_at
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        observation: SourceFreshnessObservation = observe_configured_source_freshness(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            source=SourceEntry(
                name="raw.orders",
                table="orders",
                freshness=SourceFreshnessConfig(strategy=SourceFreshnessStrategy.ADAPTER),
            ),
            observed_at=test_case.fallback_observed_at,
        )
    finally:
        adapter.close(connection)

    assert observation.observed_at == test_case.expected_observed_at
    assert observation.data_version == datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC)


@pytest.mark.parametrize(
    "test_case",
    [
        SharedSourceFreshnessColumnSqlTestCase(
            description="renders database schema table and filter for column freshness",
            source_database="warehouse",
            source_schema="raw",
            source_table="orders",
            expected_sql_fragment="FROM warehouse.raw.orders",
            freshness_filter="updated_at >= current_date - interval '2 days'",
            expected_filter_fragment="WHERE updated_at >= current_date - interval '2 days'",
        ),
        SharedSourceFreshnessColumnSqlTestCase(
            description="omits where clause when column freshness has no filter",
            source_database="warehouse",
            source_schema="raw",
            source_table="orders",
            expected_sql_fragment="FROM warehouse.raw.orders",
            unexpected_sql_fragment=" WHERE ",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_column_freshness_source_when_observing_then_renders_full_relation(
    test_case: SharedSourceFreshnessColumnSqlTestCase,
) -> None:
    adapter: CapturingQueryDuckDbAdapter = CapturingQueryDuckDbAdapter()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()

    def record_event(event: LifecycleEvent) -> None:
        events.append(event)
        adapter.calls.append(event.event_type)

    dispatcher.subscribe_lifecycle(subscriber=record_event, accepts_opaque=False)
    with invocation_scope("freshness-invocation"), dispatcher_scope(dispatcher):
        observe_configured_source_freshness(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=object()),
            source=SourceEntry(
                name="raw.orders",
                database=test_case.source_database,
                schema=test_case.source_schema,
                table=test_case.source_table,
                freshness=SourceFreshnessConfig(
                    strategy=SourceFreshnessStrategy.COLUMN,
                    value_kind=SourceFreshnessValueKind.INTEGER,
                    column="batch_id",
                    filter=test_case.freshness_filter,
                ),
            ),
            observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
        )

    assert adapter.captured_sql is not None
    assert test_case.expected_sql_fragment in adapter.captured_sql
    assert (test_case.expected_filter_fragment or "") in adapter.captured_sql
    assert (test_case.unexpected_sql_fragment or "__not_present__") not in adapter.captured_sql
    assert adapter.calls == ["operation_started", "query", "operation_completed"]
    assert events[0].payload["operation_name"] == "source_freshness_query_observation"
    assert "raw.orders" not in str(events)
    assert adapter.captured_sql not in str(events)


@pytest.mark.parametrize(
    "test_case",
    [
        SharedSourceFreshnessExpressionSubqueryTestCase(
            description="column freshness wraps an expression source as a subquery",
            expression="SELECT 1 AS id, 2 AS batch_id",
            column="batch_id",
            expected_sql_fragment="FROM (SELECT 1 AS id, 2 AS batch_id)",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_column_freshness_expression_source_when_observing_then_uses_subquery(
    test_case: SharedSourceFreshnessExpressionSubqueryTestCase,
) -> None:
    adapter: CapturingQueryDuckDbAdapter = CapturingQueryDuckDbAdapter()

    observe_configured_source_freshness(
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=object()),
        source=SourceEntry(
            name="raw_orders",
            expression=test_case.expression,
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy.COLUMN,
                value_kind=SourceFreshnessValueKind.INTEGER,
                column=test_case.column,
            ),
        ),
        observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
    )

    assert adapter.captured_sql is not None
    assert test_case.expected_sql_fragment in adapter.captured_sql
    assert f'MAX("{test_case.column}")' in adapter.captured_sql
