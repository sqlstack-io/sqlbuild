from __future__ import annotations

import json
import sys
from pathlib import Path

from sqlbuild._native import (
    DbtInteropCommand,
    DbtInteropPlan,
    DbtInteropSelectionResult,
    build_dbt_interop_plan,
)
from tests.unit.src.sqlbuild.cli.commands.main.dbt._test_types import DbtAutoInitTestCase

MINIMAL_SQLBUILD_PROJECT: str = 'name = "demo"\nadapter = "duckdb"\n'


def build_empty_dbt_plan() -> DbtInteropPlan:
    """Build a minimal dbt interop plan for CLI output tests."""

    return build_dbt_interop_plan(
        command=DbtInteropCommand.PLAN,
        dbt_command_argv=("dbt", "ls"),
        dbt_ls_nodes=(),
        sqlbuild_command_argvs=(),
        selection=DbtInteropSelectionResult(),
    )


def prepare_dbt_auto_init_dirs(*, test_case: DbtAutoInitTestCase, tmp_path: Path) -> Path:
    """Prepare current and sibling SQLBuild config files for auto-init tests."""

    dbt_project_dir: Path = tmp_path / "dbt_project"
    dbt_project_dir.mkdir()
    sibling_sqlbuild_dir: Path = tmp_path / "sqlbuild_project"
    for current_project_dir in (dbt_project_dir,) * int(test_case.has_current_sqlbuild_project):
        write_minimal_sqlbuild_project(current_project_dir)
    for sibling_project_dir in (sibling_sqlbuild_dir,) * int(
        test_case.has_sibling_sqlbuild_project
    ):
        sibling_project_dir.mkdir()
        write_minimal_sqlbuild_project(sibling_project_dir)
    return dbt_project_dir


def write_minimal_sqlbuild_project(project_dir: Path) -> None:
    """Write a minimal SQLBuild project config for wrapper tests."""

    (project_dir / "sqlbuild_project.toml").write_text(MINIMAL_SQLBUILD_PROJECT, encoding="utf-8")


def write_fake_dbt_executable(directory: Path, *, exit_code: int) -> Path:
    """Write an executable stand-in for dbt that records its arguments and exits."""

    script: Path = directory / "fake_dbt"
    script.write_text(
        f"#!{sys.executable}\n"
        "import json, pathlib, sys\n"
        f"pathlib.Path({str(directory / 'dbt_args.json')!r}).write_text(json.dumps(sys.argv[1:]))\n"
        f"sys.exit({exit_code!r})\n",
        encoding="utf-8",
    )
    script.chmod(0o755)
    return script


def read_fake_dbt_args(directory: Path) -> tuple[str, ...]:
    """The arguments the stand-in dbt in `directory` last received."""

    return tuple(json.loads((directory / "dbt_args.json").read_text(encoding="utf-8")))


def write_duckdb_sqlbuild_project(project_dir: Path) -> None:
    """Write a SQLBuild project config with an inline DuckDB connection."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n'
        'extra_paths = ["dbt_project", "fake_dbt", "dbt_args.json"]\n\n'
        '[connection]\npath = "warehouse.duckdb"\n',
        encoding="utf-8",
    )
