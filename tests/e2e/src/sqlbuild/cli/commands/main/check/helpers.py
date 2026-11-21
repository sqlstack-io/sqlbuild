"""Helpers for sqb check e2e tests."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.main.check._test_types import CheckCommandTestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project


def prepare_python_check_project(*, tmp_path: Path) -> Path:
    """Create an inline project with task, asset, and loader-backed Python checks."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_check_project",
        repo_files={
            "sqlbuild_project.toml": """
name = "python_check_project"
adapter = "duckdb"

[connection]
database = "python_check_project.duckdb"
""",
            "python/tasks/orders.py": """
from sqlbuild.assets import asset


@asset()
def export_orders(ctx):
    '''Test task export_orders.'''
    return ctx.result(payload={"rows": 3}, metadata={"rows": 3})


@asset()
def export_customers(ctx):
    '''Test task export_customers.'''
    return ctx.result(payload={"rows": 2}, metadata={"rows": 2})
""",
            "python/assets/orders.py": """
from sqlbuild.assets import asset
from python.tasks.orders import export_orders


@asset(depends_on=export_orders)
def orders_asset(ctx):
    '''Test asset orders_asset.'''
    return ctx.result(payload={"asset_rows": 3}, metadata={"asset_rows": 3})
""",
            "python/checks/orders.py": """
from sqlbuild.checks import check
from python.assets.orders import orders_asset
from python.tasks.orders import export_customers, export_orders


@check(depends_on=export_orders)
def check_orders_export(ctx):
    '''Test check check_orders_export.'''
    return ctx.pass_(message="orders exported")


@check(depends_on=export_orders, severity="warn")
def warn_orders_export(ctx):
    '''Test check warn_orders_export.'''
    return ctx.fail(message="warning check failed")


@check(depends_on=orders_asset, tags=("asset",), group="python-checks")
def check_orders_asset(ctx):
    '''Test check check_orders_asset.'''
    return ctx.result_of(node_function=orders_asset).payload["asset_rows"] == 3


@check(depends_on=(export_orders, export_customers), tags=("multi",), group="python-checks")
def check_order_customer_exports(ctx):
    '''Test check check_order_customer_exports.'''
    return ctx.pass_(
        metadata={"orders": ctx.result_of(node_function=export_orders).metadata["rows"]}
    )


@check(depends_on=[export_orders], severity="error", tags=("failure",), group="python-checks")
def fail_orders_export(ctx):
    '''Test check fail_orders_export.'''
    return ctx.fail(message="orders export failed")


@check(depends_on=export_orders, severity="error", tags=("failure",), group="python-checks")
def false_orders_export(ctx):
    '''Test check false_orders_export.'''
    return False


@check(depends_on=export_orders, severity="error", tags=("failure",), group="python-checks")
def exception_orders_export(ctx):
    '''Test check exception_orders_export.'''
    raise RuntimeError("orders exception check failed")
""",
        },
    )


def prepare_terminal_loader_check_project(*, tmp_path: Path) -> Path:
    """Create a project whose check invalidly depends on a terminal source loader."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="terminal_loader_check_project",
        repo_files={
            "sqlbuild_project.toml": """
name = "terminal_loader_check_project"
adapter = "duckdb"

[connection]
database = "terminal_loader_check_project.duckdb"
""",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader


@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 1}]
""",
            "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
""",
            "python/checks/raw.py": """
from sqlbuild.checks import check
from python.loaders.raw import raw_orders


@check(depends_on=raw_orders)
def check_raw_orders_loader(ctx):
    '''Test check check_raw_orders_loader.'''
    return True
""",
        },
    )


def prepare_check_project_by_kind(*, tmp_path: Path, project_kind: str) -> Path:
    """Create the project fixture for a check command test case."""

    project_factories: dict[str, Callable[..., Path]] = {
        "direct": prepare_python_check_project,
        "terminal_loader": prepare_terminal_loader_check_project,
    }
    return project_factories[project_kind](tmp_path=tmp_path)


def resolve_check_command(*, project_dir: Path, command: tuple[str, ...]) -> tuple[str, ...]:
    """Resolve project-dir placeholders in command arguments."""

    return tuple(part.replace("{project_dir}", str(project_dir)) for part in command)


def assert_expected_file_fragments(*, project_dir: Path, test_case: CheckCommandTestCase) -> None:
    """Assert requested project-local files exist and contain fragments."""

    relative_path: str
    fragments: tuple[str, ...]
    for relative_path, fragments in test_case.expected_file_fragments:
        file_path: Path = project_dir / relative_path
        assert file_path.exists(), f"expected file to exist: {file_path}"
        contents: str = file_path.read_text(encoding="utf-8")
        fragment: str
        for fragment in fragments:
            assert fragment in contents
