from __future__ import annotations

import subprocess
from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


def prepare_freshness_project(
    *,
    tmp_path: Path,
    raw_orders_freshness: str | None = None,
    include_error_source: bool = True,
    include_managed_source: bool = False,
) -> Path:
    repo_files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "freshness_command"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "warehouse.duckdb"\n'
        ),
        "sql/sources/raw.yml": freshness_sources_yml(
            raw_orders_freshness=raw_orders_freshness,
            include_error_source=include_error_source,
            include_managed_source=include_managed_source,
        ),
        "sql/models/orders.sql": (
            'MODEL (description "Test model orders.", '
            'materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
        ),
    }
    repo_files.update(
        {
            False: {},
            True: {
                "python/nodes/loaders/raw/loader.py": (
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader\n"
                    "def raw_managed(ctx):\n"
                    "    '''Test loader raw_managed.'''\n    return [{'event_id': 5}]\n"
                )
            },
        }[include_managed_source]
    )
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="freshness_command",
        repo_files=repo_files,
    )


def prepare_multi_schema_freshness_project(*, tmp_path: Path) -> Path:
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="multi_schema_freshness_command",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "multi_schema_freshness_command"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/sources/raw.yml": freshness_sources_yml(include_error_source=False),
            "sql/models/dev_orders.sql": (
                'MODEL (description "Test model dev_orders.", '
                'materialized table, schema dev);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "sql/models/mart_orders.sql": (
                'MODEL (description "Test model mart_orders.", '
                'materialized table, schema mart);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
        },
    )


def freshness_sources_yml(
    *,
    raw_orders_query: str = "SELECT 1 AS data_version",
    raw_orders_freshness: str | None = None,
    include_error_source: bool = True,
    include_managed_source: bool = False,
) -> str:
    freshness: str = raw_orders_freshness or (
        "    freshness:\n"
        "      strategy: sql\n"
        "      type: integer\n"
        f"      query: {raw_orders_query}\n"
    )
    error_source: str = {
        False: "",
        True: (
            "  - name: raw_error\n"
            "    description: Test source raw_error.\n"
            "    expression: SELECT 3 AS order_id\n"
            "    freshness:\n"
            "      strategy: sql\n"
            "      type: integer\n"
            "      query: SELECT missing_column AS data_version\n"
        ),
    }[include_error_source]
    managed_source: str = {
        False: "",
        True: (
            "  - name: raw_managed\n"
            "    description: Test source raw_managed.\n"
            "    managed: true\n"
            "    expression: SELECT 5 AS event_id\n"
            "    freshness:\n"
            "      strategy: sql\n"
            "      type: integer\n"
            "      query: SELECT 5 AS data_version\n"
        ),
    }[include_managed_source]
    return (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    expression: SELECT 1 AS order_id\n"
        f"{freshness}"
        f"{error_source}"
        f"{managed_source}"
        "  - name: raw_unknown\n"
        "    description: Test source raw_unknown.\n"
        "    expression: SELECT 2 AS order_id\n"
    )


def persist_direct_source_freshness(*, project_dir: Path) -> None:
    initial_build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert initial_build_result.returncode == 0, (
        initial_build_result.stdout + initial_build_result.stderr
    )
    changes_only_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert changes_only_result.returncode == 0, (
        changes_only_result.stdout + changes_only_result.stderr
    )
