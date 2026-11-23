from __future__ import annotations

from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project


def prepare_project_with_local_adapter(*, tmp_path: Path) -> Path:
    """Prepare a project that selects a nested project-local DuckDB adapter."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="project_local_adapter",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "project_local_adapter"\n'
                'adapter = "duckdb_plus"\n\n'
                "[connection]\n"
                'database = "local.duckdb"\n'
            ),
            "python/adapters/warehouse/duckdb_plus.py": (
                "from sqlbuild.adapter.contract.models import QueryResult\n"
                "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
                "class DuckDbPlusAdapter(DuckDbAdapter):\n"
                "    adapter_name = 'duckdb_plus'\n\n"
                "    def query(self, connection, sql, *, limit):\n"
                "        return QueryResult(\n"
                "            columns=('adapter_name',),\n"
                "            rows=((self.adapter_name,),),\n"
                "            truncated=False,\n"
                "        )\n"
            ),
        },
    )


def prepare_project_with_rendering_adapter(*, tmp_path: Path) -> Path:
    """Prepare a project whose project-local adapter overrides table rendering."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="rendering_adapter",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "rendering_adapter"\n'
                'adapter = "duckdb_marked"\n\n'
                "[connection]\n"
                'database = "local.duckdb"\n'
            ),
            "python/adapters/duckdb_marked.py": (
                "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
                "class DuckDbMarkedAdapter(DuckDbAdapter):\n"
                "    adapter_name = 'duckdb_marked'\n\n"
                "    def render_create_table_as(self, *, destination, sql):\n"
                "        return (\n"
                "            *super().render_create_table_as(destination=destination, sql=sql),\n"
                "            'CREATE OR REPLACE TABLE main.rendering_override_marker AS '\n"
                "            'SELECT 1 AS marked',\n"
                "        )\n"
            ),
            "sql/models/orders.sql": (
                'MODEL (description "Orders", materialized table);\n\n'
                "SELECT 1 AS order_id, 'shipped' AS status\n"
            ),
        },
    )
