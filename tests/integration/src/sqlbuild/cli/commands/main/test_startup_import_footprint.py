"""Fresh-process guard that CLI startup loads only what the invoked command needs."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    ReusedCompileImportFootprintTestCase,
    StartupImportFootprintTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    write_compile_startup_project,
)

WAREHOUSE_AND_INTEGRATION_MODULES: tuple[str, ...] = (
    "dagster",
    "databricks",
    "dlt",
    "duckdb",
    "google.cloud.bigquery",
    "ingestr",
    "jinja2",
    "psycopg",
    "pyarrow",
    "pymssql",
    "snowflake.connector",
)
COMPILE_ONLY_MODULES: tuple[str, ...] = (
    "filelock",
    "pydantic",
    "yaml",
    "sqlbuild.adapter.contract.classes.base_adapter",
    "sqlbuild.lint",
    "sqlbuild.rule_engine",
    "sqlbuild.runtime.compute_logs",
)
PRESENTATION_AND_PROVIDER_MODULES: tuple[str, ...] = (
    "pydantic_settings",
    "rich",
)
PARSE_ONLY_FORBIDDEN: tuple[str, ...] = (
    *WAREHOUSE_AND_INTEGRATION_MODULES,
    *COMPILE_ONLY_MODULES,
    *PRESENTATION_AND_PROVIDER_MODULES,
)
COMPILE_FORBIDDEN: tuple[str, ...] = (
    *WAREHOUSE_AND_INTEGRATION_MODULES,
    *PRESENTATION_AND_PROVIDER_MODULES,
)
FULL_COMPILE_MODULES: tuple[str, ...] = (
    "pydantic",
    "sqlbuild.adapter.contract.classes.base_adapter",
    "sqlbuild.compiler.pipeline",
)
INSPECTION_FORBIDDEN: tuple[str, ...] = (
    *WAREHOUSE_AND_INTEGRATION_MODULES,
    "filelock",
    "pydantic",
    "yaml",
    "sqlbuild.lint",
    "sqlbuild.rule_engine",
    *PRESENTATION_AND_PROVIDER_MODULES,
)
PROJECT_DISCOVERY_MODULES: tuple[str, ...] = (
    "sqlbuild.compiler.discovery._helpers.validation",
    "sqlbuild.compiler.macro_bridge",
    "sqlbuild.compiler.python_nodes",
)
QUERY_FORBIDDEN: tuple[str, ...] = (
    *(module for module in INSPECTION_FORBIDDEN if module != "duckdb"),
    *PROJECT_DISCOVERY_MODULES,
)
MEMORY_CONNECTION: str = '[connection]\ndatabase = ":memory:"\n'
REUSED_COMPILE_FORBIDDEN: tuple[str, ...] = (
    *WAREHOUSE_AND_INTEGRATION_MODULES,
    *FULL_COMPILE_MODULES,
    *PRESENTATION_AND_PROVIDER_MODULES,
)

_FOOTPRINT_SCRIPT: str = """
import contextlib, io, json, sys
from sqlbuild.cli.entry.main.entry import main

argv = json.loads(sys.argv[1])
forbidden = json.loads(sys.argv[2])
stderr = io.StringIO()
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
    code = main(argv)
loaded = sorted(
    name for name in forbidden
    if any(module == name or module.startswith(name + ".") for module in sys.modules)
)
print(json.dumps({"code": code, "loaded": loaded, "stderr": stderr.getvalue()}))
"""


@pytest.mark.parametrize(
    "test_case",
    [
        StartupImportFootprintTestCase(
            description="version",
            argv=("--version",),
            expected_exit_code=0,
            forbidden_modules=PARSE_ONLY_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="root help",
            argv=("--help",),
            expected_exit_code=0,
            forbidden_modules=PARSE_ONLY_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="compile help",
            argv=("compile", "--help"),
            expected_exit_code=0,
            forbidden_modules=PARSE_ONLY_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="unknown argument",
            argv=("compile", "--unknown-option"),
            expected_exit_code=2,
            forbidden_modules=PARSE_ONLY_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="plain compile",
            argv=("compile", "--json"),
            expected_exit_code=0,
            forbidden_modules=COMPILE_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="scope",
            argv=("scope", "orders"),
            expected_exit_code=0,
            forbidden_modules=INSPECTION_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="lineage",
            argv=("lineage", "orders", "--json"),
            expected_exit_code=0,
            forbidden_modules=INSPECTION_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="dag",
            argv=("dag", "--json"),
            expected_exit_code=0,
            forbidden_modules=INSPECTION_FORBIDDEN,
        ),
        StartupImportFootprintTestCase(
            description="ad hoc query",
            argv=("query", "SELECT 1 AS order_id"),
            expected_exit_code=0,
            forbidden_modules=QUERY_FORBIDDEN,
            local_config=MEMORY_CONNECTION,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fresh_interpreter_when_running_cli_then_heavy_modules_stay_unloaded(
    test_case: StartupImportFootprintTestCase, tmp_path: Path
) -> None:
    write_compile_startup_project(tmp_path)
    _ = (tmp_path / "sqlbuild_local.toml").write_text(test_case.local_config, encoding="utf-8")
    argv: list[str] = ["--project-dir", str(tmp_path), *test_case.argv]

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            sys.executable,
            "-c",
            _FOOTPRINT_SCRIPT,
            json.dumps(argv),
            json.dumps(test_case.forbidden_modules),
        ],
        check=True,
        capture_output=True,
        text=True,
        cwd=tmp_path,
    )
    outcome: dict[str, object] = json.loads(result.stdout.strip().splitlines()[-1])

    assert outcome["code"] == test_case.expected_exit_code
    assert cast(list[str], outcome["loaded"]) == []


@pytest.mark.parametrize(
    "test_case",
    [
        ReusedCompileImportFootprintTestCase(
            description="reused compile",
            argv=("compile", "--json"),
            expected_exit_code=0,
            expected_reuse_message="Inputs unchanged; reused the previous compile",
            forbidden_modules=REUSED_COMPILE_FORBIDDEN,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unchanged_project_when_compile_is_reused_then_compiler_stays_unloaded(
    test_case: ReusedCompileImportFootprintTestCase, tmp_path: Path
) -> None:
    write_compile_startup_project(tmp_path)
    command: list[str] = [
        sys.executable,
        "-c",
        _FOOTPRINT_SCRIPT,
        json.dumps(["--project-dir", str(tmp_path), *test_case.argv]),
        json.dumps(test_case.forbidden_modules),
    ]
    environment: dict[str, str] = {**os.environ, REUSE_DISABLE_ENV_VAR: "0"}

    _ = subprocess.run(command, check=True, capture_output=True, cwd=tmp_path, env=environment)
    result: subprocess.CompletedProcess[str] = subprocess.run(
        command, check=True, capture_output=True, text=True, cwd=tmp_path, env=environment
    )
    outcome: dict[str, object] = json.loads(result.stdout.strip().splitlines()[-1])

    assert outcome["code"] == test_case.expected_exit_code
    assert test_case.expected_reuse_message in cast(str, outcome["stderr"])
    assert cast(list[str], outcome["loaded"]) == []


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
