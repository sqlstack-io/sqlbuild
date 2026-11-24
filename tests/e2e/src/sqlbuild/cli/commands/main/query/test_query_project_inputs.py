"""`sqb query` reads only the project configuration it needs to connect."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.query._test_types import (
    QueryConnectionConfigErrorTestCase,
    QueryProjectDiscoveryTestCase,
    QueryTargetConnectionTestCase,
    QueryUnrelatedProjectFaultTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)

PROJECT_TOML: str = (
    'name = "orders_project"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
VALID_MODEL_SQL: str = "MODEL (materialized table);\n\nSELECT 1 AS order_id\n"
TARGETS_PROJECT_TOML: str = dedent(
    """
    name = "orders_targets"
    adapter = "duckdb"
    default_target = "dev"

    [connections.dev]
    database = "dev.duckdb"

    [connections.prod]
    database = "prod.duckdb"

    [targets.dev]
    connection = "dev"

    [targets.prod]
    connection = "prod"
    """
).lstrip()
STAGE_WORK_SCRIPT: str = """
import contextlib, io, json, sys
import sqlbuild._native as native
from sqlbuild.cli.entry.main.entry import main

_ = native.compile_stage_work_info(reset=True)
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
    code = main(json.loads(sys.argv[1]))
work = native.compile_stage_work_info()
print(json.dumps({"code": code, "discoveries": work.get("discover_project_inputs", 0)}))
"""


@pytest.mark.parametrize(
    "test_case",
    (
        QueryUnrelatedProjectFaultTestCase(
            description="model without a MODEL header",
            repo_files={"sql/models/orders.sql": "SELECT FROM WHERE (\n"},
        ),
        QueryUnrelatedProjectFaultTestCase(
            description="project Python that fails to import",
            repo_files={"python/nodes/broken.py": 'raise RuntimeError("broken project python")\n'},
        ),
        QueryUnrelatedProjectFaultTestCase(
            description="local SQL analysis opt-out the project forbids",
            repo_files={
                "sqlbuild_project.toml": PROJECT_TOML
                + "\n[settings]\nrequire_sql_analysis = true\n",
                "sqlbuild_local.toml": "[settings]\nsql_analysis = false\n",
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unrelated_broken_project_input_when_running_ad_hoc_query_then_sql_runs(
    tmp_path: Path,
    test_case: QueryUnrelatedProjectFaultTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={"sqlbuild_project.toml": PROJECT_TOML, **test_case.repo_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("query", "SELECT 7 AS order_id", "--format", "json"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == list(test_case.expected_rows)
    assert "Declaration parsing" not in result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        QueryProjectDiscoveryTestCase(
            description="ad hoc query discovers no project files",
            command=("query", "SELECT 1 AS order_id"),
            expected_exit_code=0,
            expected_project_discoveries=0,
        ),
        QueryProjectDiscoveryTestCase(
            description="dag still discovers the project",
            command=("dag", "--json"),
            expected_exit_code=0,
            expected_project_discoveries=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_when_running_command_then_project_discovery_runs_only_when_needed(
    tmp_path: Path,
    test_case: QueryProjectDiscoveryTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={
            "sqlbuild_project.toml": PROJECT_TOML,
            "sql/models/orders.sql": VALID_MODEL_SQL,
        },
    )
    argv: list[str] = ["--project-dir", str(project_dir), *test_case.command]

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [sys.executable, "-c", STAGE_WORK_SCRIPT, json.dumps(argv)],
        check=True,
        capture_output=True,
        text=True,
        cwd=tmp_path,
    )
    outcome: dict[str, int] = json.loads(result.stdout.strip().splitlines()[-1])

    assert outcome == {
        "code": test_case.expected_exit_code,
        "discoveries": test_case.expected_project_discoveries,
    }


@pytest.mark.parametrize(
    "test_case",
    (
        QueryTargetConnectionTestCase(
            description="default target connection",
            command=("query", "SELECT label FROM destination", "--format", "json"),
            expected_stdout_fragment='[{"label": "dev"}]\n',
        ),
        QueryTargetConnectionTestCase(
            description="explicit target connection",
            command=(
                "query",
                "SELECT label FROM destination",
                "--format",
                "json",
                "--target",
                "prod",
            ),
            expected_stdout_fragment='[{"label": "prod"}]\n',
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_target_connections_when_running_query_then_selected_target_database_is_read(
    tmp_path: Path,
    test_case: QueryTargetConnectionTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_targets",
        repo_files={"sqlbuild_project.toml": TARGETS_PROJECT_TOML},
    )
    for label in ("dev", "prod"):
        execute_duckdb(
            db_path=project_dir / f"{label}.duckdb",
            sql=f"CREATE TABLE destination AS SELECT '{label}' AS label",
        )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout == test_case.expected_stdout_fragment


@pytest.mark.parametrize(
    "test_case",
    (
        QueryConnectionConfigErrorTestCase(
            description="target warehouse under an adapter without warehouses",
            project_toml=TARGETS_PROJECT_TOML + '\n[targets.dev.warehouses]\nquery = "ORDERS_WH"\n',
            expected_stderr_fragment=(
                "[targets.dev.warehouses] selects a query warehouse, but the 'duckdb' adapter "
                "has no warehouse to select"
            ),
        ),
        QueryConnectionConfigErrorTestCase(
            description="target selecting an unknown connection",
            project_toml=TARGETS_PROJECT_TOML.replace(
                'connection = "dev"\n', 'connection = "missing"\n'
            ),
            expected_stderr_fragment="error[P001]: Unknown connection 'missing' selected by target.",
        ),
        QueryConnectionConfigErrorTestCase(
            description="no connection configured",
            project_toml='name = "orders_project"\nadapter = "duckdb"\n',
            expected_stderr_fragment=(
                "no target is selected and no top-level [connection] is configured"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_connection_configuration_when_running_query_then_reports_config_error(
    tmp_path: Path,
    test_case: QueryConnectionConfigErrorTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_config_error",
        repo_files={"sqlbuild_project.toml": test_case.project_toml},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("query", "SELECT 1 AS order_id"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode
    assert test_case.expected_stderr_fragment in result.stderr
    assert "RECORD" not in result.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
