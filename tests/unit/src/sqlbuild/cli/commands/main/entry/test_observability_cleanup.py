from __future__ import annotations

import threading
from collections.abc import Callable
from contextvars import Token
from io import StringIO
from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import (
    INTEGRATION_RESULT_PATH_ENV,
    CliNamespace,
    NativeProgressProjector,
    cli_observability_scope,
    current_native_progress_projector,
)
from sqlbuild.observability import OperationLifecycle
from tests.unit.src.sqlbuild.cli.commands.main.entry._test_types import (
    MachineProgressRoutingCase,
    NoExporterFastPathTestCase,
    ObservabilityCleanupCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        ObservabilityCleanupCase(
            description="command failure propagates and restores the outer projector",
            expected_original_error="original command failure",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_command_failure_when_scope_exits_then_original_error_and_projector_survive(
    tmp_path: Path,
    test_case: ObservabilityCleanupCase,
) -> None:
    args: CliNamespace = CliNamespace()
    outer: NativeProgressProjector = NativeProgressProjector(stream=StringIO(), use_color=False)
    outer_token: Token[NativeProgressProjector | None] = outer.install()

    with pytest.raises(ValueError, match=test_case.expected_original_error):
        with cli_observability_scope(args=args, project_dir=tmp_path):
            assert current_native_progress_projector() is not outer
            raise ValueError(test_case.expected_original_error)

    assert current_native_progress_projector() is outer
    outer.restore(outer_token)


@pytest.mark.parametrize(
    "test_case",
    (NoExporterFastPathTestCase("no public sink skips providers and thread", 0),),
    ids=lambda case: case.description,
)
def test_given_no_public_sink_when_observability_runs_then_provider_import_is_skipped(
    test_case: NoExporterFastPathTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    marker_path: Path = tmp_path / "provider-imported"
    helper_marker_path: Path = tmp_path / "helper-imported"
    write_repo_files(
        tmp_path,
        {
            "python/providers/exploding.py": (
                "from pathlib import Path\n"
                f"Path({str(marker_path)!r}).write_text('imported', encoding='utf-8')\n"
                "raise RuntimeError('provider must not import')\n"
            ),
            "python/event_sinks/_private.py": "raise RuntimeError('private sink imported')\n",
            "python/event_sinks/helpers.py": (
                "from pathlib import Path\n"
                f"Path({str(helper_marker_path)!r}).write_text('imported', encoding='utf-8')\n"
                "def encode(value):\n    return value\n"
            ),
        },
    )
    args: CliNamespace = CliNamespace()
    before_threads: int = sum(
        map(
            lambda thread: thread.name.startswith("sqlbuild-event-exporter-"),
            threading.enumerate(),
        )
    )

    with cli_observability_scope(args=args, project_dir=tmp_path):
        with OperationLifecycle(operation_kind="project", operation_name="project_compile"):
            pass
        active_threads: int = sum(
            map(
                lambda thread: thread.name.startswith("sqlbuild-event-exporter-"),
                threading.enumerate(),
            )
        )

    assert not marker_path.exists()
    assert helper_marker_path.exists()
    assert active_threads - before_threads == test_case.expected_thread_delta


@pytest.mark.parametrize(
    "test_case",
    (
        MachineProgressRoutingCase(
            description="json output file retains human stdout progress",
            json=False,
            json_output=Path("target/result.json"),
            expected_stdout_fragment="Project compile  START",
            expected_stderr_fragment="",
        ),
        MachineProgressRoutingCase(
            description="json mode routes human progress to stderr",
            json=True,
            json_output=None,
            expected_stdout_fragment="",
            expected_stderr_fragment="Project compile  START",
        ),
        MachineProgressRoutingCase(
            description="json mode with output file still routes progress to stderr",
            json=True,
            json_output=Path("target/result.json"),
            expected_stdout_fragment="",
            expected_stderr_fragment="Project compile  START",
        ),
        MachineProgressRoutingCase(
            description="plan json mode keeps stdout free of progress",
            json=True,
            json_output=None,
            expected_stdout_fragment="",
            expected_stderr_fragment="Project compile  START",
            command="plan",
            expected_stdout_excludes="Project compile",
        ),
        MachineProgressRoutingCase(
            description="dbt plan json flag routes progress to stderr",
            json=False,
            json_output=None,
            expected_stdout_fragment="",
            expected_stderr_fragment="Project compile  START",
            command="dbt",
            dbt_args=("--json", "--select", "orders"),
            expected_stdout_excludes="Project compile",
        ),
        MachineProgressRoutingCase(
            description="dbt plan without json keeps human stdout progress",
            json=False,
            json_output=None,
            expected_stdout_fragment="Project compile  START",
            expected_stderr_fragment="",
            command="dbt",
            dbt_args=("--select", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_json_routing_options_when_operation_runs_then_human_stream_contract_is_preserved(
    tmp_path: Path,
    test_case: MachineProgressRoutingCase,
    capsys: pytest.CaptureFixture[str],
) -> None:
    args: CliNamespace = CliNamespace()
    args.json = test_case.json
    args.json_output = test_case.json_output
    args.command = test_case.command
    args.dbt_args = list(test_case.dbt_args)

    with cli_observability_scope(args=args, project_dir=tmp_path):
        with OperationLifecycle(operation_kind="project", operation_name="project_compile"):
            pass

    captured: CaptureResult[str] = capsys.readouterr()
    assert test_case.expected_stdout_fragment in captured.out
    assert test_case.expected_stderr_fragment in captured.err
    assert test_case.expected_stdout_excludes not in captured.out


@pytest.mark.parametrize(
    "test_case",
    (
        MachineProgressRoutingCase(
            description="integration result file retains human stdout progress",
            json=False,
            json_output=None,
            expected_stdout_fragment="Project compile  START",
            expected_stderr_fragment="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_integration_result_file_when_operation_runs_then_human_progress_uses_stdout(
    tmp_path: Path,
    test_case: MachineProgressRoutingCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    args: CliNamespace = CliNamespace()
    args.event_output = tmp_path / "events.jsonl"
    monkeypatch.setenv(INTEGRATION_RESULT_PATH_ENV, str(tmp_path / "integration.jsonl"))

    with cli_observability_scope(args=args, project_dir=tmp_path):
        with OperationLifecycle(operation_kind="project", operation_name="project_compile"):
            pass

    captured: CaptureResult[str] = capsys.readouterr()
    assert test_case.expected_stdout_fragment in captured.out
    assert captured.err == test_case.expected_stderr_fragment
