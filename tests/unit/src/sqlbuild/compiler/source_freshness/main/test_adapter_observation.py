from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

import pytest

from sqlbuild._native import (
    AdapterSourceFreshnessBatch,
    Dialect,
    DirectSourceFreshnessPlanningResult,
    SourceFreshnessConfig,
    SourceFreshnessStrategy,
    SourceFreshnessUnknownReason,
    build_direct_source_freshness_planning_result,
    live_reads,
    observe_adapter_sources_freshness,
)
from sqlbuild.adapter.contract.models import TableFreshnessMetadata
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.compiler.source_freshness.main._test_types import (
    AdapterBatchIsolationTestCase,
)
from tests.unit.src.sqlbuild.compiler.source_freshness.main.helpers import (
    PerTableFreshnessDuckDbAdapter,
    observed_table_freshness,
    physical_table_sources,
)

_OBSERVED_AT: datetime = datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC)


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterBatchIsolationTestCase(
            description="absent table metadata leaves other sources observed",
            outcomes={
                "orders": observed_table_freshness(1),
                "customers": observed_table_freshness(2),
                "payments": TableFreshnessMetadata.unavailable(
                    message="table freshness metadata not found for payments"
                ),
            },
            expected_observed_names=("customers", "orders"),
            expected_unknown_reasons={"payments": SourceFreshnessUnknownReason.UNAVAILABLE},
        ),
        AdapterBatchIsolationTestCase(
            description="null data version is unknown for that source only",
            outcomes={
                "orders": observed_table_freshness(1),
                "customers": TableFreshnessMetadata(data_version=None, value_kind="integer"),
                "payments": observed_table_freshness(3),
            },
            expected_observed_names=("orders", "payments"),
            expected_unknown_reasons={"customers": SourceFreshnessUnknownReason.UNAVAILABLE},
        ),
        AdapterBatchIsolationTestCase(
            description="unavailable metadata and unreturned table are isolated",
            outcomes={
                "orders": TableFreshnessMetadata.unavailable(message="found VIEW for orders"),
                "customers": observed_table_freshness(2),
            },
            expected_observed_names=("customers",),
            expected_unknown_reasons={
                "orders": SourceFreshnessUnknownReason.UNAVAILABLE,
                "payments": SourceFreshnessUnknownReason.UNAVAILABLE,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_batch_with_unobservable_table_when_observing_then_isolates_each_source(
    test_case: AdapterBatchIsolationTestCase,
) -> None:
    adapter: PerTableFreshnessDuckDbAdapter = PerTableFreshnessDuckDbAdapter(
        outcomes=test_case.outcomes
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("freshness-invocation"), dispatcher_scope(dispatcher):
        batch: AdapterSourceFreshnessBatch = observe_adapter_sources_freshness(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=object()),
            sources=physical_table_sources(
                freshness=SourceFreshnessConfig(strategy=SourceFreshnessStrategy.ADAPTER)
            ),
            observed_at=_OBSERVED_AT,
        )

    assert tuple(sorted(batch.observations)) == test_case.expected_observed_names
    assert {
        name: unknown.reason for name, unknown in batch.unknown.items()
    } == test_case.expected_unknown_reasons
    assert len(adapter.batch_requests) == 1
    assert events[-1].event_type == "operation_completed"
    assert events[-1].payload["metadata"] == {
        "item_count": 3,
        "unknown_count": len(test_case.expected_unknown_reasons),
    }


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterBatchIsolationTestCase(
            description="auto-observed sources keep freshness beside absent table metadata",
            outcomes={
                "orders": observed_table_freshness(1),
                "customers": observed_table_freshness(2),
                "payments": TableFreshnessMetadata.unavailable(
                    message="table freshness metadata not found for payments"
                ),
            },
            expected_observed_names=("customers", "orders"),
            expected_unknown_reasons={"payments": SourceFreshnessUnknownReason.UNAVAILABLE},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_batch_with_absent_table_metadata_when_planning_then_records_others_and_reasons(
    test_case: AdapterBatchIsolationTestCase,
) -> None:
    adapter: PerTableFreshnessDuckDbAdapter = PerTableFreshnessDuckDbAdapter(
        outcomes=test_case.outcomes
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=physical_table_sources(freshness=None),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=_OBSERVED_AT,
            run_id="planning",
            render_qualified_name=adapter.render_qualified_name,
            state_table_exists_by_schema={"state_schema": False},
        )
    finally:
        adapter.close(connection)

    assert (
        tuple(sorted(record.source_name for record in result.observed_records))
        == test_case.expected_observed_names
    )
    assert result.unknown_source_names == tuple(sorted(test_case.expected_unknown_reasons))
    assert {
        name: unknown.reason for name, unknown in result.unknown_sources.items()
    } == test_case.expected_unknown_reasons
    assert len(result.changed_identities) == len(test_case.expected_observed_names)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
