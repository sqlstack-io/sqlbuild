import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.entry._test_types import (
    BrokenPipeTestCase,
    InterruptTestCase,
    PythonFreeTestCase,
    PythonStartTestCase,
)
from tests.integration.src.sqlbuild.cli.entry.helpers import (
    PYTHON_CLI,
    LauncherRun,
    compute_log_files,
    counting_environment,
    prepare_custom_rules,
    prepare_custom_skill,
    prepare_named_pipe_project,
    prepare_nothing,
    prepare_project,
    prepare_published_layer_options,
    prepare_removed_allow_exceptions,
    prepare_stale_skills,
    python_starts,
    reset_directory,
    restore_default_interrupt,
    run,
    run_without_reader,
)

INTERRUPT_DELAY_SECONDS: float = 1.0
TRACEBACK_LAST_LINE: bytes = b"BrokenPipeError: [Errno 32] Broken pipe\n"
INTERRUPT_TIMEOUT_SECONDS: float = 30.0


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        PythonFreeTestCase(
            description="top-level help",
            args=("--help",),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="short help after options",
            args=("--no-color", "-h"),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="command help",
            args=("compile", "--help"),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="nested command help",
            args=("rules", "show", "--help"),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="version among options",
            args=("--no-color", "--version"),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="unknown command",
            args=("not-a-command",),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="invalid choice",
            args=("skills", "--target", "bogus"),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="invalid integer",
            args=("build", "--concurrency", "many"),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="post-parse rule",
            args=("build", "--concurrency", "0"),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="missing option value",
            args=("--project-dir",),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="unrecognized argument",
            args=("rules", "list", "--json"),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="mutually exclusive options",
            args=("lineage", "orders", "--json", "--format", "tree"),
            prepare=prepare_nothing,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="bare sqb",
            args=(),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="clean",
            args=("clean",),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="clean outside a project",
            args=("clean",),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules list",
            args=("rules", "list"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules without an action",
            args=("rules",),
            prepare=prepare_project,
            expected_exit_code=2,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="rules list json",
            args=("rules", "--json", "list"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show",
            args=("rules", "show", "SQBRMODEL101"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show with thresholds",
            args=("rules", "show", "SQBRTEST101"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show with a string-list option default",
            args=("rules", "show", "SQBRGRAPH103"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show with a table-list option",
            args=("rules", "show", "SQBRPROJECT107"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show cross join",
            args=("rules", "show", "SQLCLEAN038"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show inline directive rule",
            args=("rules", "show", "SQBRSUPPRESSION000"),
            prepare=prepare_project,
            expected_exit_code=2,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules show unknown",
            args=("rules", "show", "SQBRNOPE101"),
            prepare=prepare_project,
            expected_exit_code=2,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="rules with stale skills",
            args=("rules", "list"),
            prepare=prepare_stale_skills,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="skills",
            args=("skills",),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="skills for one target",
            args=("skills", "--target", "claude"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_captures=1,
        ),
        PythonFreeTestCase(
            description="init",
            args=("init",),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="init a new directory",
            args=("init", "--project-dir", "shop/"),
            prepare=prepare_nothing,
            expected_exit_code=0,
            expected_captures=0,
        ),
        PythonFreeTestCase(
            description="init beside a custom skill",
            args=("init",),
            prepare=prepare_custom_skill,
            expected_exit_code=1,
            expected_captures=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_free_invocation_when_running_sqb_then_python_never_starts_and_output_matches(
    test_case: PythonFreeTestCase, tmp_path: Path
) -> None:
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)
    workspace: Path = tmp_path / "workspace"
    test_case.prepare(reset_directory(workspace))
    python: LauncherRun = run(PYTHON_CLI, args=test_case.args, cwd=workspace)
    python_logs: list[dict[str, object]] = compute_log_files(workspace)
    test_case.prepare(reset_directory(workspace))

    native: LauncherRun = run(program, args=test_case.args, cwd=workspace)

    assert native == python
    assert native[0] == test_case.expected_exit_code
    assert python_starts(marker) == 0
    assert compute_log_files(workspace) == python_logs
    assert len(python_logs) == test_case.expected_captures


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        PythonStartTestCase(
            description="custom rules",
            args=("rules", "list"),
            prepare=prepare_custom_rules,
            expected_python_starts=1,
        ),
        PythonStartTestCase(
            description="removed allow_exceptions setting",
            args=("rules", "list"),
            prepare=prepare_removed_allow_exceptions,
            expected_python_starts=1,
        ),
        PythonStartTestCase(
            description="configured rule options",
            args=("rules", "show", "SQBRPROJECT107"),
            prepare=prepare_published_layer_options,
            expected_python_starts=1,
        ),
        PythonStartTestCase(
            description="dbt passthrough help",
            args=("dbt", "plan", "--help"),
            prepare=prepare_nothing,
            expected_python_starts=1,
        ),
        PythonStartTestCase(
            description="debug diagnostics",
            args=("clean", "--debug"),
            prepare=prepare_project,
            expected_python_starts=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invocation_needing_python_when_running_sqb_then_python_runs_it(
    test_case: PythonStartTestCase, tmp_path: Path
) -> None:
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)
    workspace: Path = reset_directory(tmp_path / "workspace")
    test_case.prepare(workspace)
    python: LauncherRun = run(PYTHON_CLI, args=test_case.args, cwd=workspace)
    test_case.prepare(reset_directory(workspace))

    native: LauncherRun = run(program, args=test_case.args, cwd=workspace)

    assert native[:2] == python[:2]
    assert python_starts(marker) == test_case.expected_python_starts


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX signals and named pipes")
@pytest.mark.parametrize(
    "test_case",
    [
        InterruptTestCase(
            description="rules list waiting on the project file",
            args=("rules", "list"),
            prepare=prepare_named_pipe_project,
            expected_exit_code=-signal.SIGINT,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_free_command_when_interrupted_then_sqb_dies_by_sigint_without_python(
    test_case: InterruptTestCase, tmp_path: Path
) -> None:
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)
    project_dir: Path = reset_directory(tmp_path / "project")
    test_case.prepare(project_dir)
    process: subprocess.Popen[bytes] = subprocess.Popen(
        [str(program), *test_case.args],
        cwd=project_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        preexec_fn=restore_default_interrupt,
    )
    time.sleep(INTERRUPT_DELAY_SECONDS)
    assert process.poll() is None, "sqb returned before reading the project file"

    process.send_signal(signal.SIGINT)
    _, stderr = process.communicate(timeout=INTERRUPT_TIMEOUT_SECONDS)

    assert process.returncode == test_case.expected_exit_code, stderr
    assert python_starts(marker) == 0


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pipes")
@pytest.mark.parametrize(
    "test_case",
    [
        BrokenPipeTestCase(
            description="help kept until the final flush",
            args=("--help",),
            prepare=prepare_nothing,
            expected_exit_code=120,
            expected_stderr_end=TRACEBACK_LAST_LINE,
        ),
        BrokenPipeTestCase(
            description="a short observed report kept until the final flush",
            args=("rules", "show", "SQBRMODEL101"),
            prepare=prepare_project,
            expected_exit_code=120,
            expected_stderr_end=TRACEBACK_LAST_LINE,
        ),
        BrokenPipeTestCase(
            description="a report the failed flush drops",
            args=("rules", "list"),
            prepare=prepare_project,
            expected_exit_code=0,
            expected_stderr_end=b"",
        ),
        BrokenPipeTestCase(
            description="a report whose write fails",
            args=("rules", "--json", "list"),
            prepare=prepare_project,
            expected_exit_code=1,
            expected_stderr_end=TRACEBACK_LAST_LINE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stdout_without_reader_when_running_sqb_then_status_and_logs_match_python(
    test_case: BrokenPipeTestCase, tmp_path: Path
) -> None:
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)
    workspace: Path = reset_directory(tmp_path / "workspace")
    test_case.prepare(workspace)
    python: LauncherRun = run_without_reader(PYTHON_CLI, args=test_case.args, cwd=workspace)
    python_logs: list[dict[str, object]] = compute_log_files(workspace)
    test_case.prepare(reset_directory(workspace))

    native: LauncherRun = run_without_reader(program, args=test_case.args, cwd=workspace)

    assert (native[0], python[0]) == (test_case.expected_exit_code,) * 2
    assert native[2].endswith(test_case.expected_stderr_end), native[2]
    assert python[2].endswith(test_case.expected_stderr_end), python[2]
    assert python_starts(marker) == 0
    assert compute_log_files(workspace) == python_logs


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
