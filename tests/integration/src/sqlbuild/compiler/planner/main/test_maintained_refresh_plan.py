"""A replan of a maintained model reads its live refresh settings and marks rebuilt upstreams."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import ModelPlanEntry, PlanAction, PlanOutput, PlanReason
from sqlbuild.adapter.contract.types import MaintainedListing
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    ChangeTrackingPlanTestCase,
    MaintainedRefreshPlanTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    CLEAN_ORDERS_QUERY,
    ListingDuckDbAdapter,
    build_execution_plan_from_kwargs,
    build_project_from_test_case,
    change_tracking_project_case,
    maintained_project_case,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MaintainedRefreshPlanTestCase(
            description="listed settings that match keep both models",
            listed_settings=MaintainedListing("60 minutes", "TRANSFORM_WH", "ACTIVE"),
            upstream_query=CLEAN_ORDERS_QUERY,
            expected_upstream_action=PlanAction.KEEP_MAINTAINED,
            expected_action=PlanAction.KEEP_MAINTAINED,
            expected_reason=PlanReason.NO_CHANGE,
            expected_reinitialised_by=None,
            expected_read_names=("clean_orders", "daily_totals"),
        ),
        MaintainedRefreshPlanTestCase(
            description="a kept model listed as suspended is reported as paused",
            listed_settings=MaintainedListing("60 minutes", "TRANSFORM_WH", "SUSPENDED"),
            upstream_query=CLEAN_ORDERS_QUERY,
            expected_upstream_action=PlanAction.KEEP_MAINTAINED,
            expected_action=PlanAction.KEEP_MAINTAINED,
            expected_reason=PlanReason.NO_CHANGE,
            expected_reinitialised_by=None,
            expected_read_names=("clean_orders", "daily_totals"),
            expected_paused=True,
        ),
        MaintainedRefreshPlanTestCase(
            description="a lag changed outside the project is altered in place",
            listed_settings=MaintainedListing("2 hours", "TRANSFORM_WH", "ACTIVE"),
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
            listed_settings=MaintainedListing("1 hour", "TRANSFORM_WH", "ACTIVE"),
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
    assert entries["daily_totals"].maintained_paused == test_case.expected_paused
    assert entries["clean_orders"].reinitialised_by is None
    assert entries["daily_totals"].target_lag == "'1 hour'"
    assert entries["daily_totals"].refresh_mode == "incremental"
    assert entries["daily_totals"].refresh_warehouse == "transform_wh"
    assert tuple(sorted(adapter.read_names)) == test_case.expected_read_names
    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        ChangeTrackingPlanTestCase(
            description="a table a maintained model reads, directly or through a view, is tracked",
            select=(),
            expected_maintained_readers={
                "orders": ("daily_totals",),
                "customers": (),
                "payments": ("daily_totals",),
                "settled_payments": ("daily_totals",),
                "daily_totals": (),
            },
            expected_refresh_on_build={
                "orders": False,
                "customers": False,
                "payments": False,
                "settled_payments": False,
                "daily_totals": True,
            },
        ),
        ChangeTrackingPlanTestCase(
            description="the reader need not be selected for its upstream to be tracked",
            select=("orders", "customers", "payments"),
            expected_maintained_readers={
                "orders": ("daily_totals",),
                "customers": (),
                "payments": ("daily_totals",),
            },
            expected_refresh_on_build={"orders": False, "customers": False, "payments": False},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_read_by_maintained_model_when_planning_then_it_names_its_maintained_readers(
    test_case: ChangeTrackingPlanTestCase, adapter: DuckDbAdapter, connection: Any
) -> None:
    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=build_project_from_test_case(change_tracking_project_case()),
        adapter=adapter,
        connection=connection,
        select=test_case.select,
    )

    readers: dict[str, tuple[str, ...]] = {
        entry.name: entry.maintained_readers for entry in plan.model_entries
    }
    assert readers == test_case.expected_maintained_readers
    assert {
        entry.name: entry.refresh_on_build for entry in plan.model_entries
    } == test_case.expected_refresh_on_build
    assert plan.model_locations["daily_totals"].name == "daily_totals"
