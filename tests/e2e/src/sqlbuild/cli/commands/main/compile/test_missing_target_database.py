"""CLI e2e coverage: missing-location errors name unused connection values and one setting."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ConnectionLocationTestCase,
    MissingTargetDatabaseTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        MissingTargetDatabaseTestCase(
            description="project defaults without a database",
            project_toml=(
                'name = "orders"\nadapter = "snowflake"\n\n[defaults]\nschema = "analytics"\n'
            ),
            expected_error=(
                "error[S101]: snowflake execution requires explicit target database. model "
                "'orders' resolved to database=None schema='analytics'. Set them in the model "
                "config, the environment config, or the project defaults.\n"
                "  = help: to set project-wide defaults, set this in sqlbuild_project.toml:\n"
                "            [defaults]\n"
                '            database = "my_database"\n'
            ),
            expected_example_count=1,
        ),
        MissingTargetDatabaseTestCase(
            description="a named connection's database is named as unused",
            project_toml=(
                'name = "orders"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
                '[connections.warehouse]\naccount = "example"\ndatabase = "ANALYTICS"\n\n'
                '[targets.dev]\nschema = "orders"\n'
            ),
            expected_error=(
                "error[S101]: snowflake execution requires explicit target database. model "
                "'orders' resolved to database=None schema='orders'. Set them in the model "
                "config, the environment config, or the project defaults. The connection's "
                "database 'ANALYTICS' is not used because the project defines named connections "
                "or targets.\n"
                "  = help: to set project-wide defaults, set this in sqlbuild_project.toml:\n"
                "            [defaults]\n"
                '            database = "ANALYTICS"\n'
            ),
            expected_example_count=1,
        ),
        MissingTargetDatabaseTestCase(
            description="the ignored connection schema is shown with command-line vars applied",
            project_toml=(
                'name = "orders"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
                '[vars]\norders_schema = "ORDERS"\n\n'
                '[connections.warehouse]\naccount = "example"\nschema = "${orders_schema}"\n\n'
                '[targets.dev]\ndatabase = "ANALYTICS"\n'
            ),
            expected_error=(
                "The connection's schema 'ORDERS_CI' is not used because the project defines "
                "named connections or targets.\n"
                "  = help: to build this target into that schema, set this in "
                "sqlbuild_project.toml:\n"
                "            [targets.dev]\n"
                '            schema = "ORDERS_CI"\n'
            ),
            expected_example_count=0,
            command=("--no-color", "compile", "--vars", '{"orders_schema": "ORDERS_CI"}'),
        ),
        MissingTargetDatabaseTestCase(
            description="a named connection's schema is named as unused",
            project_toml=(
                'name = "orders"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
                '[connections.warehouse]\naccount = "example"\nschema = "ORDERS"\n\n'
                '[targets.dev]\ndatabase = "ANALYTICS"\n'
            ),
            expected_error=(
                "error[S104]: Named target 'dev' must explicitly set schema to a nonblank literal "
                "or 'preserve'. Resource schemas, defaults, and connection schemas do not select "
                "a named target schema strategy. The connection's schema 'ORDERS' is not used "
                "because the project defines named connections or targets.\n"
                "  = help: to build this target into that schema, set this in "
                "sqlbuild_project.toml:\n"
                "            [targets.dev]\n"
                '            schema = "ORDERS"\n'
            ),
            expected_example_count=0,
        ),
        MissingTargetDatabaseTestCase(
            description="an ignored connection schema stops a bare intermediate loader",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\n\n[connection]\n'
                'database = "dev.duckdb"\nschema = "raw"\n\n'
                '[connections.archive]\ndatabase = "archive.duckdb"\n'
            ),
            extra_files={
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized view, "
                    "schema staging);\n\nSELECT 1 AS order_id\n"
                ),
                "python/loaders/orders.py": (
                    "from sqlbuild.loaders import loader\n\n\n@loader\n"
                    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
                    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
                ),
            },
            expected_error=(
                "error[S103]: Intermediate loader 'raw_orders' has no explicitly resolved "
                "physical write schema. Set schema on the resource or its defaults, the selected "
                "target or its loader_schema, or the project's only top-level [connection]. The "
                "connection's schema 'raw' is not used because the project defines named "
                "connections or targets.\n"
                "  = help: to set project-wide defaults, set this in sqlbuild_project.toml:\n"
                "            [defaults]\n"
                '            schema = "raw"\n'
            ),
            expected_example_count=1,
        ),
        MissingTargetDatabaseTestCase(
            description="sqb load stops a bare loader whose connection schema is not used",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\n\n[connection]\n'
                'database = "dev.duckdb"\nschema = "raw"\n\n'
                '[connections.archive]\ndatabase = "archive.duckdb"\n'
            ),
            extra_files={
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized view, "
                    "schema staging);\n\nSELECT 1 AS order_id\n"
                ),
                "python/loaders/orders.py": (
                    "from sqlbuild.loaders import loader\n\n\n@loader\n"
                    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
                    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
                ),
            },
            expected_error=(
                "error[S103]: Intermediate loader 'raw_orders' has no explicitly resolved "
                "physical write schema. Set schema on the resource or its defaults, the selected "
                "target or its loader_schema, or the project's only top-level [connection]. The "
                "connection's schema 'raw' is not used because the project defines named "
                "connections or targets.\n"
                "  = help: to set project-wide defaults, set this in sqlbuild_project.toml:\n"
                "            [defaults]\n"
                '            schema = "raw"\n'
            ),
            expected_example_count=1,
            command=("--no-color", "load", "--select", "raw_orders"),
        ),
        MissingTargetDatabaseTestCase(
            description="a loader under a preserved target schema stops compile",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n'
                '[defaults]\nschema = "analytics"\n\n[targets.dev]\nschema = "preserve"\n'
            ),
            extra_files={
                "python/loaders/orders.py": (
                    "from sqlbuild.loaders import loader\n\n\n@loader\n"
                    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
                    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
                ),
            },
            expected_error=(
                "error[S103]: Intermediate loader 'raw_orders' would write to a schema named "
                "'preserve'. The target's schema 'preserve' keeps each model's own schema, and a "
                "loader has none, so its destination must name a schema.\n"
                "  = help: to choose where it writes, set its destination in its @loader "
                "decorator:\n"
                '            @loader(destination="my_schema.__loader__raw_orders")\n'
            ),
            expected_example_count=0,
        ),
        MissingTargetDatabaseTestCase(
            description="sqb load stops a loader under a preserved target schema",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n'
                '[defaults]\nschema = "analytics"\n\n[targets.dev]\nschema = "preserve"\n'
            ),
            extra_files={
                "python/loaders/orders.py": (
                    "from sqlbuild.loaders import loader\n\n\n@loader\n"
                    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
                    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
                ),
            },
            expected_error=(
                "error[S103]: Intermediate loader 'raw_orders' would write to a schema named "
                "'preserve'. The target's schema 'preserve' keeps each model's own schema, and a "
                "loader has none, so its destination must name a schema.\n"
                "  = help: to choose where it writes, set its destination in its @loader "
                "decorator:\n"
                '            @loader(destination="my_schema.__loader__raw_orders")\n'
            ),
            expected_example_count=0,
            command=("--no-color", "load", "--select", "raw_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_without_location_when_compiling_then_error_names_the_setting(
    test_case: MissingTargetDatabaseTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized view);\n\n"
                "SELECT 1 AS order_id\n"
            ),
        }
        | test_case.extra_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_error in result.stderr
    assert result.stderr.count("[defaults]") == test_case.expected_example_count


@pytest.mark.parametrize(
    "test_case",
    (
        ConnectionLocationTestCase(
            description="an inline connection database and schema place snowflake models",
            project_toml=(
                'name = "orders"\nadapter = "snowflake"\n\n[connection]\naccount = "example"\n'
                'database = "ANALYTICS"\nschema = "ORDERS"\n'
            ),
            expected_qualified_name="ANALYTICS.ORDERS.orders",
        ),
        ConnectionLocationTestCase(
            description="an inline connection schema places bigquery models beside defaults",
            project_toml=(
                'name = "orders"\nadapter = "bigquery"\n\n[connection]\n'
                'project = "example-project"\nschema = "orders"\n\n'
                '[defaults]\ndatabase = "example-project"\n'
            ),
            expected_qualified_name="`example-project.orders.orders`",
        ),
        ConnectionLocationTestCase(
            description="a duckdb target without a schema builds into the defaults schema",
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\npath = "dev.duckdb"\nschema = "raw"\n\n'
                '[defaults]\nschema = "analytics"\n\n[targets.dev]\n'
            ),
            expected_qualified_name="analytics.orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_connection_location_when_compiling_then_models_build_there(
    test_case: ConnectionLocationTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized view);\n\n"
                "SELECT 1 AS order_id\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("compile", "--no-cache", "--json"), project_dir=project_dir
    )
    payload: dict[str, Any] = json.loads(result.stdout)

    assert result.returncode == 0, f"{test_case.description}: {result.stdout}{result.stderr}"
    assert [model["qualified_name"] for model in payload["resources"]["models"]] == [
        test_case.expected_qualified_name
    ], test_case.description
