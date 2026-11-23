"""Real CLI coverage for how a scenario run stops on SIGINT and SIGTERM."""

from __future__ import annotations

import signal
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    SignalledScenarioTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    signalled_scenario_run,
    write_endless_scenario_project,
)

_STOP_NOTICE: str = "Interrupted; cleaning up running scenarios..."


@pytest.mark.parametrize(
    "test_case",
    [
        SignalledScenarioTestCase(
            description="Ctrl+C cancels the running statement",
            sent_signal=signal.SIGINT,
            expected_exit_code=130,
            expected_last_line="error: interrupted (Ctrl+C)",
        ),
        SignalledScenarioTestCase(
            description="SIGTERM cancels the running statement the same way",
            sent_signal=signal.SIGTERM,
            expected_exit_code=143,
            expected_last_line="error: terminated by SIGTERM",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_scenario_when_signalled_then_its_statement_is_cancelled_promptly(
    test_case: SignalledScenarioTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_endless_scenario_project(tmp_path / "scenario_signals")

    exit_code, output, stop_seconds = signalled_scenario_run(
        project_dir=project_dir, sent_signal=test_case.sent_signal
    )

    assert exit_code == test_case.expected_exit_code, output
    assert stop_seconds < 15, output
    assert output.count(_STOP_NOTICE) == 1, output
    assert output.splitlines()[-1] == test_case.expected_last_line, output
