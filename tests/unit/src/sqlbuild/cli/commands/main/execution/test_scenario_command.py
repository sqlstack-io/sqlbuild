from __future__ import annotations

from contextvars import Token
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompileInputError,
    NativeProgressProjector,
    ScenarioTestCommandRequest,
    run_scenario_test_command,
)
from sqlbuild.observability import EventDispatcher, LifecycleEvent, dispatcher_scope
from tests.unit.src.sqlbuild.cli.commands.main.execution._test_types import (
    ScenarioCompilePresentationTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.execution.helpers import (
    INVALID_SCENARIO_SQL,
    VALID_SCENARIO_SQL,
    ScenarioProgressStream,
    configure_native_scenario_command,
)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCompilePresentationTestCase(
            "non-tty canonical compile owns rows",
            False,
            "Project compile  START",
            "Compiling project...",
            "operation_completed",
        ),
        ScenarioCompilePresentationTestCase(
            "tty planning spinner owns rows",
            True,
            "\u2713 Project compiled",
            "Project compile  START",
            "operation_completed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_compile_when_running_then_presentation_has_one_owner(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: ScenarioCompilePresentationTestCase,
) -> None:
    stream: ScenarioProgressStream = ScenarioProgressStream(tty=test_case.tty)
    configure_native_scenario_command(
        monkeypatch=monkeypatch,
        stream=stream,
        scenario_sql=VALID_SCENARIO_SQL,
        project_dir=tmp_path,
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)
    token: Token[NativeProgressProjector | None] = projector.install()

    try:
        with dispatcher_scope(dispatcher):
            exit_code: int = run_scenario_test_command(
                request=ScenarioTestCommandRequest(project_dir=tmp_path)
            )
    finally:
        projector.restore(token)

    assert exit_code == 0
    assert test_case.expected_fragment in stream.getvalue()
    assert test_case.unexpected_fragment not in stream.getvalue()
    assert "PASS=0  FAIL=0  TOTAL=0" in stream.getvalue()
    operations: list[object] = [event.payload.get("operation_name") for event in events]
    compile_start: int = operations.index("project_compile")
    assert operations.count("project_compile") == 2
    assert tuple(event.event_type for event in events[compile_start : compile_start + 3]) == (
        "operation_started",
        "invocation_phase",
        test_case.expected_terminal,
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCompilePresentationTestCase(
            "non-tty compile failure",
            False,
            "Project compile  FAIL",
            "Compiled project.",
            "operation_failed",
        ),
        ScenarioCompilePresentationTestCase(
            "tty compile failure cleans spinner",
            True,
            "",
            "Project compile  START",
            "operation_failed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_compile_failure_when_running_then_lifecycle_fails_and_progress_closes(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: ScenarioCompilePresentationTestCase,
) -> None:
    stream: ScenarioProgressStream = ScenarioProgressStream(tty=test_case.tty)
    configure_native_scenario_command(
        monkeypatch=monkeypatch,
        stream=stream,
        scenario_sql=INVALID_SCENARIO_SQL,
        project_dir=tmp_path,
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)
    token: Token[NativeProgressProjector | None] = projector.install()

    try:
        with dispatcher_scope(dispatcher):
            with pytest.raises(CompileInputError, match="must declare fixture CTEs"):
                run_scenario_test_command(request=ScenarioTestCommandRequest(project_dir=tmp_path))
    finally:
        projector.restore(token)

    assert test_case.expected_fragment in stream.getvalue()
    assert test_case.unexpected_fragment not in stream.getvalue()
    assert tuple(event.event_type for event in events[-2:]) == (
        "operation_started",
        test_case.expected_terminal,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
