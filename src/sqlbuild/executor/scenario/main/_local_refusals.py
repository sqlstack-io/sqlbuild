"""Why scenarios cannot replay locally."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import ScenarioExecutionPlan
from sqlbuild.executor.scenario._helpers.local.execution import (
    local_scenario_refusals as _local_scenario_refusals,
)
from sqlbuild.executor.scenario.models import LocalConversion


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
