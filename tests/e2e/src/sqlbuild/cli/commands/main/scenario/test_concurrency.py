"""E2E tests for concurrent scenario execution, timings, and cleanup."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioCliE2ETestCase,
    ScenarioConcurrencyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_scenario_project_files,
    list_scenario_relation_names,
    scenario_result_lines,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SCENARIO_ROW: re.Pattern[str] = re.compile(r"^order_totals_pass\s+PASS  \d+\.\d\ds$", re.M)
_SUMMARY: re.Pattern[str] = re.compile(r"PASS=2  FAIL=1  TOTAL=3  \(\d+\.\d\ds\)")


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioConcurrencyE2ETestCase(
            description="cli concurrency runs scenarios concurrently",
            command_args=("--concurrency", "4"),
            settings_config="",
            expected_header="(concurrency: 4)",
        ),
        ScenarioConcurrencyE2ETestCase(
            description="project concurrency setting is honoured",
            command_args=(),
            settings_config="\n[settings]\nconcurrency = 3\n",
            expected_header="(concurrency: 3)",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_concurrency_when_running_scenarios_then_results_match_sequential_run(
    test_case: ScenarioConcurrencyE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    sequential_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="sequential", repo_files=repo_files
    )
    repo_files["sqlbuild_project.toml"] += test_case.settings_config
    concurrent_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="concurrent", repo_files=repo_files
    )

    sequential: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--concurrency", "1"),
        project_dir=sequential_dir,
    )
    concurrent: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", *test_case.command_args),
        project_dir=concurrent_dir,
    )

    assert sequential.returncode == 1, sequential.stdout + sequential.stderr
    assert concurrent.returncode == 1, concurrent.stdout + concurrent.stderr
    assert "(concurrency: 1)" in sequential.stdout
    assert test_case.expected_header in concurrent.stdout
    assert scenario_result_lines(concurrent.stdout) == scenario_result_lines(sequential.stdout)
    assert "expected order_totals: actual=1 expected=1 mismatched=1" in concurrent.stdout
    assert _SCENARIO_ROW.search(concurrent.stdout) is not None, concurrent.stdout
    assert _SUMMARY.search(concurrent.stdout) is not None, concurrent.stdout
    assert list_scenario_relation_names(db_path=concurrent_dir / "scenario_demo.duckdb") == ()


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCliE2ETestCase(
            description="json output reports per-scenario and total durations",
            command=("--no-color", "scenario", "test", "--concurrency", "4", "--json"),
            expected_exit_code=1,
            expected_stderr_fragments=("(concurrency: 4)",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_json_output_when_running_scenarios_concurrently_then_reports_durations(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scenario_json", repo_files=build_scenario_project_files()
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    scenarios: list[dict[str, object]] = cast(list[dict[str, object]], payload["scenarios"])
    assert sorted(str(scenario["name"]) for scenario in scenarios) == [
        "order_totals_fail",
        "order_totals_pass",
        "orders_assert_pass",
    ]
    assert all(isinstance(scenario["duration_ms"], int) for scenario in scenarios)
    summary: dict[str, object] = cast(dict[str, object], payload["summary"])
    assert (summary["pass_count"], summary["fail_count"], summary["total_count"]) == (2, 1, 3)
    assert isinstance(summary["duration_ms"], int)
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCliE2ETestCase(
            description="retained table replaced by a view model is dropped as a table",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_retained_artifact_of_another_type_when_rerunning_then_replaces_and_cleans_up(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scenario_retype", repo_files=build_scenario_project_files()
    )
    retained: subprocess.CompletedProcess[str] = run_sqb(
        command=(*test_case.command, "--retain"), project_dir=project_dir
    )
    assert retained.returncode == 0, retained.stdout + retained.stderr
    orders_model: Path = project_dir / "sql/models/orders.sql"
    orders_model.write_text(
        orders_model.read_text(encoding="utf-8").replace("materialized table", "materialized view"),
        encoding="utf-8",
    )

    rerun: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert rerun.returncode == test_case.expected_exit_code, rerun.stdout + rerun.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in rerun.stdout
    assert list_scenario_relation_names(db_path=project_dir / "scenario_demo.duckdb") == ()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
