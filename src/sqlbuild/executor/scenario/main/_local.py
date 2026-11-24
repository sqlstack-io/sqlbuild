"""Local scenario replay entrypoint."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import ScenarioExecutionPlan
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.executor.scenario._helpers.local.execution import (
    execute_local_scenario_load_only_run as _execute_local_scenario_load_only_run,
)
from sqlbuild.executor.scenario._helpers.local.execution import (
    local_scenario_refusals as _local_scenario_refusals,
)
from sqlbuild.executor.scenario.classes.running_scenarios import RunningScenarios
from sqlbuild.executor.scenario.models import LocalConversion, ScenarioRunResult


def execute_local_scenario_load_only_run(
    *,
    project_dir: Path,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    promotion_mode: TablePromotionMode,
    capture_adapter: str | None = None,
    capture_dialect: str | None = None,
    running: RunningScenarios | None = None,
    conversion: LocalConversion | None = None,
) -> ScenarioRunResult:
    """Run one scenario locally against a run-scoped DuckDB database."""

    return _execute_local_scenario_load_only_run(
        project_dir=project_dir,
        scenario_plan=scenario_plan,
        adapter=adapter,
        promotion_mode=promotion_mode,
        capture_adapter=capture_adapter,
        capture_dialect=capture_dialect,
        running=running,
        conversion=conversion,
    )


def local_scenario_refusals(
    *,
    project_dir: Path,
    scenario_plan: ScenarioExecutionPlan,
    capture_adapter: str | None,
    capture_dialect: str | None,
    conversion: LocalConversion,
) -> tuple[dict[str, object], ...]:
    """Why a scenario cannot replay locally; empty when it can."""

    return _local_scenario_refusals(
        project_dir=project_dir,
        scenario_plan=scenario_plan,
        capture_adapter=capture_adapter,
        capture_dialect=capture_dialect,
        conversion=conversion,
    )
