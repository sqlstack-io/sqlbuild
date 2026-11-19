"""A table a maintained model reads is created with change tracking on every build path."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

import pytest

from sqlbuild._native import ModelPlanEntry, PlanReason
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.executor.run._helpers.materializations.snapshot import execute_snapshot_entry
from sqlbuild.executor.run.main._execute import execute_table_entry
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.run.helpers import (
    build_declared_columns,
    build_table_plan_entry,
)
from tests.integration.src.sqlbuild.executor.run.maintained._test_types import (
    ChangeTrackingSnapshotTestCase,
    ChangeTrackingTableTestCase,
)
from tests.integration.src.sqlbuild.executor.run.maintained.helpers import (
    TrackedSnapshotDuckDbAdapter,
    TrackingDuckDbAdapter,
)
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import (
    build_snapshot_execution_plan_entry,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ChangeTrackingTableTestCase(
            description="a staged build tracks changes on the stage it swaps in",
            maintained_readers=("daily_totals",),
            promotion_mode=TablePromotionMode.STAGED,
            declared_columns=(),
            expected_tracking=(True,),
        ),
        ChangeTrackingTableTestCase(
            description="type enforcement rebuilds the stage with change tracking too",
            maintained_readers=("daily_totals",),
            promotion_mode=TablePromotionMode.STAGED,
            declared_columns=(("order_id", "BIGINT"),),
            expected_tracking=(True, True),
        ),
        ChangeTrackingTableTestCase(
            description="a table no maintained model reads is created as before",
            maintained_readers=(),
            promotion_mode=TablePromotionMode.STAGED,
            declared_columns=(("order_id", "BIGINT"),),
            expected_tracking=(False, False),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_read_by_maintained_model_when_building_then_every_create_tracks_changes(
    test_case: ChangeTrackingTableTestCase, connection: Any
) -> None:
    adapter: TrackingDuckDbAdapter = TrackingDuckDbAdapter()
    entry: ModelPlanEntry = replace(
        build_table_plan_entry(
            name="orders",
            sql="SELECT 1::INTEGER AS order_id",
            target_schema="staging",
            target_name="orders",
            type_enforcement=bool(test_case.declared_columns),
        ),
        maintained_readers=test_case.maintained_readers,
    )

    result: ModelExecutionResult = execute_table_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="tracking_run",
            query_change_tracking=False,
        ),
        declared_columns=build_declared_columns(test_case.declared_columns),
        promotion_mode=test_case.promotion_mode,
    )

    assert result.status == ExecutionStatus.SUCCESS, result.error_message
    assert tuple(adapter.tracking) == test_case.expected_tracking


@pytest.mark.parametrize(
    "test_case",
    [
        ChangeTrackingSnapshotTestCase(
            description="a snapshot's first build and its full rebuild both track changes",
            maintained_readers=("daily_totals",),
            expected_tracked=(
                "main.customer_snapshot",
                "main.__sqb_rebuild__customer_snapshot",
            ),
        ),
        ChangeTrackingSnapshotTestCase(
            description="a snapshot no maintained model reads is built as before",
            maintained_readers=(),
            expected_tracked=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_read_by_maintained_model_when_rebuilding_then_it_tracks_changes(
    test_case: ChangeTrackingSnapshotTestCase,
) -> None:
    adapter: TrackedSnapshotDuckDbAdapter = TrackedSnapshotDuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    entry: ModelPlanEntry = replace(
        build_snapshot_execution_plan_entry(), maintained_readers=test_case.maintained_readers
    )

    results: list[ModelExecutionResult] = [
        execute_snapshot_entry(
            context=ModelMaterializationContext(
                entry=replace(entry, reason=reason),
                adapter=adapter,
                connection=connection,
                model_locations={},
                seed_locations={},
                source_map={},
                python_source_read_map={},
                model_audits=(),
                run_id="tracking_run",
                query_change_tracking=False,
            )
        )
        for reason in (PlanReason.FIRST_RUN, PlanReason.FULL_REFRESH)
    ]

    assert [result.status for result in results] == [ExecutionStatus.SUCCESS] * 2, [
        result.error_message for result in results
    ]
    assert tuple(adapter.tracked) == test_case.expected_tracked
    adapter.close(connection)
