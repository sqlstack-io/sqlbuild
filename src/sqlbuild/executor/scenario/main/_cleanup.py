"""Scenario cleanup execution."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import MaterializationType, ScenarioExecutionPlan
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.relations.classes.run_relation_listings import RunRelationListings
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.scenario._helpers.lifecycle.cleanup import (
    collect_scenario_cleanup_targets,
    confirmed_absent_targets,
    existing_scenario_cleanup_targets,
)
from sqlbuild.executor.scenario.constants import SCENARIO_EXEC_CLEANUP_FAILED
from sqlbuild.executor.scenario.models import (
    ScenarioCleanupExecutionResult,
    ScenarioCleanupTarget,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle


def execute_scenario_cleanup(
    *,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    connection: Any,
) -> ScenarioCleanupExecutionResult:
    """Drop the scenario-owned relations of the current plan that exist."""

    with OperationLifecycle(
        operation_kind="scenario", operation_name="scenario_cleanup"
    ) as lifecycle:
        result: ScenarioCleanupExecutionResult = _execute_scenario_cleanup(
            scenario_plan=scenario_plan, adapter=adapter, connection=connection
        )
        if result.status == ExecutionStatus.FAILED:
            lifecycle.failed(error_code=result.error_code)
        return result


def _execute_scenario_cleanup(
    *,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    connection: Any,
) -> ScenarioCleanupExecutionResult:
    statement_recorder: StatementRecorder = StatementRecorder()
    cleanup_targets: tuple[ScenarioCleanupTarget, ...] = collect_scenario_cleanup_targets(
        scenario_plan=scenario_plan,
        adapter=adapter,
    )

    try:
        dropped: tuple[ScenarioCleanupTarget, ...] = existing_scenario_cleanup_targets(
            targets=cleanup_targets, adapter=adapter, connection=connection
        )
        drop_target: ScenarioCleanupTarget
        for drop_target in dropped:
            with diagnostics_context(
                sqlbuild_phase="scenario_cleanup",
                sqlbuild_action_name="drop_relation",
                sqlbuild_scenario=scenario_plan.name,
                sqlbuild_artifact_kind=drop_target.kind.value,
                sqlbuild_artifact_name=drop_target.logical_name,
            ):
                if drop_target.materialization_type == MaterializationType.VIEW:
                    adapter.drop_view(
                        connection=connection,
                        destination=drop_target.target_relation,
                        if_exists=True,
                        statement_recorder=statement_recorder,
                    )
                elif drop_target.listed:
                    adapter.drop_table(
                        connection=connection,
                        destination=drop_target.target_relation,
                        statement_recorder=statement_recorder,
                    )
                else:
                    adapter.drop(
                        connection=connection,
                        destination=drop_target.target_relation,
                        if_exists=True,
                        statement_recorder=statement_recorder,
                    )
    except Exception as exc:
        statement_recorder.log(message=f"scenario cleanup {scenario_plan.name} failed error={exc}")
        return ScenarioCleanupExecutionResult(
            scenario_name=scenario_plan.name,
            status=ExecutionStatus.FAILED,
            targets=cleanup_targets,
            lifecycle_events=statement_recorder.snapshot(),
            error_code=SCENARIO_EXEC_CLEANUP_FAILED,
            error_help="Drop retained scenario artifacts manually or rerun `sqb janitor`.",
            error_message=str(exc),
        )

    RunRelationListings.remember_absent(
        adapter=adapter,
        relations=confirmed_absent_targets(
            targets=cleanup_targets, dropped=dropped, adapter=adapter
        ),
    )
    return ScenarioCleanupExecutionResult(
        scenario_name=scenario_plan.name,
        status=ExecutionStatus.SUCCESS,
        targets=cleanup_targets,
        lifecycle_events=statement_recorder.snapshot(),
    )
