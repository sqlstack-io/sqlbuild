from __future__ import annotations

import sys
from pathlib import Path

import pytest

from sqlbuild._native import (
    TransientLineCoordinator,
    discover_project_inputs,
    shared_transient_line_coordinator,
)
from tests.unit.src.sqlbuild.presentation.classes._test_types import (
    ConsoleLineTestCase,
    TransientLineCoordinatorTestCase,
)
from tests.unit.src.sqlbuild.presentation.classes.helpers import (
    RecordingSpinnerOwner,
    RecordingStream,
)


@pytest.mark.parametrize(
    "test_case",
    (
        TransientLineCoordinatorTestCase(
            description="persistent line on the live terminal clears then redraws the spinner",
            persistent_stream_is_tty=True,
            expected_events=("clear", "write:Warehouse connected\n", "redraw"),
        ),
        TransientLineCoordinatorTestCase(
            description="redirected output does not disturb the terminal spinner",
            persistent_stream_is_tty=False,
            expected_events=("write:Warehouse connected\n",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_live_spinner_when_writing_persistent_line_then_line_never_joins_spinner(
    test_case: TransientLineCoordinatorTestCase,
) -> None:
    events: list[str] = []
    coordinator: TransientLineCoordinator = TransientLineCoordinator()
    owner: RecordingSpinnerOwner = RecordingSpinnerOwner(events=events)
    coordinator.claim(stream=RecordingStream(events=[], tty=True), owner=owner)

    coordinator.write_persistent(
        stream=RecordingStream(events=events, tty=test_case.persistent_stream_is_tty),
        text="Warehouse connected\n",
    )

    assert tuple(events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    (
        TransientLineCoordinatorTestCase(
            description="released spinner is neither cleared nor redrawn",
            persistent_stream_is_tty=True,
            expected_events=("write:Warehouse connected\n",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_released_spinner_when_writing_persistent_line_then_writes_directly(
    test_case: TransientLineCoordinatorTestCase,
) -> None:
    events: list[str] = []
    coordinator: TransientLineCoordinator = TransientLineCoordinator()
    owner: RecordingSpinnerOwner = RecordingSpinnerOwner(events=events)
    coordinator.claim(stream=RecordingStream(events=[], tty=True), owner=owner)
    coordinator.release(owner=owner)

    coordinator.write_persistent(
        stream=RecordingStream(events=events, tty=test_case.persistent_stream_is_tty),
        text="Warehouse connected\n",
    )

    assert tuple(events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    (
        TransientLineCoordinatorTestCase(
            description="cleared spinner is erased once and never redrawn",
            persistent_stream_is_tty=True,
            expected_events=("clear", "write:error[S301]: failed\n"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_live_spinner_when_clearing_active_line_then_later_writes_do_not_redraw_it(
    test_case: TransientLineCoordinatorTestCase,
) -> None:
    events: list[str] = []
    coordinator: TransientLineCoordinator = TransientLineCoordinator()
    owner: RecordingSpinnerOwner = RecordingSpinnerOwner(events=events)
    coordinator.claim(stream=RecordingStream(events=[], tty=True), owner=owner)

    coordinator.clear_active()
    coordinator.clear_active()
    coordinator.write_persistent(
        stream=RecordingStream(events=events, tty=test_case.persistent_stream_is_tty),
        text="error[S301]: failed\n",
    )

    assert tuple(events) == test_case.expected_events


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    (
        ConsoleLineTestCase(
            description="a native console line clears and then redraws the live spinner",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\n[settings]\ncolumn_contract_mode = "implicit"\n'
            ),
            expected_events=(
                "clear",
                "write:Warning: settings.column_contract_mode in sqlbuild_project.toml is "
                "deprecated; replace it with settings.check_declared_columns = true\n",
                "redraw",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_live_spinner_when_native_code_writes_a_console_line_then_line_never_joins_spinner(
    test_case: ConsoleLineTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    events: list[str] = []
    owner: RecordingSpinnerOwner = RecordingSpinnerOwner(events=events)
    coordinator: TransientLineCoordinator = shared_transient_line_coordinator()
    _ = (tmp_path / "sqlbuild_project.toml").write_text(test_case.project_toml, encoding="utf-8")
    monkeypatch.setattr(sys, "stderr", RecordingStream(events=events, tty=True))
    coordinator.claim(stream=RecordingStream(events=[], tty=True), owner=owner)

    _ = discover_project_inputs(project_dir=tmp_path)
    coordinator.release(owner=owner)

    assert tuple(events) == test_case.expected_events
