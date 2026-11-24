"""E2E coverage: every model's replay comes only from its own change and replay_on_change."""

from __future__ import annotations

import json
import subprocess
from datetime import date
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build.own_change_replay._test_types import (
    FullRefreshIntrinsicTestCase,
    FunctionCallerReplayTestCase,
    OwnChangePlanBuildTestCase,
    PlanningErrorRowTestCase,
    RefusedPlanTestCase,
    RenamePlanBuildTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.own_change_replay.helpers import (
    ENFORCED_CONTRACT_CONFIG,
    add_upstream_column,
    assert_plan_fails,
    assert_plan_text,
    build_and_assert_rows,
    build_event_project,
    build_function_project,
    build_star_project,
    declare_downstream_column_type,
    direct_reference_merge_files,
    edit_direct_reference_merge,
    insert_table_between_upstream_and_view,
    mark_first_day,
    merge_downstream_files,
    plan,
    plan_models,
    relation_rows,
    relation_types,
    rename_upstream_with_cli,
    rename_upstream_with_migrate_from,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_REPLAYED_TAIL: list[tuple[Any, ...]] = [
    (2, date(2026, 9, 2)),
    (3, date(2026, 9, 3)),
    (101, date(2026, 9, 1)),
]
_CHANGED_UPSTREAM_SQL: str = "WHERE\n  NOT id IS NULL\n"
_RENAMED_ROWS: dict[str, list[tuple[Any, ...]]] = {
    "upstream_renamed": [
        (1, date(2026, 9, 1)),
        (2, date(2026, 9, 2)),
        (3, date(2026, 9, 3)),
    ],
    "downstream_mb": _REPLAYED_TAIL,
}


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="new upstream table is a first run and the microbatch continues forward",
            expected_plan_fragments=(
                "First run (1)",
                "upstream_new",
                "Query changed (1)",
                "upstream_v           recreate view",
                "downstream_mb        delete_insert (timestamp, microbatch)",
            ),
            unexpected_plan_fragments=("Upstream changed", "full rebuild", "cause"),
            expected_reasons={
                "upstream_new": "first_run",
                "upstream_v": "query_changed",
                "downstream_mb": "normal_incremental",
            },
            expected_rows={"downstream_mb": _REPLAYED_TAIL},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_new_upstream_table_when_planning_and_building_then_downstream_continues_forward(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(tmp_path=tmp_path, extra_files={})
    mark_first_day(project_dir=project_dir, relations=("downstream_mb",))
    insert_table_between_upstream_and_view(project_dir=project_dir)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    assert all("cascade" not in model for model in models.values())
    assert models["downstream_mb"]["backfill"] == {"action": "forward", "duration": None}
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="merge model with cursor intrinsics plans and continues forward",
            expected_plan_fragments=(
                "First run (1)",
                "downstream           merge (timestamp)",
                "downstream_mb        delete_insert (timestamp, microbatch)",
            ),
            unexpected_plan_fragments=("Upstream changed", "full rebuild", "error"),
            expected_reasons={
                "downstream": "normal_incremental",
                "downstream_mb": "normal_incremental",
            },
            expected_rows={"downstream": _REPLAYED_TAIL, "downstream_mb": _REPLAYED_TAIL},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_intrinsic_merge_downstream_when_upstream_is_new_then_plan_succeeds(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(
        tmp_path=tmp_path,
        extra_files=merge_downstream_files(extra_config="  full_refresh false,\n"),
    )
    mark_first_day(project_dir=project_dir, relations=("downstream", "downstream_mb"))
    insert_table_between_upstream_and_view(project_dir=project_dir)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        RenamePlanBuildTestCase(
            description="rename with identical SQL continues forward",
            extra_sql="",
            expected_plan_fragments=(
                "upstream_renamed  migrate  main.upstream -> main.upstream_renamed",
                "Renamed (1)",
                "upstream_renamed     continue forward",
            ),
            unexpected_plan_fragments=("First run", "Upstream changed", "full rebuild"),
            expected_query_changed=False,
            expected_rows=_RENAMED_ROWS,
            expected_relation_types={"upstream": ["VIEW"], "upstream_renamed": ["BASE TABLE"]},
        ),
        RenamePlanBuildTestCase(
            description="rename with changed SQL is a renamed query change",
            extra_sql=_CHANGED_UPSTREAM_SQL,
            expected_plan_fragments=(
                "upstream_renamed  migrate  main.upstream -> main.upstream_renamed",
                "Renamed (1)",
                "upstream_renamed     continue forward",
                "+  NOT id IS NULL",
            ),
            unexpected_plan_fragments=("First run", "Upstream changed", "full rebuild"),
            expected_query_changed=True,
            expected_rows=_RENAMED_ROWS,
            expected_relation_types={"upstream": ["VIEW"], "upstream_renamed": ["BASE TABLE"]},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_migrate_from_rename_when_planning_and_building_then_plans_rename_not_first_run(
    test_case: RenamePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(tmp_path=tmp_path, extra_files={})
    mark_first_day(project_dir=project_dir, relations=("downstream_mb",))
    rename_upstream_with_migrate_from(project_dir=project_dir, extra_sql=test_case.extra_sql)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert models["upstream_renamed"]["reason"] == "renamed"
    assert models["upstream_renamed"].get("query_changed", False) is (
        test_case.expected_query_changed
    )
    assert models["downstream_mb"]["reason"] == "normal_incremental"
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)
    assert {
        name: relation_types(project_dir=project_dir, name=name)
        for name in test_case.expected_relation_types
    } == test_case.expected_relation_types
    assert plan_models(project_dir=project_dir)["upstream_renamed"]["reason"] == "no_change"


@pytest.mark.parametrize(
    "test_case",
    [
        RenamePlanBuildTestCase(
            description="sqb rename followed by a SQL edit plans a renamed query change",
            extra_sql=_CHANGED_UPSTREAM_SQL,
            expected_plan_fragments=("Renamed (1)", "upstream_renamed", "+  NOT id IS NULL"),
            unexpected_plan_fragments=("First run", "Upstream changed", "full rebuild"),
            expected_query_changed=True,
            expected_rows=_RENAMED_ROWS,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqb_rename_when_planning_then_model_is_renamed_not_first_run(
    test_case: RenamePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(tmp_path=tmp_path, extra_files={})
    mark_first_day(project_dir=project_dir, relations=("downstream_mb",))
    rename_upstream_with_cli(project_dir=project_dir, extra_sql=test_case.extra_sql)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert (models["upstream_renamed"]["reason"], models["downstream_mb"]["reason"]) == (
        "renamed",
        "normal_incremental",
    )
    assert models["upstream_renamed"].get("query_changed", False) is (
        test_case.expected_query_changed
    )
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionCallerReplayTestCase(
            description="forward-only caller and its downstream both continue forward",
            caller_config="",
            expected_plan_fragments=(
                "Function changed (1)",
                "order_amounts        continue forward",
                "cause  function udf__adjust_amount changed",
                "order_totals         merge (timestamp)",
            ),
            expected_caller_action="incremental_merge",
            expected_rows={
                "order_amounts": [
                    (1, date(2026, 9, 1), 11),
                    (2, date(2026, 9, 2), 21),
                    (3, date(2026, 9, 3), 130),
                ],
                "order_totals": [
                    (1, date(2026, 9, 1), 22),
                    (2, date(2026, 9, 2), 42),
                    (3, date(2026, 9, 3), 260),
                ],
            },
        ),
        FunctionCallerReplayTestCase(
            description="full replay caller rebuilds while its downstream continues forward",
            caller_config="  replay_on_change full,\n",
            expected_plan_fragments=(
                "Function changed (1)",
                "order_amounts        full rebuild",
                "cause  function udf__adjust_amount changed",
                "order_totals         merge (timestamp)",
            ),
            expected_caller_action="create_table",
            expected_rows={
                "order_amounts": [
                    (1, date(2026, 9, 1), 110),
                    (2, date(2026, 9, 2), 120),
                    (3, date(2026, 9, 3), 130),
                ],
                "order_totals": [
                    (1, date(2026, 9, 1), 22),
                    (2, date(2026, 9, 2), 42),
                    (3, date(2026, 9, 3), 260),
                ],
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_changed_function_when_planning_and_building_then_only_direct_caller_replays(
    test_case: FunctionCallerReplayTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_function_project(
        tmp_path=tmp_path, caller_config=test_case.caller_config
    )

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=("Upstream changed",),
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert models["order_amounts"]["reason"] == "function_changed"
    assert models["order_amounts"]["changed_functions"] == ["udf__adjust_amount"]
    assert models["order_amounts"]["action"] == test_case.expected_caller_action
    assert models["order_totals"]["reason"] == "normal_incremental"
    assert "changed_functions" not in models["order_totals"]
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        RefusedPlanTestCase(
            description="full_refresh false caller refuses a function-driven full rebuild",
            expected_fragments=(
                "error[S203]",
                "model 'order_amounts' sets full_refresh false",
                "function udf__adjust_amount changed with replay_on_change full",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_full_refresh_false_caller_when_function_forces_full_rebuild_then_plan_fails(
    test_case: RefusedPlanTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_function_project(
        tmp_path=tmp_path, caller_config="  replay_on_change full,\n  full_refresh false,\n"
    )

    assert_plan_fails(project_dir=project_dir, args=(), expected=test_case.expected_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanningErrorRowTestCase(
            description="a model that can't be planned is an error row and the rest still plans",
            expected_code="S203",
            expected_error_model="order_amounts",
            expected_planned_model="order_totals",
            expected_text_fragments=(
                "Planning failed",
                "error[S203]: model 'order_amounts' sets full_refresh false",
                "Plan failed with errors",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_model_that_cannot_be_planned_when_planning_then_it_is_an_error_row_and_builds_refuse(
    test_case: PlanningErrorRowTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_function_project(
        tmp_path=tmp_path, caller_config="  replay_on_change full,\n  full_refresh false,\n"
    )
    totals_before: list[tuple[Any, ...]] = relation_rows(
        project_dir=project_dir, relation=test_case.expected_planned_model
    )

    planned_json: subprocess.CompletedProcess[str] = plan(project_dir=project_dir, args=("--json",))
    planned_text: subprocess.CompletedProcess[str] = plan(project_dir=project_dir)
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    document: dict[str, Any] = json.loads(planned_json.stdout)
    models: dict[str, dict[str, Any]] = {model["name"]: model for model in document["models"]}
    assert planned_json.returncode == 1, planned_json.stdout + planned_json.stderr
    assert document["has_errors"] is True
    assert models[test_case.expected_error_model]["action"] == "error"
    assert models[test_case.expected_error_model]["error"]["code"] == test_case.expected_code
    assert models[test_case.expected_planned_model]["action"] != "error"
    assert "error" not in models[test_case.expected_planned_model]
    assert planned_text.returncode == 1
    assert all(fragment in planned_text.stdout for fragment in test_case.expected_text_fragments), (
        planned_text.stdout
    )
    assert built.returncode != 0, built.stdout + built.stderr
    assert f"error[{test_case.expected_code}]" in built.stdout + built.stderr
    assert (
        relation_rows(project_dir=project_dir, relation=test_case.expected_planned_model)
        == totals_before
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FullRefreshIntrinsicTestCase(
            description="explicit full refresh of a cursor-intrinsic merge spans every value",
            expected_plan_fragments=("(unbounded)",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_full_refresh_flag_on_cursor_intrinsic_merge_when_building_then_rebuilds_every_row(
    test_case: FullRefreshIntrinsicTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(
        tmp_path=tmp_path, extra_files=merge_downstream_files(extra_config="")
    )
    before: list[tuple[Any, ...]] = relation_rows(project_dir=project_dir, relation="downstream")

    planned: subprocess.CompletedProcess[str] = plan(
        project_dir=project_dir, args=("--full-refresh", "--select", "downstream")
    )
    rebuilt: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--full-refresh", "--select", "downstream"),
        project_dir=project_dir,
    )

    assert planned.returncode == 0, planned.stdout + planned.stderr
    assert all(fragment in planned.stdout for fragment in test_case.expected_plan_fragments), (
        planned.stdout
    )
    assert rebuilt.returncode == 0, rebuilt.stdout + rebuilt.stderr
    assert relation_rows(project_dir=project_dir, relation="downstream") == before


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="downstream references rewritten by sqb rename are not a query change",
            expected_plan_fragments=("Renamed (1)", "down_merge           merge (timestamp)"),
            unexpected_plan_fragments=("Query changed", "full rebuild", "S203"),
            expected_reasons={
                "upstream_renamed": "renamed",
                "upstream_v": "no_change",
                "down_merge": "normal_incremental",
                "downstream_mb": "normal_incremental",
            },
            expected_rows={"down_merge": _REPLAYED_TAIL, "downstream_mb": _REPLAYED_TAIL},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_upstream_when_downstream_refs_are_rewritten_then_downstream_continues_forward(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(
        tmp_path=tmp_path, extra_files=direct_reference_merge_files()
    )
    mark_first_day(project_dir=project_dir, relations=("down_merge", "downstream_mb"))
    rename_upstream_with_cli(project_dir=project_dir, extra_sql="")

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    assert models["upstream_v"]["action"] == "create_view"
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)
    rebuilt: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)
    assert (rebuilt["down_merge"]["reason"], rebuilt["upstream_v"]["reason"]) == (
        "normal_incremental",
        "no_change",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="a real edit alongside rewritten references is a query change",
            expected_plan_fragments=("Query changed (1)", "down_merge           full rebuild"),
            unexpected_plan_fragments=("S203",),
            expected_reasons={"down_merge": "query_changed"},
            expected_rows={},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_upstream_and_real_downstream_edit_when_planning_then_query_change_applies(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_event_project(
        tmp_path=tmp_path, extra_files=direct_reference_merge_files()
    )
    rename_upstream_with_cli(project_dir=project_dir, extra_sql="")
    edit_direct_reference_merge(project_dir=project_dir)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    assert models["down_merge"]["action"] == "create_table"


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="upstream column added to a star downstream evolves it forward",
            expected_plan_fragments=("Schema changed (1)", "down                 continue forward"),
            unexpected_plan_fragments=("full rebuild", "S203"),
            expected_reasons={"down": "schema_changed"},
            expected_rows={
                "down": [(1, date(2026, 9, 1), None), (1, date(2026, 9, 1), "web")],
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_upstream_column_added_when_downstream_selects_star_then_no_replay(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_star_project(
        tmp_path=tmp_path, downstream_config="  full_refresh false,\n"
    )
    add_upstream_column(project_dir=project_dir)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    assert models["down"]["backfill"] == {"action": "forward", "duration": None}
    build_and_assert_rows(project_dir=project_dir, expected_rows=test_case.expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        OwnChangePlanBuildTestCase(
            description="own contract column type change follows replay_on_change full",
            expected_plan_fragments=("down                 full rebuild",),
            unexpected_plan_fragments=("S203",),
            expected_reasons={"down": "config_changed"},
            expected_rows={},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_own_declared_column_change_when_planning_then_own_replay_policy_applies(
    test_case: OwnChangePlanBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = build_star_project(
        tmp_path=tmp_path, downstream_config=ENFORCED_CONTRACT_CONFIG
    )
    declare_downstream_column_type(project_dir=project_dir)

    assert_plan_text(
        project_dir=project_dir,
        expected=test_case.expected_plan_fragments,
        unexpected=test_case.unexpected_plan_fragments,
    )
    models: dict[str, dict[str, Any]] = plan_models(project_dir=project_dir)

    assert {name: models[name]["reason"] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
    assert models["down"]["backfill"] == {"action": "full", "duration": None}


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
