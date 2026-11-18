"""A target opts into planning from its synced copy of warehouse state; other plans read live."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import PlanFromTestCase
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    plan_offline_snowflake_project,
    planned_entries,
    write_synced_control_store,
    write_targeted_snowflake_project,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    RecordingSnowflakeWarehouse,
    SyntheticSnowflakeProject,
)

_TARGET: str = "dev"
_MODEL_SCHEMAS: tuple[str, ...] = ("staging", "intermediate", "marts")
_SYNCED: str = "Planned from synced state: checked in "
_NO_STORE: str = "Planning live: the project has no control store"
_LIVE: str = "Planning live:"


@pytest.mark.parametrize(
    "test_case",
    [
        PlanFromTestCase(
            description="a target without the setting plans live and says nothing",
            target_plan_from=None,
            requested=None,
            store=True,
            expected_report="",
            expected_absent=(_LIVE, _SYNCED),
            expected_live_listing=True,
        ),
        PlanFromTestCase(
            description="the target's setting plans from the synced copy after one check",
            target_plan_from="synced_state",
            requested=None,
            store=True,
            expected_report=_SYNCED,
            expected_absent=(_LIVE,),
            expected_live_listing=False,
        ),
        PlanFromTestCase(
            description="the flag plans from the synced copy for a target without the setting",
            target_plan_from=None,
            requested="synced_state",
            store=True,
            expected_report=_SYNCED,
            expected_absent=(_LIVE,),
            expected_live_listing=False,
        ),
        PlanFromTestCase(
            description="the flag plans live for a target with the setting",
            target_plan_from="synced_state",
            requested="live",
            store=True,
            expected_report="",
            expected_absent=(_LIVE, _SYNCED),
            expected_live_listing=True,
        ),
        PlanFromTestCase(
            description="a project without a control store plans live and says why",
            target_plan_from="synced_state",
            requested=None,
            store=False,
            expected_report=_NO_STORE,
            expected_absent=(_SYNCED,),
            expected_live_listing=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_from_when_planning_then_reads_the_synced_copy_only_when_asked(
    test_case: PlanFromTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_CONTROL_DATABASE_URL", raising=False)
    project: SyntheticSnowflakeProject = write_targeted_snowflake_project(
        project_dir=tmp_path / "orders_platform",
        target=_TARGET,
        plan_from=test_case.target_plan_from,
    )
    live: CompilePipelineResult = plan_offline_snowflake_project(
        project=project,
        warehouse=RecordingSnowflakeWarehouse(relations=project.relations),
        plan_from="live",
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=project.relations
    )
    write_synced_control_store(
        project_dir=(tmp_path / "no_store", project.project_dir)[test_case.store],
        target=_TARGET,
        warehouse=warehouse,
        schemas=_MODEL_SCHEMAS,
    )
    warehouse.queries.clear()
    _ = capsys.readouterr()

    result: CompilePipelineResult = plan_offline_snowflake_project(
        project=project, warehouse=warehouse, plan_from=test_case.requested
    )

    err: str = capsys.readouterr().err
    assert planned_entries(result) == planned_entries(live)
    assert (
        any(
            query.kind == "show_tables" and "STAGING" in f"{query.sql} {query.params}"
            for query in warehouse.queries
        )
        == test_case.expected_live_listing
    )
    assert test_case.expected_report in err
    assert not any(text in err for text in test_case.expected_absent)
