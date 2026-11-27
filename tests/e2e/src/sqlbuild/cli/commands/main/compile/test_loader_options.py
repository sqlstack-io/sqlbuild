"""E2E coverage for `@loader` options that SQLLoad checks while `sqb compile` reads the project."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DeclaredTypeCompileTestCase,
    LoaderOptionsCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import loaded_source_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_DUCKDB_PROJECT: str = 'name = "declared_types"\nadapter = "duckdb"\n'
_SNOWFLAKE_PROJECT: str = (
    'name = "declared_types"\nadapter = "snowflake"\n\n'
    '[connection]\naccount = "example"\nuser = "example"\ndatabase = "RAW"\nschema = "SHOP"\n'
)


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderOptionsCompileTestCase(
            description="a merge loader without unique_key fails",
            loader_options="write_strategy='merge'",
            expected_exit_code=1,
            expected_stderr_fragment=(
                "error[D011]: python/nodes/loaders/raw_orders/loader.py @loader 'raw_orders': "
                'write_strategy = "merge" needs a unique_key'
            ),
        ),
        LoaderOptionsCompileTestCase(
            description="a merge loader with unique_key compiles",
            loader_options="write_strategy='merge', unique_key=['id']",
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_loader_strategy_options_when_compiling_then_sqlload_checks_how_they_combine(
    test_case: LoaderOptionsCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_options",
        repo_files={
            "sqlbuild_project.toml": 'name = "loader_options"\nadapter = "duckdb"\n',
            "sql/sources/raw.yml": "sources:\n  - name: raw_orders\n    managed: true\n",
            "python/nodes/loaders/raw_orders/loader.py": (
                "from sqlbuild.loaders import loader\n\n\n"
                f"@loader({test_case.loader_options})\n"
                "def raw_orders(ctx):\n"
                "    '''Load the orders.'''\n"
                "    return [{'id': 1}]\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr, result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        DeclaredTypeCompileTestCase(
            description="DuckDB INTEGER is not the BIGINT SQLLoad creates",
            repo_files=loaded_source_files(
                project_toml=_DUCKDB_PROJECT, column_type="INTEGER", loader_columns="[]"
            ),
            expected_exit_code=1,
            expected_stderr_fragment=(
                "@loader 'raw_orders': source 'raw_orders' declares column id as INTEGER, which "
                "SQLLoad creates as BIGINT in DuckDB; declare `type: BIGINT` in its sources file"
            ),
        ),
        DeclaredTypeCompileTestCase(
            description="DuckDB TIMESTAMP is the timestamp SQLLoad creates",
            repo_files=loaded_source_files(
                project_toml=_DUCKDB_PROJECT, column_type="TIMESTAMP", loader_columns="[]"
            ),
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        DeclaredTypeCompileTestCase(
            description="Snowflake INTEGER is the NUMBER(38,0) SQLLoad creates",
            repo_files=loaded_source_files(
                project_toml=_SNOWFLAKE_PROJECT, column_type="INTEGER", loader_columns="[]"
            ),
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        DeclaredTypeCompileTestCase(
            description="an intermediate loader's columns are checked too",
            repo_files=loaded_source_files(
                project_toml=_DUCKDB_PROJECT,
                column_type="BIGINT",
                loader_columns="[{'name': 'id', 'type': 'INTEGER'}]",
            ),
            expected_exit_code=1,
            expected_stderr_fragment=(
                "@loader 'fetch_orders': it declares column id as INTEGER, which SQLLoad creates "
                "as BIGINT in DuckDB; declare {'name': 'id', 'type': 'BIGINT'} in columns="
            ),
        ),
        DeclaredTypeCompileTestCase(
            description="a self-managed loader writes its own table, so any type compiles",
            repo_files={
                "sqlbuild_project.toml": _DUCKDB_PROJECT,
                "sql/sources/raw.yml": (
                    "sources:\n  - name: raw_status\n    description: Statuses.\n"
                    "    managed: true\n"
                    "    columns:\n      - name: status_id\n        type: INTEGER\n"
                ),
                "python/nodes/loaders/raw_status/loader.py": (
                    "from sqlbuild.loaders import loader\n\n\n"
                    "@loader\n"
                    "def raw_status(ctx):\n"
                    "    '''Load the statuses.'''\n"
                    "    ctx.execute_sql(f'CREATE TABLE {ctx.destination} AS SELECT 1 AS status_id')\n"
                ),
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        DeclaredTypeCompileTestCase(
            description="a source a load fills declares what SQLLoad creates",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "declared_types"\nadapter = "duckdb"\n\n'
                    '[connections.landing]\nkind = "filesystem"\nroot = "landing"\n'
                ),
                "sql/sources/shop.yml": (
                    "load:\n  name: shop\n  kind: files\n  connection: landing\n"
                    "  path: orders/\n  format: csv\n  table_name: shop__orders\n"
                    "sources:\n  - name: shop__orders\n    description: Orders.\n"
                    "    columns:\n      - name: id\n        type: INTEGER\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragment=(
                "source 'shop__orders' declares column id as INTEGER, which SQLLoad creates as "
                "BIGINT in DuckDB; declare `type: BIGINT`"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_loaded_sources_declared_type_when_compiling_then_it_must_be_what_sqlload_creates(
    test_case: DeclaredTypeCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="declared_types", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr, result.stderr
