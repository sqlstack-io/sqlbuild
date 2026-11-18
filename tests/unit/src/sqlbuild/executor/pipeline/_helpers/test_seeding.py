"""Tests for seed pipeline execution helpers."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast
from unittest.mock import patch

import pytest

from sqlbuild._native import PlanOutput, TerminalEventIndex, format_seed_execution_json
from sqlbuild.executor.build.models import SeedExecutionResult
from sqlbuild.executor.pipeline._helpers.seeding import run_seed_pipeline
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.executor.pipeline._helpers._test_types import (
    SeedFingerprintSetupTestCase,
    SeedPipelineConcurrencyTestCase,
    SeedPipelineLifecycleTestCase,
    SeedPipelineQueueTestCase,
)
from tests.unit.src.sqlbuild.executor.pipeline._helpers.helpers import (
    RecordingDuckDbAdapter,
    SeedPipelineTestAdapter,
    SlowSeedPipelineAdapter,
    build_seed_plan,
    build_tracked_seed_plan,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SeedPipelineConcurrencyTestCase(
            description="uses bounded concurrent connections and preserves seed result order",
            seed_names=("seed_a", "seed_b", "seed_c"),
            max_concurrency=2,
            expected_connection_count=2,
            expected_seed_order=("seed_a", "seed_b", "seed_c"),
            expected_json_asset_order=("seed_a", "seed_b", "seed_c"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_seeds_when_running_seed_pipeline_then_uses_concurrent_connections(
    test_case: SeedPipelineConcurrencyTestCase,
) -> None:
    adapter: SeedPipelineTestAdapter = SeedPipelineTestAdapter(barrier_targets=("seed_a", "seed_b"))
    connection_starts: list[int] = []

    results: tuple[SeedExecutionResult, ...] = run_seed_pipeline(
        plan=build_seed_plan(seed_names=test_case.seed_names),
        connection_config={},
        adapter=adapter,
        max_concurrency=test_case.max_concurrency,
        on_connection_start=connection_starts.append,
    )

    assert connection_starts == [test_case.expected_connection_count]
    assert tuple(result.seed_name for result in results) == test_case.expected_seed_order
    payload: dict[str, Any] = cast(
        dict[str, Any],
        json.loads(
            format_seed_execution_json(results=results, plan=build_seed_plan(seed_names=()))
        ),
    )
    assets: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["assets"])
    assert tuple(asset["name"] for asset in assets) == test_case.expected_json_asset_order
    assert len(adapter.connections) == test_case.expected_connection_count
    loads_by_target: dict[str, object] = dict(adapter.loads)
    assert loads_by_target["seed_a"] is not loads_by_target["seed_b"]
    assert adapter.closed_connections == adapter.connections


@pytest.mark.parametrize(
    "test_case",
    (
        SeedPipelineLifecycleTestCase(
            description="concurrent attempts complete before callbacks",
            seed_names=("seed_a", "seed_b"),
            expected_terminal_types=(
                "resource_attempt_completed",
                "resource_attempt_completed",
            ),
            expected_resource_ids=frozenset({"seed:seed_a", "seed:seed_b"}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_concurrent_standalone_seeds_when_callbacks_run_then_each_terminal_precedes_callback(
    test_case: SeedPipelineLifecycleTestCase,
) -> None:
    plan: PlanOutput = build_seed_plan(seed_names=test_case.seed_names)
    adapter: SeedPipelineTestAdapter = SeedPipelineTestAdapter(barrier_targets=("seed_a", "seed_b"))
    dispatcher: EventDispatcher = EventDispatcher()
    terminal_index: TerminalEventIndex = TerminalEventIndex()
    _ = dispatcher.subscribe_lifecycle(subscriber=terminal_index.consume, accepts_opaque=False)
    callback_terminals: list[str] = []

    def on_seed_complete(result: SeedExecutionResult) -> None:
        terminal: LifecycleEvent | None = terminal_index.resource_terminal(
            resource_name=result.seed_name,
            resource_id=f"seed:{result.seed_name}",
        )
        assert terminal is not None
        callback_terminals.append(terminal.event_type)

    with invocation_scope("seed-pipeline-invocation"), dispatcher_scope(dispatcher):
        results: tuple[SeedExecutionResult, ...] = run_seed_pipeline(
            plan=plan,
            connection_config={},
            adapter=adapter,
            max_concurrency=2,
            run_id="seed-pipeline-run",
            on_seed_complete=on_seed_complete,
        )

    resource_events: tuple[LifecycleEvent, ...] = terminal_index.events()
    assert tuple(result.seed_name for result in results) == test_case.seed_names
    assert tuple(callback_terminals) == test_case.expected_terminal_types
    assert len(resource_events) == 4
    assert {event.invocation_id for event in resource_events} == {"seed-pipeline-invocation"}
    assert {event.run_id for event in resource_events} == {"seed-pipeline-run"}
    assert {event.resource_id for event in resource_events} == test_case.expected_resource_ids


@pytest.mark.parametrize(
    "test_case",
    (
        SeedPipelineLifecycleTestCase(
            description="base exception fails one attempt without callback",
            seed_names=("seed_a",),
            expected_terminal_types=(
                "resource_attempt_started",
                "resource_attempt_failed",
            ),
            expected_resource_ids=frozenset({"seed:seed_a"}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_seed_executor_raises_when_running_worker_then_attempt_fails_once_without_callback(
    test_case: SeedPipelineLifecycleTestCase,
) -> None:
    adapter: SeedPipelineTestAdapter = SeedPipelineTestAdapter(barrier_targets=("seed_a",))
    dispatcher: EventDispatcher = EventDispatcher()
    terminal_index: TerminalEventIndex = TerminalEventIndex()
    _ = dispatcher.subscribe_lifecycle(subscriber=terminal_index.consume, accepts_opaque=False)
    callbacks: list[SeedExecutionResult] = []

    with (
        invocation_scope("seed-exception-invocation"),
        dispatcher_scope(dispatcher),
        patch.object(adapter, "load_seed", side_effect=KeyboardInterrupt("stop")),
        pytest.raises(KeyboardInterrupt, match="stop"),
    ):
        _ = run_seed_pipeline(
            plan=build_seed_plan(seed_names=test_case.seed_names),
            connection_config={},
            adapter=adapter,
            max_concurrency=1,
            run_id="seed-exception-run",
            on_seed_complete=callbacks.append,
        )

    resource_events: tuple[LifecycleEvent, ...] = terminal_index.events()
    assert tuple(event.event_type for event in resource_events) == test_case.expected_terminal_types
    assert {event.resource_id for event in resource_events} == test_case.expected_resource_ids
    assert callbacks == []


@pytest.mark.parametrize(
    "test_case",
    [
        SeedPipelineQueueTestCase(
            description="seeds behind a slow one load on the free connection meanwhile",
            seed_names=("seed_slow", "seed_b", "seed_c", "seed_d"),
            slow_seed="seed_slow",
            max_concurrency=2,
            expected_load_order=("seed_b", "seed_c", "seed_d", "seed_slow"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_slow_seed_when_running_seed_pipeline_then_other_seeds_take_the_free_connection(
    test_case: SeedPipelineQueueTestCase,
) -> None:
    adapter: SlowSeedPipelineAdapter = SlowSeedPipelineAdapter(
        slow_target=test_case.slow_seed, other_count=len(test_case.seed_names) - 1
    )

    results: tuple[SeedExecutionResult, ...] = run_seed_pipeline(
        plan=build_seed_plan(seed_names=test_case.seed_names),
        connection_config={},
        adapter=adapter,
        max_concurrency=test_case.max_concurrency,
    )

    assert tuple(adapter.order) == test_case.expected_load_order, test_case.description
    assert tuple(result.seed_name for result in results) == test_case.seed_names


@pytest.mark.parametrize(
    "test_case",
    [
        SeedFingerprintSetupTestCase(
            description="concurrent seeds on an empty schema set the table up once and each "
            "record a row",
            seed_names=("seed_a", "seed_b", "seed_c", "seed_d"),
            max_concurrency=2,
            expected_table_setups=1,
            expected_fingerprint_rows=4,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_seeds_when_recording_identity_then_the_fingerprint_table_is_set_up_once(
    test_case: SeedFingerprintSetupTestCase,
    tmp_path: Path,
) -> None:
    adapter: RecordingDuckDbAdapter = RecordingDuckDbAdapter()
    config: dict[str, object] = {"database": str(tmp_path / "warehouse.duckdb")}

    results: tuple[SeedExecutionResult, ...] = run_seed_pipeline(
        plan=build_tracked_seed_plan(seed_names=test_case.seed_names, seed_dir=tmp_path),
        connection_config=config,
        adapter=adapter,
        max_concurrency=test_case.max_concurrency,
        run_id="seed-fingerprint-run",
        query_change_tracking=True,
    )

    assert {result.status.value for result in results} == {"success"}, results
    assert [result.warning_messages for result in results] == [()] * len(test_case.seed_names)
    setups: int = sum(
        sql.startswith("CREATE TABLE IF NOT EXISTS main._sqlbuild_fingerprints")
        for sql in adapter.statements
    )
    assert setups == test_case.expected_table_setups, adapter.statements
    connection: Any = adapter.connect(config)
    rows: list[tuple[Any, ...]] = adapter.execute(
        connection=connection, sql="SELECT count(*) FROM main._sqlbuild_fingerprints"
    ).fetchall()
    adapter.close(connection)
    assert rows == [(test_case.expected_fingerprint_rows,)]
