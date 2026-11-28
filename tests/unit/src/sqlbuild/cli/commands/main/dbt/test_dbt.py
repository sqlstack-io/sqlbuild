from __future__ import annotations

import sys
from collections.abc import Callable
from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import (
    DbtInteropCommand,
    DbtInteropExecutionRequest,
    DbtInteropPlan,
    run_dbt_command,
)
from tests.unit.src.sqlbuild.cli.commands.main.dbt._test_types import (
    DbtAutoInitTestCase,
    DbtDebugWrapperTestCase,
    DbtExecutionWrapperTestCase,
    DbtPlanProgressTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.dbt.helpers import (
    MINIMAL_SQLBUILD_PROJECT,
    build_empty_dbt_plan,
    prepare_dbt_auto_init_dirs,
    read_fake_dbt_args,
    write_duckdb_sqlbuild_project,
    write_fake_dbt_executable,
    write_minimal_sqlbuild_project,
)
from tests.unit.src.sqlbuild.integrations.dbt.profile.helpers import write_duckdb_dbt_workspace


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanProgressTestCase(
            description="human output writes progress and plan to stdout",
            args=(),
            expected_stdout_fragments=(
                "Compiling dbt project...",
                "dbt interop plan generated",
                "Plan ready",
            ),
            expected_stderr_fragments=(),
        ),
        DbtPlanProgressTestCase(
            description="json output writes progress to stderr and json to stdout",
            args=("--json",),
            expected_stdout_fragments=('"command": "plan"',),
            expected_stderr_fragments=(
                "Compiling dbt project...",
                "dbt interop plan generated",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_plan_when_running_then_writes_progress_to_expected_stream(
    test_case: DbtPlanProgressTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    def plan_dbt_interop_from_project(
        *,
        project_dir: Path,
        args: tuple[str, ...],
        on_progress: Callable[[str], None],
        progress_stream: object,
        use_color: bool,
    ) -> DbtInteropPlan:
        del project_dir, args, progress_stream, use_color
        assert callable(on_progress)
        on_progress("Compiling dbt project...")
        on_progress("Generated dbt interop plan. (0.01s)")
        return build_empty_dbt_plan()

    monkeypatch.setattr(
        "sqlbuild._native.plan_dbt_interop_from_project",
        plan_dbt_interop_from_project,
    )
    write_minimal_sqlbuild_project(tmp_path)
    exit_code: int = run_dbt_command(
        command=DbtInteropCommand.PLAN,
        project_dir=tmp_path,
        args=test_case.args,
        no_color=True,
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in captured.out
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        DbtExecutionWrapperTestCase(
            description="dbt run strips the local verbose flag before execution",
            command_name="run",
            args=("--verbose", "--select", "tag:nightly"),
            expected_forwarded_args=("--select", "tag:nightly"),
            expected_progress_stream=sys.stdout,
        ),
        DbtExecutionWrapperTestCase(
            description="dbt build keeps human output on stdout",
            command_name="build",
            args=("--select", "tag:nightly"),
            expected_forwarded_args=("--select", "tag:nightly"),
            expected_progress_stream=sys.stdout,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_execution_command_when_running_then_routes_expected_stream_and_args(
    test_case: DbtExecutionWrapperTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured_calls: list[tuple[DbtInteropCommand, tuple[str, ...], object]] = []

    def execute_dbt_interop_from_project(request: DbtInteropExecutionRequest) -> int:
        captured_calls.append((request.command, request.args, request.progress_stream))
        return 0

    monkeypatch.setattr(
        "sqlbuild._native.execute_dbt_interop_from_project",
        execute_dbt_interop_from_project,
    )
    write_minimal_sqlbuild_project(tmp_path)

    exit_code: int = run_dbt_command(
        command=DbtInteropCommand(test_case.command_name),
        project_dir=tmp_path,
        args=test_case.args,
        no_color=True,
    )

    assert exit_code == 0
    assert captured_calls[0][1] == test_case.expected_forwarded_args
    assert captured_calls[0][2] is test_case.expected_progress_stream


@pytest.mark.parametrize(
    "test_case",
    [
        DbtDebugWrapperTestCase(
            description="debug runs dbt then SQLBuild diagnostics and skips SQLBuild connection",
            args=("--project-dir", "dbt_project", "--no-connection"),
            expected_dbt_args=("debug", "--project-dir"),
            expected_sqlbuild_no_connection=True,
            expected_exit_code=0,
            dbt_exit_code=0,
            expected_stderr_fragments=(
                "Running dbt debug...",
                "Running SQLBuild diagnostics...",
            ),
        ),
        DbtDebugWrapperTestCase(
            description="debug returns failure when dbt debug fails after SQLBuild diagnostics",
            args=(
                "--project-dir",
                "dbt_project",
            ),
            expected_dbt_args=("debug", "--project-dir"),
            expected_sqlbuild_no_connection=False,
            expected_exit_code=1,
            dbt_exit_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_debug_command_when_running_then_invokes_dbt_and_sqlbuild_debug(
    test_case: DbtDebugWrapperTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "dbt_project").mkdir()
    monkeypatch.setenv(
        "DBT_EXECUTABLE",
        str(write_fake_dbt_executable(tmp_path, exit_code=test_case.dbt_exit_code)),
    )
    write_duckdb_sqlbuild_project(tmp_path)

    exit_code: int = run_dbt_command(
        command=DbtInteropCommand.DEBUG,
        project_dir=tmp_path,
        args=test_case.args,
        no_color=True,
    )

    assert exit_code == test_case.expected_exit_code
    assert read_fake_dbt_args(tmp_path) == (
        *test_case.expected_dbt_args,
        str(tmp_path / "dbt_project"),
    )
    captured: CaptureResult[str] = capsys.readouterr()
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in captured.err
    assert ("skipped by --no-connection" in captured.out) is (
        test_case.expected_sqlbuild_no_connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DbtAutoInitTestCase(
            description="uses current SQLBuild project when config exists",
            has_current_sqlbuild_project=True,
            has_sibling_sqlbuild_project=False,
            dbt_args=("--select", "orders"),
            expected_init_called=False,
            expected_forwarded_project_dir_name="dbt_project",
            expected_request_dbt_project_dir_name=None,
            expected_request_profiles_dir=None,
            expected_request_target_name=None,
        ),
        DbtAutoInitTestCase(
            description="uses existing sibling SQLBuild twin when current project is dbt",
            has_current_sqlbuild_project=False,
            has_sibling_sqlbuild_project=True,
            dbt_args=("--select", "orders"),
            expected_init_called=False,
            expected_forwarded_project_dir_name="sqlbuild_project",
            expected_request_dbt_project_dir_name=None,
            expected_request_profiles_dir=None,
            expected_request_target_name=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_sqlbuild_project_when_running_dbt_command_then_uses_expected_project(
    test_case: DbtAutoInitTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    dbt_project_dir: Path = prepare_dbt_auto_init_dirs(test_case=test_case, tmp_path=tmp_path)
    forwarded_project_dirs: list[Path] = []

    def plan_dbt_interop_from_project(
        *,
        project_dir: Path,
        args: tuple[str, ...],
        on_progress: Callable[[str], None],
        progress_stream: object,
        use_color: bool,
    ) -> DbtInteropPlan:
        del args, on_progress, progress_stream, use_color
        forwarded_project_dirs.append(project_dir)
        return build_empty_dbt_plan()

    monkeypatch.setattr(
        "sqlbuild._native.plan_dbt_interop_from_project",
        plan_dbt_interop_from_project,
    )

    exit_code: int = run_dbt_command(
        command=DbtInteropCommand.PLAN,
        project_dir=dbt_project_dir,
        args=test_case.dbt_args,
        no_color=True,
    )

    assert exit_code == 0
    assert forwarded_project_dirs[0].name == test_case.expected_forwarded_project_dir_name
    assert (forwarded_project_dirs[0] / "sqlbuild_project.toml").read_text(encoding="utf-8") == (
        MINIMAL_SQLBUILD_PROJECT
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DbtAutoInitTestCase(
            description="creates sibling SQLBuild twin from typed dbt config flags",
            has_current_sqlbuild_project=False,
            has_sibling_sqlbuild_project=False,
            dbt_args=(
                "--project-dir",
                "dbt_project",
                "--profiles-dir",
                "profiles",
                "--target",
                "dev",
                "--select",
                "orders",
            ),
            expected_init_called=True,
            expected_forwarded_project_dir_name="sqlbuild_project",
            expected_request_dbt_project_dir_name="dbt_project",
            expected_request_profiles_dir="profiles",
            expected_request_target_name="dev",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_sqlbuild_twin_when_running_dbt_command_then_initializes_and_uses_twin(
    test_case: DbtAutoInitTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    dbt_project_dir: Path = prepare_dbt_auto_init_dirs(test_case=test_case, tmp_path=tmp_path)
    write_duckdb_dbt_workspace(dbt_project_dir)
    forwarded_project_dirs: list[Path] = []
    forwarded_args: list[tuple[str, ...]] = []

    def plan_dbt_interop_from_project(
        *,
        project_dir: Path,
        args: tuple[str, ...],
        on_progress: Callable[[str], None],
        progress_stream: object,
        use_color: bool,
    ) -> DbtInteropPlan:
        del on_progress, progress_stream, use_color
        forwarded_project_dirs.append(project_dir)
        forwarded_args.append(args)
        return build_empty_dbt_plan()

    monkeypatch.setattr(
        "sqlbuild._native.plan_dbt_interop_from_project",
        plan_dbt_interop_from_project,
    )

    exit_code: int = run_dbt_command(
        command=DbtInteropCommand.PLAN,
        project_dir=dbt_project_dir,
        args=test_case.dbt_args,
        no_color=True,
    )

    assert exit_code == 0
    captured: CaptureResult[str] = capsys.readouterr()
    assert "Inspecting dbt project and profile..." in captured.err
    assert "SQLBuild dbt setup created" in captured.err
    assert "twin config" in captured.err
    assert "Inspecting dbt project and profile..." not in captured.out
    assert forwarded_project_dirs[0].name == test_case.expected_forwarded_project_dir_name
    assert Path(forwarded_args[0][forwarded_args[0].index("--project-dir") + 1]).is_absolute()
    assert Path(forwarded_args[0][forwarded_args[0].index("--profiles-dir") + 1]).is_absolute()
    twin_config: str = (forwarded_project_dirs[0] / "sqlbuild_project.toml").read_text(
        encoding="utf-8"
    )
    assert f'project_dir = "../{test_case.expected_request_dbt_project_dir_name}' in twin_config
    assert f'target = "{test_case.expected_request_target_name}"' in twin_config
