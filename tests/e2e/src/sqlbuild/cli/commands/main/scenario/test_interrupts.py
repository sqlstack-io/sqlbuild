"""E2E tests for Ctrl-C during scenario test, local replay, and capture."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioInterruptE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    InterruptedScenarioRun,
    build_interrupt_project_files,
    interrupt_scenario_run,
    list_scenario_relation_names,
    scenario_relation_names_if_present,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_STOP_NOTICE: str = "Interrupted; cleaning up running scenarios..."
_ABANDON_NOTICE: str = "Interrupted again; skipped scenario cleanup."
_NOTICES: tuple[str, ...] = (_STOP_NOTICE, _ABANDON_NOTICE)
_DEBUG: tuple[str, ...] = ("--debug",)
"""`sqb scenario test` prints the START row a test waits for only with --debug."""


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioInterruptE2ETestCase(
            description="sequential run cancels the in-flight statement",
            args=("test", "--concurrency", "1", "--json"),
            trigger="order_totals START",
            global_args=_DEBUG,
            long_model=True,
        ),
        ScenarioInterruptE2ETestCase(
            description="concurrent run cancels every in-flight statement",
            args=("test", "--concurrency", "4", "--json"),
            trigger="order_totals START",
            global_args=_DEBUG,
            long_model=True,
        ),
        ScenarioInterruptE2ETestCase(
            description="capture cancels the in-flight fixture statement",
            args=("capture", "slow_0"),
            trigger="slow_0 START",
            trigger_stream="stdout",
            long_fixture=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_long_statement_when_interrupted_then_stops_promptly_and_cleans_up(
    test_case: ScenarioInterruptE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_interrupt",
        repo_files=build_interrupt_project_files(
            scenario_count=4,
            long_model=test_case.long_model,
            long_fixture=test_case.long_fixture,
            slow_hook=False,
        ),
    )

    result: InterruptedScenarioRun = interrupt_scenario_run(
        project_dir=project_dir,
        args=test_case.args,
        trigger=test_case.trigger,
        trigger_stream=test_case.trigger_stream,
        interrupt_count=test_case.interrupt_count,
        global_args=test_case.global_args,
    )

    assert result.returncode != 0, result.output
    assert result.stop_seconds < test_case.expected_max_stop_seconds, result.output
    assert tuple(result.output.count(notice) for notice in _NOTICES) == (
        test_case.expected_notice_counts
    ), result.output
    assert "expected  order_totals" not in result.output
    assert (
        scenario_relation_names_if_present(db_path=project_dir / "scenario_interrupt.duckdb") == ()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioInterruptE2ETestCase(
            description="local replay cancels the in-flight statement with clean json stdout",
            args=("test", "--run-on", "local", "--json"),
            trigger="order_totals START",
            global_args=_DEBUG,
            long_model=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_replay_when_interrupted_then_stops_promptly_with_clean_json_stdout(
    test_case: ScenarioInterruptE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_interrupt",
        repo_files=build_interrupt_project_files(
            scenario_count=4,
            long_model=test_case.long_model,
            long_fixture=test_case.long_fixture,
            slow_hook=False,
        ),
    )
    capture: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "capture"), project_dir=project_dir
    )

    result: InterruptedScenarioRun = interrupt_scenario_run(
        project_dir=project_dir,
        args=test_case.args,
        trigger=test_case.trigger,
        trigger_stream=test_case.trigger_stream,
        interrupt_count=test_case.interrupt_count,
        global_args=test_case.global_args,
    )

    assert capture.returncode == 0, capture.stdout + capture.stderr
    assert result.returncode != 0, result.output
    assert result.stdout == ""
    assert result.stop_seconds < test_case.expected_max_stop_seconds, result.output
    assert tuple(result.output.count(notice) for notice in _NOTICES) == (
        test_case.expected_notice_counts
    ), result.output


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioInterruptE2ETestCase(
            description="second interrupt abandons cleanup of busy workers",
            args=("test", "--concurrency", "4", "--json"),
            trigger="order_totals START",
            global_args=_DEBUG,
            slow_hook_seconds=60.0,
            interrupt_count=2,
            expected_notice_counts=(1, 1),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repeat_interrupt_when_workers_are_busy_then_exits_and_next_run_cleans_up(
    test_case: ScenarioInterruptE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_interrupt",
        repo_files=build_interrupt_project_files(
            scenario_count=4,
            long_model=test_case.long_model,
            long_fixture=test_case.long_fixture,
            slow_hook=True,
        ),
    )
    db_path: Path = project_dir / "scenario_interrupt.duckdb"

    result: InterruptedScenarioRun = interrupt_scenario_run(
        project_dir=project_dir,
        args=test_case.args,
        trigger=test_case.trigger,
        trigger_stream=test_case.trigger_stream,
        interrupt_count=test_case.interrupt_count,
        global_args=test_case.global_args,
        hook_seconds=test_case.slow_hook_seconds,
    )
    leftovers: tuple[str, ...] = list_scenario_relation_names(db_path=db_path)
    rerun: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert result.returncode != 0, result.output
    assert result.stdout == ""
    assert result.stop_seconds < test_case.expected_max_stop_seconds, result.output
    assert tuple(result.output.count(notice) for notice in _NOTICES) == (
        test_case.expected_notice_counts
    ), result.output
    assert leftovers != ()
    assert rerun.returncode == 0, rerun.stdout + rerun.stderr
    assert list_scenario_relation_names(db_path=db_path) == ()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
