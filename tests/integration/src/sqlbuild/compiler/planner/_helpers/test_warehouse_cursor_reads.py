from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    CursorSnapshotScope,
    Dialect,
    ModelCursorSnapshot,
    PlannerInputError,
    WarehouseSnapshot,
    gather_warehouse_snapshot,
    live_reads,
)
from sqlbuild.adapter.relations.main.open_inspection_catalog import open_inspection_catalog
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.planner._helpers._test_types import (
    ConcurrentCursorReadsTestCase,
    EligibleMaxFailureTestCase,
    FailedCursorReadTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner._helpers.helpers import (
    ConcurrentDuckDbAdapter,
    TrackingCursorExecute,
    build_fanned_cursor_project,
    create_events,
    cursor_progress,
    gather_with_catalog,
    unreadable_inputs,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentCursorReadsTestCase(
            description="batched reads run concurrently within the bound",
            relation_count=120,
            concurrency=4,
            failing_relations=frozenset(),
            expected_start_progress=(
                "Inspecting cursor bounds for 120 relations in 3 statements (3 concurrent)..."
            ),
            expected_unreadable={},
            expected_statements=3,
            expected_max_active=3,
        ),
        ConcurrentCursorReadsTestCase(
            description="one failed relation stays unavailable and others succeed",
            relation_count=8,
            concurrency=4,
            failing_relations=frozenset({"staging.events_5"}),
            expected_start_progress=(
                "Inspecting cursor bounds for 8 relations in 1 statement (1 concurrent)..."
            ),
            expected_unreadable={"daily_5": ("events_5.event_time",)},
            expected_statements=9,
            expected_max_active=4,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_catalog_when_reading_cursor_bounds_then_matches_sequential_reads(
    test_case: ConcurrentCursorReadsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    create_events(connection, test_case.relation_count)
    project: CompiledProjectView = build_fanned_cursor_project(test_case.relation_count)
    sequential_execute: TrackingCursorExecute = TrackingCursorExecute(
        failing_relations=test_case.failing_relations
    )
    parallel_execute: TrackingCursorExecute = TrackingCursorExecute(
        failing_relations=test_case.failing_relations
    )
    sequential_progress: list[str] = []
    parallel_progress: list[str] = []

    sequential: WarehouseSnapshot = gather_with_catalog(
        project=project,
        adapter=adapter,
        connection=connection,
        execute=sequential_execute,
        progress=sequential_progress,
    )
    parallel: WarehouseSnapshot = gather_with_catalog(
        project=project,
        adapter=ConcurrentDuckDbAdapter(),
        connection=connection,
        execute=parallel_execute,
        progress=parallel_progress,
    )

    unreadable: dict[str, tuple[str, ...]] = unreadable_inputs(parallel)
    reads: list[str] = cursor_progress(parallel_progress)
    assert parallel.cursor_snapshots == sequential.cursor_snapshots
    assert sequential_execute.max_active == 1
    assert 1 < parallel_execute.max_active <= test_case.expected_max_active
    assert sorted(parallel_execute.cursor_sql) == sorted(sequential_execute.cursor_sql)
    assert len(parallel_execute.cursor_sql) == test_case.expected_statements
    assert reads[0] == test_case.expected_start_progress
    assert len(reads) == test_case.expected_statements + 2
    assert unreadable == test_case.expected_unreadable


@pytest.mark.parametrize(
    "test_case",
    [
        FailedCursorReadTestCase(
            description="failed statement names the unreadable input",
            fail_on_fetch=False,
            expected_unreadable_input_names=("events_0.event_time",),
            expected_empty_input_names=(),
            expected_unavailable_watermark_tags=(
                "daily_0__events_0__min",
                "daily_0__events_0__max",
            ),
        ),
        FailedCursorReadTestCase(
            description="failed fetch names the unreadable input",
            fail_on_fetch=True,
            expected_unreadable_input_names=("events_0.event_time",),
            expected_empty_input_names=(),
            expected_unavailable_watermark_tags=(
                "daily_0__events_0__min",
                "daily_0__events_0__max",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_cursor_read_when_gathering_then_reports_failure_and_continues(
    test_case: FailedCursorReadTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    create_events(connection, 2)
    progress: list[str] = []

    snapshot: WarehouseSnapshot = gather_with_catalog(
        project=build_fanned_cursor_project(2),
        adapter=adapter,
        connection=connection,
        execute=TrackingCursorExecute(
            failing_relations=frozenset({"staging.events_0"}),
            fail_on_fetch=test_case.fail_on_fetch,
        ),
        progress=progress,
    )

    failed: ModelCursorSnapshot = snapshot.cursor_snapshots["daily_0"]
    assert failed.unreadable_input_names == test_case.expected_unreadable_input_names
    assert failed.empty_input_names == test_case.expected_empty_input_names
    assert failed.unavailable_watermark_tags == test_case.expected_unavailable_watermark_tags
    assert snapshot.cursor_snapshots["daily_1"].upstream_maxes
    assert any(message.startswith("Failed cursor bounds (1/2)") for message in progress)


@pytest.mark.parametrize(
    "test_case",
    [
        EligibleMaxFailureTestCase(
            description="failure names the model and relation",
            model_count=4,
            expected_error_fragment=(
                "model 'daily_2': failed to query highest eligible target cursor for "
                "marts.daily_2.event_time"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failing_eligible_max_read_when_reading_in_parallel_then_plan_fails_naming_relation(
    test_case: EligibleMaxFailureTestCase,
    connection: Any,
) -> None:
    create_events(connection, test_case.model_count)
    index: int
    for index in range(test_case.model_count):
        connection.execute(f"CREATE TABLE marts.daily_{index} (event_time TIMESTAMP)")
        connection.execute(f"INSERT INTO marts.daily_{index} VALUES ('2099-01-01')")
    keys: frozenset[CompiledObjectKey] = frozenset(
        CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=f"daily_{index}")
        for index in range(test_case.model_count)
    )
    adapter: ConcurrentDuckDbAdapter = ConcurrentDuckDbAdapter()

    with (
        open_inspection_catalog(adapter=adapter, connection=connection),
        pytest.raises(PlannerInputError, match=test_case.expected_error_fragment) as raised,
    ):
        _ = gather_warehouse_snapshot(
            project=build_fanned_cursor_project(
                test_case.model_count,
                extra_config={
                    "cursor_type": "timestamp",
                    "cursor_grain": "day",
                    "cursor_start_max_ahead": "1d",
                },
            ),
            dialect=Dialect(adapter),
            reads=live_reads(
                adapter=adapter,
                connection=connection,
                execute=TrackingCursorExecute(
                    failing_eligible_relations=frozenset({"marts.daily_2"})
                ),
            ),
            cursor_scope=CursorSnapshotScope(
                model_keys=keys,
                runtime_producer_keys=keys,
                invocation_time=datetime(2026, 1, 20, tzinfo=UTC),
            ),
        )

    assert isinstance(raised.value.__cause__, RuntimeError)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
