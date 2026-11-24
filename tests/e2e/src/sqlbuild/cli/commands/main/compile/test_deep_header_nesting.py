"""E2E coverage for deeply nested header values failing discovery instead of crashing it."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DeepCallNestingCompileCase,
    DeepHeaderNestingCompileCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_CONFIG: str = 'name = "orders"\nadapter = "duckdb"\n'
_ORDERS_MODEL: str = "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id\n"
_COMPILE_TIME_BOUND_SECONDS: float = 120.0
_NESTING_HELP: str = "help: flatten the value so it nests at most 256 levels deep"
_CONSTANT_CALL_NESTING: tuple[str, ...] = (
    "error[D013]",
    "sql/constants/limits.sql:3' contains invalid SQLBuild header syntax: "
    "values nest deeper than 256 levels",
    _NESTING_HELP,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DeepHeaderNestingCompileCase(
            description="native_model_1k",
            path="sql/models/orders.sql",
            prefix="MODEL (\n  description 'Orders.',\n  tags ",
            suffix=",\n);\n\nSELECT 1 AS order_id\n",
            depth=1_000,
            expected_fragments=(
                "error[D002]",
                "sql/models/orders.sql:3' contains invalid SQLBuild header syntax: "
                "values nest deeper than 256 levels",
                _NESTING_HELP,
            ),
        ),
        DeepHeaderNestingCompileCase(
            description="native_constant_20k",
            path="sql/constants/limits.sql",
            prefix="\nCONSTANT (\n  name order_limits,\n  value ",
            suffix=",\n);\n",
            depth=20_000,
            expected_fragments=(
                "error[D013]",
                "sql/constants/limits.sql:4' contains invalid SQLBuild header syntax: "
                "values nest deeper than 256 levels",
                _NESTING_HELP,
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deeply_nested_header_value_when_compiling_then_fails_with_coded_discovery_error(
    test_case: DeepHeaderNestingCompileCase, tmp_path: Path
) -> None:
    nested_value: str = "[" * test_case.depth + "1" + "]" * test_case.depth
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_CONFIG,
            "sql/models/orders.sql": _ORDERS_MODEL,
            test_case.path: test_case.prefix + nested_value + test_case.suffix,
        },
    )

    started: float = time.monotonic()
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )
    elapsed: float = time.monotonic() - started

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert elapsed < _COMPILE_TIME_BOUND_SECONDS


@pytest.mark.parametrize(
    "test_case",
    [
        DeepCallNestingCompileCase(
            description="calls_240",
            depth=240,
            expected_fragments=_CONSTANT_CALL_NESTING,
        ),
        DeepCallNestingCompileCase(
            description="calls_256",
            depth=256,
            expected_fragments=_CONSTANT_CALL_NESTING,
        ),
        DeepCallNestingCompileCase(
            description="calls_257",
            depth=257,
            expected_fragments=_CONSTANT_CALL_NESTING,
        ),
        DeepCallNestingCompileCase(
            description="calls_20k",
            depth=20_000,
            expected_fragments=_CONSTANT_CALL_NESTING,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deeply_nested_constant_calls_when_compiling_then_fails_coded(
    test_case: DeepCallNestingCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_CONFIG,
            "sql/models/orders.sql": _ORDERS_MODEL,
            "sql/constants/limits.sql": (
                "CONSTANT (\n  name order_limits,\n  value "
                + "limit(level " * test_case.depth
                + "1"
                + ")" * test_case.depth
                + ",\n);\n"
            ),
        },
    )

    started: float = time.monotonic()
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    elapsed: float = time.monotonic() - started

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert elapsed < _COMPILE_TIME_BOUND_SECONDS


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
