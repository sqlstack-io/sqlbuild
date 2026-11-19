"""A replan of a maintained model reads its live refresh settings and marks rebuilt upstreams."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import ModelPlanEntry, PlanAction, PlanOutput, PlanReason
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    MaintainedRefreshPlanTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    CLEAN_ORDERS_QUERY,
    ListingDuckDbAdapter,
    build_execution_plan_from_kwargs,
    build_project_from_test_case,
    maintained_project_case,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MaintainedRefreshPlanTestCase(
            description="listed settings that match keep both models",
            listed_settings=("60 minutes", "TRANSFORM_WH"),
            upstream_query=CLEAN_ORDERS_QUERY,
            expected_upstream_action=PlanAction.KEEP_MAINTAINED,
            expected_action=PlanAction.KEEP_MAINTAINED,
            expected_reason=PlanReason.NO_CHANGE,
            expected_reinitialised_by=None,
            expected_read_names=("clean_orders", "daily_totals"),
        ),
        MaintainedRefreshPlanTestCase(
            description="a lag changed outside the project is altered in place",
            listed_settings=("2 hours", "TRANSFORM_WH"),
            upstream_query=CLEAN_ORDERS_QUERY,
            expected_upstream_action=PlanAction.ALTER_MAINTAINED,
            expected_action=PlanAction.ALTER_MAINTAINED,
            expected_reason=PlanReason.CONFIG_CHANGED,
            expected_reinitialised_by=None,
            expected_read_names=("clean_orders", "daily_totals"),
        ),
        MaintainedRefreshPlanTestCase(
            description="a name that holds no dynamic table is created",
            listed_settings=None,
            upstream_query=CLEAN_ORDERS_QUERY,
            expected_upstream_action=PlanAction.CREATE_MAINTAINED,
            expected_action=PlanAction.CREATE_MAINTAINED,
            expected_reason=PlanReason.FIRST_RUN,
            expected_reinitialised_by=None,
            expected_read_names=("clean_orders", "daily_totals"),
        ),
        MaintainedRefreshPlanTestCase(
            description="a recreated upstream marks the kept reader as recomputing in full",
            listed_settings=("1 hour", "TRANSFORM_WH"),
            upstream_query="SELECT 2 AS order_id",
            expected_upstream_action=PlanAction.CREATE_MAINTAINED,
            expected_action=PlanAction.KEEP_MAINTAINED,
            expected_reason=PlanReason.NO_CHANGE,
            expected_reinitialised_by="clean_orders",
            expected_read_names=("daily_totals",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_maintained_models_when_replanning_then_live_settings_and_rebuilds_decide(
    test_case: MaintainedRefreshPlanTestCase,
) -> None:
    adapter: ListingDuckDbAdapter = ListingDuckDbAdapter(test_case.listed_settings)
    connection: Any = adapter.connect({"database": ":memory:"})
    connection.execute("CREATE SCHEMA staging")
    first: PlanOutput = build_execution_plan_from_kwargs(
        project=build_project_from_test_case(maintained_project_case(CLEAN_ORDERS_QUERY)),
        adapter=adapter,
        connection=connection,
    )
    connection.execute("CREATE TABLE staging.clean_orders AS SELECT 1 AS order_id")
    connection.execute("CREATE TABLE staging.daily_totals AS SELECT 1 AS orders")
    warnings: list[str] = []
    for entry in first.model_entries:
        warnings.extend(
            try_write_fingerprint(
                entry=entry,
                adapter=adapter,
                connection=connection,
                run_id="first_build",
                query_change_tracking=True,
            )
        )

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=build_project_from_test_case(maintained_project_case(test_case.upstream_query)),
        adapter=adapter,
        connection=connection,
    )

    entries: dict[str, ModelPlanEntry] = {entry.name: entry for entry in plan.model_entries}
    assert warnings == []
    assert entries["clean_orders"].action == test_case.expected_upstream_action
    assert entries["daily_totals"].action == test_case.expected_action
    assert entries["daily_totals"].reason == test_case.expected_reason
    assert entries["daily_totals"].reinitialised_by == test_case.expected_reinitialised_by
    assert entries["clean_orders"].reinitialised_by is None
    assert entries["daily_totals"].target_lag == "'1 hour'"
    assert entries["daily_totals"].refresh_mode == "incremental"
    assert entries["daily_totals"].refresh_warehouse == "transform_wh"
    assert tuple(sorted(adapter.read_names)) == test_case.expected_read_names
    adapter.close(connection)
