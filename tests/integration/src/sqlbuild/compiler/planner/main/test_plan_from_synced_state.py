"""A target opts into planning from its synced copy of warehouse state; other plans read live."""

from __future__ import annotations

import re
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    PlanFromTestCase,
    SyncedScanTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    plan_offline_snowflake_project,
    planned_entries,
    uuid7_at,
    write_synced_control_store,
    write_targeted_snowflake_project,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeRelation,
    RecordingSnowflakeWarehouse,
    SyntheticSnowflakeProject,
)

_TARGET: str = "dev"
_MODEL_SCHEMAS: tuple[str, ...] = ("staging", "intermediate", "marts")
_SYNCED: str = "Planned from synced state: checked in "
_NO_STORE: str = "Planning live: the project has no control store"
_LIVE: str = "Planning live:"
_FIRST_OBSERVED: datetime = datetime(2026, 2, 1, tzinfo=UTC)
_SCAN_STARTED: datetime = datetime(2026, 3, 1, tzinfo=UTC)
# The scan read the page two seconds after it started, and the store received it a second later.
_PAGE_RECEIVED: datetime = _SCAN_STARTED + timedelta(seconds=3)


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


@pytest.mark.parametrize(
    "test_case",
    [
        SyncedScanTestCase(
            description="a table replaced with the same columns before a later scan is not described",
            last_ddl_after_scan_start=timedelta(hours=-1),
            expected_live_describe=False,
        ),
        SyncedScanTestCase(
            description="a table replaced within the clock skew before the scan started is described",
            last_ddl_after_scan_start=timedelta(minutes=-1),
            expected_live_describe=True,
        ),
        SyncedScanTestCase(
            description="a table replaced after the scan started but before its page was read is described",
            last_ddl_after_scan_start=timedelta(seconds=1),
            expected_live_describe=True,
        ),
        SyncedScanTestCase(
            description="a table replaced after the scan is described",
            last_ddl_after_scan_start=timedelta(hours=1),
            expected_live_describe=True,
        ),
        SyncedScanTestCase(
            description="a table replaced a second after its page was read is described",
            last_ddl_after_scan_start=timedelta(seconds=3),
            expected_live_describe=True,
        ),
        SyncedScanTestCase(
            description="a table replaced a second after its page was read by a location clock 10 minutes fast is described",
            last_ddl_after_scan_start=timedelta(seconds=3),
            expected_live_describe=True,
            location_clock_ahead=timedelta(minutes=10),
        ),
        SyncedScanTestCase(
            description="a table replaced with the same columns before a scan by a location clock 10 minutes fast is not described",
            last_ddl_after_scan_start=timedelta(hours=-1),
            expected_live_describe=False,
            location_clock_ahead=timedelta(minutes=10),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_later_scan_found_a_record_unchanged_when_planning_from_synced_state_then_describes_only_tables_altered_since_it_started(
    test_case: SyncedScanTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_CONTROL_DATABASE_URL", raising=False)
    project: SyntheticSnowflakeProject = write_targeted_snowflake_project(
        project_dir=tmp_path / "orders_platform", target=_TARGET, plan_from="synced_state"
    )
    name: str = project.incremental_model_names[0].upper()
    original: FakeRelation = next(filter(lambda relation: relation.name == name, project.relations))
    others: tuple[FakeRelation, ...] = tuple(
        filter(lambda relation: relation.name != name, project.relations)
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=(
            *others,
            replace(original, last_ddl=_SCAN_STARTED + test_case.last_ddl_after_scan_start),
        )
    )
    write_synced_control_store(
        project_dir=project.project_dir,
        target=_TARGET,
        warehouse=warehouse,
        schemas=_MODEL_SCHEMAS,
        observed_at=_FIRST_OBSERVED,
        scan_id=uuid7_at(_SCAN_STARTED + test_case.location_clock_ahead),
        received_at=_PAGE_RECEIVED,
    )
    warehouse.queries.clear()

    _ = plan_offline_snowflake_project(project=project, warehouse=warehouse)

    assert (
        any(
            query.kind in ("show_columns", "columns")
            and re.search(rf"\b{name}\b", f"{query.sql} {query.params}", re.IGNORECASE)
            for query in warehouse.queries
        )
        == test_case.expected_live_describe
    )
