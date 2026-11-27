"""Tests for planning progress output."""

from __future__ import annotations

from io import StringIO

import pytest

from sqlbuild._native import (
    CliStyle,
    PlanningProgressReporter,
)
from tests.unit.src.sqlbuild.cli.commands.shared._helpers._test_types import (
    PlanningFinishTestCase,
    PlanningPhaseEndTestCase,
    PlanningProgressTestCase,
)
from tests.unit.src.sqlbuild.presentation.classes.helpers import (
    RecordingStream,
    persistent_lines,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanningProgressTestCase(
            description="writes each planning message on its own line",
            messages=("Inspecting warehouse state...", "Generating plan..."),
            expected_output="Inspecting warehouse state...\nGenerating plan...\n",
        ),
        PlanningProgressTestCase(
            description="dims planning messages when color is enabled",
            messages=("Inspecting warehouse state...",),
            expected_output=f"{CliStyle(use_color=True).muted('Inspecting warehouse state...')}\n",
            use_color=True,
        ),
        PlanningProgressTestCase(
            description="completes cloned status messages before later output",
            messages=(
                "Applying clone plan...",
                "Applied clone plan. (0.10s)",
                "sqb clone",
            ),
            expected_output=("Applying clone plan...\n✓ Clone plan applied  (0.10s)\nsqb clone\n"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planning_messages_when_reporting_then_writes_expected_output(
    test_case: PlanningProgressTestCase,
) -> None:
    stream: StringIO = StringIO()
    reporter: PlanningProgressReporter = PlanningProgressReporter(
        stream=stream, use_color=test_case.use_color
    )

    message: str
    for message in test_case.messages:
        reporter.on_progress(message)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        PlanningPhaseEndTestCase(
            description="failed watermark ends its phase before later output",
            messages=(
                "Inspecting runtime cursor watermark (1/1): dev.orders.updated_at [max]...",
                "Failed runtime cursor watermark (1/1): dev.orders.updated_at [max] (0.10s): boom",
            ),
            expected_lines=(
                "Failed runtime cursor watermark (1/1): dev.orders.updated_at [max] (0.10s): boom",
                "error[E001]: later output",
                "",
            ),
        ),
        PlanningPhaseEndTestCase(
            description="prepared clone plan ends its phase before later output",
            messages=("Preparing clone plan...", "Prepared clone plan. (0.10s)"),
            expected_lines=("✓ Clone plan prepared  (0.10s)", "error[E001]: later output", ""),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_phase_ending_message_on_terminal_when_reporting_then_spinner_stops(
    test_case: PlanningPhaseEndTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_NO_PROGRESS", raising=False)
    stream: RecordingStream = RecordingStream(events=[], tty=True)
    reporter: PlanningProgressReporter = PlanningProgressReporter(stream=stream, use_color=False)

    message: str
    for message in test_case.messages:
        reporter.on_progress(message)
    stream.write("error[E001]: later output\n")

    assert persistent_lines(stream.getvalue()) == test_case.expected_lines


@pytest.mark.parametrize(
    "test_case",
    [
        PlanningFinishTestCase(
            description="closes active planning status with blank line before later output",
            messages_before_finish=("Applying clone plan...",),
            blank_line_after=True,
            messages_after_finish=("sqb clone",),
            expected_output="Applying clone plan...\n\nsqb clone\n",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_active_planning_status_when_finishing_then_closes_with_blank_line(
    test_case: PlanningFinishTestCase,
) -> None:
    stream: StringIO = StringIO()
    reporter: PlanningProgressReporter = PlanningProgressReporter(stream=stream, use_color=False)

    message: str
    for message in test_case.messages_before_finish:
        reporter.on_progress(message)
    reporter.finish(blank_line_after=test_case.blank_line_after)
    for message in test_case.messages_after_finish:
        reporter.on_progress(message)

    assert stream.getvalue() == test_case.expected_output
