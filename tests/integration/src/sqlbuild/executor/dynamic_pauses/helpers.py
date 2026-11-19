"""A DuckDB adapter that tracks dynamic-table scheduling, and the plans the lease tests build."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from typing import Any

from sqlbuild._native import PlanOutput
from sqlbuild.adapter.contract.types import (
    DynamicListing,
    DynamicOperation,
    TablePromotionMode,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.main._execute import execute_build_plan
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildExecutionResult,
    BuildRuntimeParams,
)
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    BuildExecutionPlanTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    build_execution_plan_from_kwargs,
    build_project_from_test_case,
)

type LeaseProject = BuildExecutionPlanTestCase

_DYNAMIC: dict[str, object] = {
    "materialized": "dynamic",
    "refresh": {"lag": "1h"},
    "refresh_warehouse": "transform_wh",
}
_CREATED_STATE: str = "ACTIVE"
TOTALS_QUERY: str = "SELECT count(*) AS orders FROM staging.orders"
_STATES: dict[DynamicOperation, str] = {
    DynamicOperation.SUSPEND: "SUSPENDED",
    DynamicOperation.RESUME: "ACTIVE",
}


class PausingDuckDbAdapter(DuckDbAdapter):
    """Builds dynamic models as tables and keeps their scheduling state as Snowflake would."""

    def __init__(self) -> None:
        super().__init__()
        self.states: dict[str, str] = {}
        self.operations: list[tuple[str, str]] = []

    def session_warehouse(self, *, connection: Any) -> str | None:
        del connection
        return "SESSION_WH"

    def dynamic_listing(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> DynamicListing | None:
        del connection, database, schema
        listings: dict[str, DynamicListing] = {
            listed: DynamicListing("1 hour", "TRANSFORM_WH", state)
            for listed, state in self.states.items()
        }
        return listings.get(name)

    def render_create_dynamic(
        self, *, destination: str, sql: str, target_lag: str, refresh_mode: str, warehouse: str
    ) -> tuple[str, ...]:
        del target_lag, refresh_mode, warehouse
        name: str = destination.rsplit(".", 1)[-1]
        self.states[name] = _CREATED_STATE
        self.operations.append(("create", name))
        return (f"CREATE OR REPLACE TABLE {destination} AS {sql}",)

    def render_dynamic_operation(
        self, *, destination: str, operation: DynamicOperation
    ) -> tuple[str, ...]:
        name: str = destination.rsplit(".", 1)[-1]
        self.states[name] = _STATES.get(operation, self.states[name])
        self.operations.append((operation.value, name))
        return ("SELECT 1",)


def lease_project_case(
    *,
    orders_query: str,
    report_locations: dict[str, str],
    refresh_on_build: bool,
    totals_query: str = TOTALS_QUERY,
) -> BuildExecutionPlanTestCase:
    """Orders, dynamic daily totals built from them, and any report table that reads those."""

    dynamic: dict[str, object] = {**_DYNAMIC, "refresh_on_build": refresh_on_build}
    return BuildExecutionPlanTestCase(
        description="orders and the daily totals built from them as a dynamic model",
        setup_sql=(),
        model_locations={"orders": "staging", "daily_totals": "staging", **report_locations},
        model_configs={
            "orders": {"materialized": "table"},
            "daily_totals": dynamic,
            "order_report": {"materialized": "table"},
        },
        model_queries={
            "orders": orders_query,
            "daily_totals": totals_query,
            "order_report": "SELECT orders FROM staging.daily_totals",
        },
        model_deps={"daily_totals": ("orders",), "order_report": ("daily_totals",)},
        full_refresh=False,
        expected_action={},
        expected_reason={},
    )


def plan_lease_project(
    *,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    case: BuildExecutionPlanTestCase,
    select: tuple[str, ...] = (),
) -> PlanOutput:
    """Plan the selected models against the warehouse as it stands."""

    return build_execution_plan_from_kwargs(
        project=build_project_from_test_case(case),
        adapter=adapter,
        connection=connection,
        select=select,
    )


def build_lease_project(
    *,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    case: BuildExecutionPlanTestCase,
    runtime: BuildRuntimeParams,
    database_path: Path,
    select: tuple[str, ...] = (),
    callbacks: BuildCallbacks | None = None,
) -> BuildExecutionResult:
    """Plan the selected models, then build them with `runtime` and `callbacks`."""

    return execute_build_plan(
        plan=plan_lease_project(adapter=adapter, connection=connection, case=case, select=select),
        adapter=adapter,
        connection_config={"database": str(database_path)},
        connections=(connection,),
        scheduler_connection=connection,
        runtime=replace(
            runtime,
            promotion_mode=TablePromotionMode(adapter.default_table_promotion_mode()),
        ),
        callbacks=callbacks,
    )


def pause_events(*, connection: Any) -> list[tuple[str, str, str, str]]:
    """Each recorded pause event's type, holding run, recording run and model, sorted."""

    rows: list[tuple[Any, ...]] = connection.execute(
        "SELECT event_type, run_id, recorded_by, relation_name "
        "FROM staging._sqlbuild_dynamic_pauses WHERE event_type <> 'heartbeat' "
        "ORDER BY 1, 2, 3, 4"
    ).fetchall()
    return [(str(row[0]), str(row[1]), str(row[2]), str(row[3])) for row in rows]


def pause_table_names(*, connection: Any) -> list[tuple[Any, ...]]:
    """The pause state table, when a build created one in staging."""

    return connection.execute(
        "SELECT table_name FROM information_schema.tables "
        "WHERE table_schema = 'staging' AND table_name = '_sqlbuild_dynamic_pauses'"
    ).fetchall()
