"""Tests for display-only model full-refresh planning."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    build_display_only_sqlbuild_plan,
    format_plan,
)
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner.main.execution._test_types import (
    DisplayFullRefreshTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DisplayFullRefreshTestCase(
            description="unset follows CLI full refresh",
            config_values={},
            cli_full_refresh=True,
            expected_action=PlanAction.CREATE_TABLE,
            expected_reason=PlanReason.FULL_REFRESH,
            expected_full_refresh_heading=True,
        ),
        DisplayFullRefreshTestCase(
            description="false opts out of CLI full refresh",
            config_values={"full_refresh": False},
            cli_full_refresh=True,
            expected_action=PlanAction.INCREMENTAL_APPEND,
            expected_reason=PlanReason.NO_CHANGE,
            expected_full_refresh_heading=False,
        ),
        DisplayFullRefreshTestCase(
            description="true forces without CLI full refresh",
            config_values={"full_refresh": True},
            cli_full_refresh=False,
            expected_action=PlanAction.CREATE_TABLE,
            expected_reason=PlanReason.FULL_REFRESH,
            expected_full_refresh_heading=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_incremental_override_when_building_display_plan_then_uses_effective_refresh(
    test_case: DisplayFullRefreshTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql="SELECT 1 AS id",
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "append",
            }
            | test_case.config_values
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="public",
            name="orders",
            qualified_name="public.orders",
        ),
    )

    plan: PlanOutput = build_display_only_sqlbuild_plan(
        project=CompiledProject(
            run_id="test",
            effective_target_name=None,
            effective_connection={},
            effective_vars={},
            models=(model,),
        ),
        selected_model_names=("orders",),
        full_refresh=test_case.cli_full_refresh,
    )

    entry: ModelPlanEntry = plan.model_entries[0]
    assert entry.action == test_case.expected_action
    assert entry.reason == test_case.expected_reason
    rendered: str = format_plan(
        plan=plan,
        full_refresh=test_case.cli_full_refresh,
        use_color=False,
    )
    assert ("Full refresh (1)" in rendered) is test_case.expected_full_refresh_heading


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
