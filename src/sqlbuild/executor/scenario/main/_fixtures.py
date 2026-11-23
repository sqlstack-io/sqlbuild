"""Scenario fixture materialization."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import ScenarioFixturePlan
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.scenario.constants import SCENARIO_EXEC_FIXTURE_FAILED
from sqlbuild.executor.scenario.models import ScenarioFixtureExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus


def execute_scenario_fixture(
    *,
    scenario_name: str,
    fixture_plan: ScenarioFixturePlan,
    adapter: BaseAdapter,
    connection: Any,
    schema_prepared: bool = False,
) -> ScenarioFixtureExecutionResult:
    """Materialize one source/ref/seed fixture as a scenario-owned table."""

    statement_recorder: StatementRecorder = StatementRecorder()
    target_relation: str = resolve_relation_location_qualified_name(
        adapter=adapter,
        location=fixture_plan.destination,
    )

    try:
        if not schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=fixture_plan.destination.database,
                schema=fixture_plan.destination.schema,
                statement_recorder=statement_recorder,
            )
        with diagnostics_context(
            sqlbuild_phase="scenario_fixture",
            sqlbuild_action_name="create_table",
            sqlbuild_scenario=scenario_name,
            sqlbuild_fixture_kind=fixture_plan.kind.value,
            sqlbuild_fixture_name=fixture_plan.logical_name,
        ):
            adapter.create_table_as(
                connection=connection,
                destination=target_relation,
                sql=fixture_plan.sql,
                config=None,
                statement_recorder=statement_recorder,
            )
    except Exception as exc:
        statement_recorder.log(
            message="scenario fixture "
            f"{scenario_name}:{fixture_plan.kind.value}:{fixture_plan.logical_name} "
            f"failed error={exc}"
        )
        return ScenarioFixtureExecutionResult(
            scenario_name=scenario_name,
            kind=fixture_plan.kind,
            logical_name=fixture_plan.logical_name,
            target_relation=target_relation,
            status=ExecutionStatus.FAILED,
            lifecycle_events=statement_recorder.snapshot(),
            error_code=SCENARIO_EXEC_FIXTURE_FAILED,
            error_help=(
                "Check the fixture CTE SQL and rerun with --retain to inspect generated SQL."
            ),
            error_message=str(exc),
        )

    return ScenarioFixtureExecutionResult(
        scenario_name=scenario_name,
        kind=fixture_plan.kind,
        logical_name=fixture_plan.logical_name,
        target_relation=target_relation,
        status=ExecutionStatus.SUCCESS,
        lifecycle_events=statement_recorder.snapshot(),
    )
