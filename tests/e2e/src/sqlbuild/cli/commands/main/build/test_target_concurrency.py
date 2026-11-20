"""E2E coverage for per-target concurrency and its precedence."""

from __future__ import annotations

import subprocess
from pathlib import Path

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    TargetConcurrencyE2ETestCase,
    TargetMicrobatchWarningE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    target_concurrency_microbatch_files,
    target_concurrency_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TargetConcurrencyE2ETestCase(
            description="the selected target's concurrency replaces the project setting",
            target_concurrency="concurrency = 3\n",
            local_settings="",
            command=("--no-color", "build", "--verbose", "--target", "ci"),
            expected_fragment="concurrency  3 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="--concurrency overrides the target",
            target_concurrency="concurrency = 3\n",
            local_settings="",
            command=("--no-color", "build", "--verbose", "--target", "ci", "--concurrency", "1"),
            expected_fragment="concurrency  1 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="the target overrides a local settings override",
            target_concurrency="concurrency = 3\n",
            local_settings="[settings]\nconcurrency = 5\n",
            command=("--no-color", "build", "--verbose", "--target", "ci"),
            expected_fragment="concurrency  3 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="a target without concurrency keeps the local settings override",
            target_concurrency="",
            local_settings="[settings]\nconcurrency = 5\n",
            command=("--no-color", "build", "--verbose", "--target", "ci"),
            expected_fragment="concurrency  5 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="a local target concurrency overrides the project target",
            target_concurrency="concurrency = 3\n",
            local_settings="[targets.ci]\nconcurrency = 4\n",
            command=("--no-color", "build", "--verbose", "--target", "ci"),
            expected_fragment="concurrency  4 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="other targets keep the project setting",
            target_concurrency="concurrency = 3\n",
            local_settings="",
            command=("--no-color", "build", "--verbose"),
            expected_fragment="concurrency  2 configured limit\n",
        ),
        TargetConcurrencyE2ETestCase(
            description="seed uses the target's concurrency",
            target_concurrency="concurrency = 3\n",
            local_settings="",
            command=("--no-color", "seed", "--target", "ci"),
            expected_fragment="concurrency: 3",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_concurrency_when_running_then_documented_precedence_applies(
    tmp_path: Path,
    test_case: TargetConcurrencyE2ETestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_CONCURRENCY", raising=False)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="target_concurrency",
        repo_files=target_concurrency_project_files(
            target_concurrency=test_case.target_concurrency,
            local_settings=test_case.local_settings,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_fragment in result.stdout + result.stderr
    assert table_exists(db_path=project_dir / "warehouse.duckdb", table_name="products")


@pytest.mark.parametrize(
    "test_case",
    [
        TargetConcurrencyE2ETestCase(
            description="zero is rejected with a coded configuration error",
            target_concurrency="concurrency = 0\n",
            local_settings="",
            command=("--no-color", "build", "--target", "ci"),
            expected_fragment="targets.ci.concurrency must be an integer >= 1",
        ),
        TargetConcurrencyE2ETestCase(
            description="a non-integer is rejected with a coded configuration error",
            target_concurrency='concurrency = "8"\n',
            local_settings="",
            command=("--no-color", "build", "--target", "ci"),
            expected_fragment="targets.ci.concurrency must be an integer >= 1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_target_concurrency_when_running_then_config_error_is_coded(
    tmp_path: Path,
    test_case: TargetConcurrencyE2ETestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_CONCURRENCY", raising=False)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="target_concurrency",
        repo_files=target_concurrency_project_files(
            target_concurrency=test_case.target_concurrency,
            local_settings=test_case.local_settings,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode != 0
    assert "D001" in output
    assert test_case.expected_fragment in output
    assert not (project_dir / "warehouse.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        TargetMicrobatchWarningE2ETestCase(
            description="the target's concurrency enables the concurrent microbatch warning",
            command=("--no-color", "build", "--target", "ci"),
            expected_warning_count=1,
        ),
        TargetMicrobatchWarningE2ETestCase(
            description="--concurrency 1 overrides the target and suppresses the warning",
            command=("--no-color", "build", "--target", "ci", "--concurrency", "1"),
            expected_warning_count=0,
        ),
        TargetMicrobatchWarningE2ETestCase(
            description="a target without concurrency keeps the serial project setting",
            command=("--no-color", "build", "--target", "dev"),
            expected_warning_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_concurrency_when_planning_microbatches_then_warning_uses_it(
    test_case: TargetMicrobatchWarningE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_CONCURRENCY", raising=False)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="target_microbatch",
        repo_files=target_concurrency_microbatch_files(),
    )
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "warehouse.duckdb"))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00'), (2, '2026-01-01 01:30:00')"
    )
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    connection = duckdb.connect(str(project_dir / "warehouse.duckdb"))
    connection.execute(
        "INSERT INTO raw_events VALUES (3, '2026-01-01 02:30:00'), (4, '2026-01-01 03:30:00')"
    )
    connection.close()

    incremental: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = incremental.stdout + incremental.stderr
    assert initial.returncode == 0, initial.stdout + initial.stderr
    assert incremental.returncode == 0, output
    assert (
        output.count("Concurrent microbatch execution is enabled")
        == test_case.expected_warning_count
    )
