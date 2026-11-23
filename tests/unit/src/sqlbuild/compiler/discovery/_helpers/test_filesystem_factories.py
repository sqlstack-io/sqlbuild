"""Tests for Python-node factory discovery."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredPythonNodeFunctions,
    PythonNodeDiscoveryError,
    discover_python_node_functions,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverPythonNodeFactoriesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers mixed nodes returned by a factory",
            files={
                "python/nodes/factories/generated/factory.py": """
from sqlbuild.assets import asset
from sqlbuild.checks import check
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


def make_task(name):
    @asset(name=name)
    def generated_task(ctx):
        return {"name": name}
    return generated_task


def make_asset(name, dependency):
    @asset(name=name, depends_on=dependency)
    def generated_asset(ctx):
        return {"name": name}
    return generated_asset


def make_loader(name, dependency):
    @loader(name=name, depends_on=[dependency])
    def generated_loader(ctx):
        return []
    return generated_loader


def make_check(name, dependency):
    @check(name=name, depends_on=dependency)
    def generated_check(ctx):
        return True
    return generated_check


@factory
def regional_pipeline():
    task_node = make_task("fetch_orders")
    asset_node = make_asset("orders_export", task_node)
    loader_node = make_loader("raw_orders", task_node)
    check_node = make_check("orders_export_exists", asset_node)
    return [task_node, asset_node, loader_node, check_node]
""",
            },
            expected_loader_names=("raw_orders",),
            expected_asset_names=("fetch_orders", "orders_export"),
            expected_check_names=("orders_export_exists",),
            expected_loader_dependency_counts=(1,),
            expected_asset_dependency_counts=(0, 1),
            expected_check_dependency_counts=(1,),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers tuple nodes returned by a factory",
            files={
                "python/nodes/loaders/generated/loaders.py": """
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


def make_loader(name):
    @loader(name=name)
    def generated_loader(ctx):
        return []
    return generated_loader


@factory
def generated_loaders():
    return (make_loader("raw_orders"), make_loader("raw_customers"))
""",
            },
            expected_loader_names=("raw_orders", "raw_customers"),
            expected_asset_names=(),
            expected_check_names=(),
            expected_loader_dependency_counts=(0, 0),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers set node returned by a factory",
            files={
                "python/nodes/checks/generated.py": """
from sqlbuild.checks import check
from sqlbuild.factories import factory


@factory
def generated_checks():
    @check(name="orders_export_exists", depends_on=())
    def generated_check(ctx):
        return True
    return {generated_check}
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=("orders_export_exists",),
            expected_check_dependency_counts=(0,),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers single node returned by a factory in assets folder",
            files={
                "python/nodes/assets/generated/assets.py": """
from sqlbuild.assets import asset
from sqlbuild.factories import factory


@factory
def generated_asset():
    @asset(name="orders_export")
    def export(ctx):
        return {"ok": True}
    return export
""",
            },
            expected_loader_names=(),
            expected_asset_names=("orders_export",),
            expected_check_names=(),
            expected_asset_dependency_counts=(0,),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers factory in loaders folder",
            files={
                "python/nodes/loaders/generated/loaders.py": """
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def generated_loader():
    @loader(name="raw_orders")
    def load(ctx):
        return []
    return load
""",
            },
            expected_loader_names=("raw_orders",),
            expected_asset_names=(),
            expected_check_names=(),
            expected_loader_dependency_counts=(0,),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers node kind from its decorator regardless of python subfolder",
            files={
                "python/nodes/checks/generated.py": """
from sqlbuild.assets import asset


@asset(name="orders_export")
def export(ctx):
    return {}
""",
            },
            expected_loader_names=(),
            expected_asset_names=("orders_export",),
            expected_check_names=(),
            expected_asset_dependency_counts=(0,),
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="discovers mixed-kind factory next to direct nodes using a shared helper",
            files={
                "python/nodes/_helpers.py": """
REGION = "emea"
""",
                "python/nodes/orders/pipeline/prepare_orders/asset.py": (
                    "from python.nodes._helpers import REGION\n"
                    "from sqlbuild.checks import check\n"
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.assets import asset\n"
                    "\n"
                    "\n"
                    '@asset(name="prepare_orders")\n'
                    "def prepare_orders(ctx):\n"
                    '    return {"region": REGION}\n'
                ),
                "python/nodes/orders/pipeline/regional_checks/checks.py": (
                    "from python.nodes._helpers import REGION\n"
                    "from sqlbuild.checks import check\n"
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.assets import asset\n"
                    "from python.nodes.orders.pipeline.prepare_orders.asset import prepare_orders\n"
                    "\n"
                    "\n"
                    "@factory\n"
                    "def regional_checks():\n"
                    '    @check(name=f"orders_{REGION}_exists", depends_on=prepare_orders)\n'
                    "    def generated_check(ctx):\n"
                    "        return True\n"
                    "    return generated_check\n"
                ),
                "python/nodes/orders/pipeline/__init__.py": (
                    "from .prepare_orders.asset import prepare_orders\n"
                    "from .regional_checks.checks import regional_checks\n"
                ),
            },
            expected_loader_names=(),
            expected_asset_names=("prepare_orders",),
            expected_check_names=("orders_emea_exists",),
            expected_asset_dependency_counts=(0,),
            expected_check_dependency_counts=(1,),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_factory_nodes_when_discovering_python_nodes_then_returns_generated_nodes(
    test_case: DiscoverPythonNodeFactoriesTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.files)

    result: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=tmp_path)

    assert tuple(node.name for node in result.loaders) == test_case.expected_loader_names
    assert tuple(node.name for node in result.assets) == test_case.expected_asset_names
    assert tuple(node.name for node in result.checks) == test_case.expected_check_names
    assert tuple(len(node.depends_on) for node in result.loaders) == (
        test_case.expected_loader_dependency_counts
    )
    assert tuple(len(node.depends_on) for node in result.assets) == (
        test_case.expected_asset_dependency_counts
    )
    assert tuple(len(node.depends_on) for node in result.checks) == (
        test_case.expected_check_dependency_counts
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns invalid shape",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    return "not a node"
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must return a SQLBuild node function",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns none",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    return None
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must return a SQLBuild node function",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns dict",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    return {"node": object()}
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must return a SQLBuild node function",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns bytes",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    return b"not a node"
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must return a SQLBuild node function",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns object",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    return object()
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must return a SQLBuild node function",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns class",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


class NotANode:
    pass


@factory
def broken_factory():
    return NotANode
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="returned item 0 that is not a SQLBuild asset",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returns nested structures",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.assets import asset


@asset(name="generated_task")
def generated_task(ctx):
    return None


@factory
def broken_factory():
    return [[generated_task]]
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="returned item 0 that is not a SQLBuild asset",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory returned item is not decorated",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


def helper(ctx):
    return None


@factory
def broken_factory():
    return [helper]
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="returned item 0 that is not a SQLBuild asset",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory requires arguments",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory(region):
    return []
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="must not require arguments",
        ),
        DiscoverPythonNodeFactoriesTestCase(
            description="raises when factory body fails",
            files={
                "python/nodes/tasks/generated.py": """
from sqlbuild.factories import factory


@factory
def broken_factory():
    raise RuntimeError("boom")
""",
            },
            expected_loader_names=(),
            expected_asset_names=(),
            expected_check_names=(),
            expected_error_fragment="failed during discovery: boom",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_factory_when_discovering_python_nodes_then_raises(
    test_case: DiscoverPythonNodeFactoriesTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.files)

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discover_python_node_functions(project_dir=tmp_path)
