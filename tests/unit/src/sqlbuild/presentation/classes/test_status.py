from __future__ import annotations

import io
import subprocess
import sys

import pytest

from sqlbuild._native import TransientStatusReporter
from tests.unit.src.sqlbuild.presentation.classes._test_types import (
    ProgressSpinnersDisabledTestCase,
    StatusReporterExitTestCase,
    StatusReporterOutputTestCase,
)
from tests.unit.src.sqlbuild.presentation.classes.helpers import (
    OPEN_NESTED_SPINNER_SCRIPT,
    OPEN_SPINNER_SCRIPT,
    RecordingStream,
    apply_no_progress_env,
    drive_status_reporter,
    persistent_lines,
    without_no_progress_env,
)


class _TtyStream(io.StringIO):
    def isatty(self) -> bool:
        return True


@pytest.mark.parametrize(
    "test_case",
    (
        ProgressSpinnersDisabledTestCase(
            description="unset env keeps spinners enabled",
            env_value=None,
            expected_disabled=False,
        ),
        ProgressSpinnersDisabledTestCase(
            description="truthy '1' disables spinners",
            env_value="1",
            expected_disabled=True,
        ),
        ProgressSpinnersDisabledTestCase(
            description="truthy 'true' is case-insensitive and trimmed",
            env_value="  TRUE  ",
            expected_disabled=True,
        ),
        ProgressSpinnersDisabledTestCase(
            description="truthy 'on' disables spinners",
            env_value="on",
            expected_disabled=True,
        ),
        ProgressSpinnersDisabledTestCase(
            description="falsey '0' keeps spinners enabled",
            env_value="0",
            expected_disabled=False,
        ),
        ProgressSpinnersDisabledTestCase(
            description="arbitrary value keeps spinners enabled",
            env_value="maybe",
            expected_disabled=False,
        ),
        ProgressSpinnersDisabledTestCase(
            description="empty value keeps spinners enabled",
            env_value="",
            expected_disabled=False,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_no_progress_env_when_building_status_on_tty_then_toggles_spinner(
    test_case: ProgressSpinnersDisabledTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    apply_no_progress_env(monkeypatch=monkeypatch, env_value=test_case.env_value)
    stream: _TtyStream = _TtyStream()

    reporter: TransientStatusReporter = TransientStatusReporter(stream=stream, use_color=False)
    reporter.start("Working...")
    reporter.close()

    assert ("Working...\n" in stream.getvalue()) == test_case.expected_disabled


@pytest.mark.parametrize(
    "test_case",
    (
        StatusReporterOutputTestCase(
            description="plain stream writes every message as a muted line",
            use_color=True,
            tty=False,
            expected_persistent_lines=(
                "\x1b[2mDiscovering project...\x1b[0m",
                "\x1b[2mCompiling project graph...\x1b[0m",
                "\x1b[32m✓\x1b[0m Project graph compiled  \x1b[2m(0.1s)\x1b[0m",
                "",
                "\x1b[2mResolving tests...\x1b[0m",
                "\x1b[32m✓\x1b[0m 2 tests prepared",
                "",
                "\x1b[2mFormatting SQL...\x1b[0m",
                "Formatting SQL  ERROR",
                "styled",
                "",
            ),
        ),
        StatusReporterOutputTestCase(
            description="terminal erases live status and keeps only completions",
            use_color=False,
            tty=True,
            expected_persistent_lines=(
                "✓ Project graph compiled  (0.1s)",
                "",
                "✓ 2 tests prepared",
                "",
                "Formatting SQL  ERROR",
                "styled",
                "",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_status_sequence_when_reporting_then_writes_expected_persistent_lines(
    test_case: StatusReporterOutputTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_NO_PROGRESS", raising=False)
    stream: RecordingStream = RecordingStream(events=[], tty=test_case.tty)
    reporter: TransientStatusReporter = TransientStatusReporter(
        stream=stream, use_color=test_case.use_color
    )

    drive_status_reporter(reporter)

    assert persistent_lines(stream.getvalue()) == test_case.expected_persistent_lines


@pytest.mark.parametrize(
    "test_case",
    (
        StatusReporterExitTestCase(
            description="an open spinner is stopped before the interpreter exits",
            script=OPEN_SPINNER_SCRIPT,
            expected_returncode=0,
        ),
        StatusReporterExitTestCase(
            description="an unclosed nested progress spinner is stopped before finalization",
            script=OPEN_NESTED_SPINNER_SCRIPT,
            expected_returncode=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_open_spinner_when_process_exits_then_exits_cleanly(
    test_case: StatusReporterExitTestCase,
) -> None:
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        (sys.executable, "-c", test_case.script),
        capture_output=True,
        text=True,
        check=False,
        env=without_no_progress_env(),
        timeout=60,
    )

    assert completed.returncode == test_case.expected_returncode, completed.stderr
