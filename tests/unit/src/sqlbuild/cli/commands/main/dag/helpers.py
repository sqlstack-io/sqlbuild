from __future__ import annotations

from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.models import QueryResult
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class NoConnectDuckDbAdapter(DuckDbAdapter):
    """DuckDB adapter test double that fails if dag opens a connection."""

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        raise AssertionError("dag should not connect")

    def _execute(self, connection: Any, sql: str) -> Any:
        del connection, sql
        raise AssertionError("dag should not execute SQL")

    def query(self, connection: Any, sql: str, *, limit: int | None) -> QueryResult:
        del connection, sql, limit
        raise AssertionError("dag should not query")

    def close(self, connection: Any) -> None:
        del connection
        raise AssertionError("dag should not close a connection")


def prepare_static_dag_project(root: Path) -> Path:
    """Create a minimal local project for dag command tests."""

    project_dir: Path = root / "project"
    models_dir: Path = project_dir / "sql" / "models"
    models_dir.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "dag_project"',
                'adapter = "duckdb"',
                'default_target = "dev"',
                "",
                "[targets.dev]",
                'schema = "analytics"',
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (models_dir / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    return project_dir


def prepare_advisory_placement_dag_project(root: Path) -> Path:
    """Create a dag project with an advisory over-broad global declaration."""

    project_dir: Path = prepare_static_dag_project(root)
    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8") + "\n[scopes]\nenforce_placement = false\n",
        encoding="utf-8",
    )
    constants_dir: Path = project_dir / "sql" / "constants"
    constants_dir.mkdir()
    (constants_dir / "value.sql").write_text("CONSTANT (name value, value 1);\n", encoding="utf-8")
    (project_dir / "sql" / "models" / "orders.sql").write_text(
        'MODEL (description "Test model orders.", '
        'materialized table);\n\nSELECT @const("value") AS order_id\n',
        encoding="utf-8",
    )
    return project_dir


def prepare_python_dag_project(root: Path) -> Path:
    """Create a local project with Python assets and checks."""

    project_dir: Path = prepare_static_dag_project(root)
    tasks_dir: Path = project_dir / "python" / "nodes" / "tasks"
    assets_dir: Path = project_dir / "python" / "nodes" / "assets"
    loaders_dir: Path = project_dir / "python" / "nodes" / "loaders"
    checks_dir: Path = project_dir / "python" / "nodes" / "checks"
    tasks_dir.mkdir(parents=True)
    assets_dir.mkdir(parents=True)
    loaders_dir.mkdir(parents=True)
    checks_dir.mkdir(parents=True)
    (tasks_dir / "prepare_orders.py").write_text(
        "\n".join(
            (
                "from sqlbuild.assets import asset",
                "from sqlbuild.refs import model",
                "",
                "@asset(",
                "    depends_on=model('orders'),",
                "    tags=['daily'],",
                "    group='python',",
                "    meta={'owner': 'data'},",
                ")",
                "def prepare_orders(ctx):",
                "    '''Test task prepare_orders.'''",
                "    return ctx.result(payload={'rows': 1}, metadata={'source': 'fixture'})",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (loaders_dir / "warehouse_export.py").write_text(
        "\n".join(
            (
                "from sqlbuild.loaders import loader",
                "from python.nodes.tasks.prepare_orders import prepare_orders",
                "",
                "@loader(",
                "    depends_on=(prepare_orders,),",
                "    columns=[{'name': 'order_id', 'type': 'integer'}],",
                ")",
                "def warehouse_export(ctx):",
                "    '''Test loader warehouse_export.'''",
                "    return [{'order_id': 1}]",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (assets_dir / "orders_export.py").write_text(
        "\n".join(
            (
                "from sqlbuild.assets import asset",
                "from python.nodes.loaders.warehouse_export import warehouse_export",
                "from python.nodes.tasks.prepare_orders import prepare_orders",
                "",
                "@asset(",
                "    depends_on=(prepare_orders, warehouse_export),",
                "    tags=['daily', 'external'],",
                "    group='exports',",
                "    description='Orders export artifact',",
                "    meta={'system': 'object_store'},",
                "    columns=[{'name': 'order_id', 'type': 'integer'}],",
                "    column_lineage={",
                "        'order_id': [{'node': 'prepare_orders', 'column': 'order_id'}]",
                "    },",
                ")",
                "def orders_export(ctx):",
                "    return ctx.result(materialized=True)",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (checks_dir / "check_orders_export.py").write_text(
        "\n".join(
            (
                "from sqlbuild.checks import check",
                "from python.nodes.assets.orders_export import orders_export",
                "from python.nodes.loaders.warehouse_export import warehouse_export",
                "from python.nodes.tasks.prepare_orders import prepare_orders",
                "",
                "@check(",
                "    depends_on=(orders_export, prepare_orders, warehouse_export),",
                "    tags=['daily'],",
                "    group='exports',",
                "    description='Orders export is present',",
                "    meta={'owner': 'quality'},",
                ")",
                "def check_orders_export(ctx):",
                "    return True",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (checks_dir / "check_loader_export.py").write_text(
        "\n".join(
            (
                "from sqlbuild.checks import check",
                "from python.nodes.loaders.warehouse_export import warehouse_export",
                "",
                "@check(depends_on=warehouse_export, tags=['loader'])",
                "def check_loader_export(ctx):",
                "    '''Test check check_loader_export.'''",
                "    return ctx.pass_(message='loader rows available')",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return project_dir
