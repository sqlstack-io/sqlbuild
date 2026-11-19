"""Tests for task and asset discovery."""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable
from pathlib import Path
from types import ModuleType

import pytest

from sqlbuild._native import (
    DiscoveredAssetFunction,
    DiscoveredCheckFunction,
    DiscoveredPythonNodeFunctions,
    PythonNodeDiscoveryError,
    PythonNodeIdentity,
    discover_python_node_functions,
)
from sqlbuild.compiler.python_nodes.main.identity import build_python_node_identity
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverCheckFunctionsTestCase,
    DiscoverPythonRootImportTestCase,
    DiscoverPythonRootNodesTestCase,
    DiscoverTaskAssetFunctionsTestCase,
    PythonRootHelperIdentityTestCase,
    PythonRootProjectIsolationTestCase,
    RestoredPythonPackageIdentityTestCase,
    UnrelatedPythonPackageTestCase,
)

_HELPER_PACKAGE_FILES: dict[str, str] = {
    "python/helpers/__init__.py": "",
    "python/helpers/values.py": """
from pathlib import Path

Path(__file__).resolve().parents[2].joinpath("init_count.txt").open("a").write("values\\n")
STATUS = "shipped"
""",
    "python/helpers/clean.py": """
from .values import STATUS


def normalize_status(value):
    return f"{value.strip().lower()}:{STATUS}"
""",
    "python/orders.py": """
import python.helpers.clean
from sqlbuild.assets import asset


@asset
def orders(ctx):
    return python.helpers.clean.normalize_status(" Shipped ")
""",
}


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverTaskAssetFunctionsTestCase(
            description="discovers decorated tasks and assets from explicit folders",
            files={
                "python/tasks/windows.py": """
from sqlbuild.assets import asset

@asset(tags=("api",), group="ingestion")
def fetch_window(ctx):
    '''Fetch an API window.'''
    return {"window": "today"}

def helper():
    return None
""",
                "python/assets/exports.py": """
from sqlbuild.assets import asset
from python.tasks.windows import fetch_window

@asset(
    depends_on=fetch_window,
    columns=[{"name": "customer_id", "type": "string"}],
    column_lineage={"customer_id": [{"node": "dim_customers", "column": "customer_id"}]},
)
def export_customers(ctx):
    return {"uri": "s3://exports/customers.parquet"}
""",
                "python/assets/__init__.py": """
from sqlbuild.assets import asset

@asset
def ignored_init_asset(ctx):
    return None
""",
            },
            expected_task_names=("fetch_window",),
            expected_task_dependency_counts=(0,),
            expected_task_tags=(("api",),),
            expected_asset_names=("export_customers",),
            expected_asset_dependency_counts=(1,),
            expected_asset_column_names=(("customer_id",),),
            expected_asset_lineage_columns=(("customer_id",),),
        ),
        DiscoverTaskAssetFunctionsTestCase(
            description="returns empty tuples when task and asset folders do not exist",
            files={},
            expected_task_names=(),
            expected_task_dependency_counts=(),
            expected_task_tags=(),
            expected_asset_names=(),
            expected_asset_dependency_counts=(),
            expected_asset_column_names=(),
            expected_asset_lineage_columns=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_dir_when_discovering_python_nodes_then_returns_expected(
    test_case: DiscoverTaskAssetFunctionsTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    tasks: tuple[DiscoveredAssetFunction, ...] = tuple(
        discover_python_node_functions(project_dir=tmp_path).assets
    )
    assets: tuple[DiscoveredAssetFunction, ...] = tuple(
        discover_python_node_functions(project_dir=tmp_path).assets
    )

    assert tuple(task.name for task in tasks) == test_case.expected_task_names
    assert (
        tuple(len(task.depends_on) for task in tasks) == test_case.expected_task_dependency_counts
    )
    assert tuple(task.tags for task in tasks) == test_case.expected_task_tags
    assert tuple(asset.name for asset in assets) == test_case.expected_asset_names
    assert tuple(len(asset.depends_on) for asset in assets) == (
        test_case.expected_asset_dependency_counts
    )
    actual_asset_column_names: list[tuple[str, ...]] = []
    for asset in assets:
        column_names: list[str] = []
        for column in asset.columns:
            column_names.append(column.name)
        actual_asset_column_names.append(tuple(column_names))
    assert tuple(actual_asset_column_names) == test_case.expected_asset_column_names
    assert (
        tuple(tuple((asset.column_lineage or {}).keys()) for asset in assets)
        == test_case.expected_asset_lineage_columns
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverTaskAssetFunctionsTestCase(
            description="raises clear error when task import fails",
            files={"python/tasks/broken.py": "import missing_task_dependency\n"},
            expected_task_names=(),
            expected_task_dependency_counts=(),
            expected_task_tags=(),
            expected_asset_names=(),
            expected_asset_dependency_counts=(),
            expected_asset_column_names=(),
            expected_asset_lineage_columns=(),
            expected_error_fragment="Failed to import Python node file",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_import_error_when_discovering_then_raises_clear_error(
    test_case: DiscoverTaskAssetFunctionsTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discover_python_node_functions(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverCheckFunctionsTestCase(
            description="discovers decorated checks from explicit folder",
            files={
                "python/assets/exports.py": """
from sqlbuild.assets import asset

@asset
def export_customers(ctx):
    return {"uri": "s3://exports/customers.parquet"}
""",
                "python/checks/exports.py": """
from sqlbuild.checks import check
from python.assets.exports import export_customers

@check(depends_on=export_customers, severity="warn", tags=("exports",))
def export_customers_exists(ctx):
    return True

def helper():
    return None
""",
                "python/checks/__init__.py": """
from sqlbuild.checks import check
from python.assets.exports import export_customers

@check(depends_on=export_customers)
def ignored_init_check(ctx):
    return True
""",
            },
            expected_check_names=("export_customers_exists",),
            expected_check_dependency_counts=(1,),
            expected_check_severities=("warn",),
            expected_check_tags=(("exports",),),
        ),
        DiscoverCheckFunctionsTestCase(
            description="returns empty tuple when check folder does not exist",
            files={},
            expected_check_names=(),
            expected_check_dependency_counts=(),
            expected_check_severities=(),
            expected_check_tags=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_dir_when_discovering_checks_then_returns_expected(
    test_case: DiscoverCheckFunctionsTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    checks: tuple[DiscoveredCheckFunction, ...] = tuple(
        discover_python_node_functions(project_dir=tmp_path).checks
    )

    assert tuple(check.name for check in checks) == test_case.expected_check_names
    assert tuple(len(check.depends_on) for check in checks) == (
        test_case.expected_check_dependency_counts
    )
    assert tuple(check.severity.value for check in checks) == test_case.expected_check_severities
    assert tuple(check.tags for check in checks) == test_case.expected_check_tags


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverPythonRootNodesTestCase(
            description="discovers every node kind in one module directly under python",
            files={
                "python/orders.py": """
from sqlbuild.assets import asset
from sqlbuild.checks import check
from sqlbuild.loaders import loader
from sqlbuild.assets import asset

@asset
def prepare_orders(ctx):
    return None

@loader(depends_on=[prepare_orders])
def raw_orders(ctx):
    return []

@asset(depends_on=prepare_orders)
def orders_export(ctx):
    return None

@check(depends_on=orders_export)
def orders_export_exists(ctx):
    return True
""",
            },
            expected_nodes=(
                ("loader", "raw_orders", "python/orders.py"),
                ("task", "prepare_orders", "python/orders.py"),
                ("asset", "orders_export", "python/orders.py"),
                ("check", "orders_export_exists", "python/orders.py"),
            ),
        ),
        DiscoverPythonRootNodesTestCase(
            description="discovers nodes in nested folders that import undecorated helpers",
            files={
                "python/_helpers.py": """
def export_uri(name):
    return f"s3://exports/{name}.parquet"
""",
                "python/orders/__init__.py": "",
                "python/orders/utils.py": """
from sqlbuild.assets import asset

def order_label(value):
    return str(value)

LABEL = task
""",
                "python/orders/ingest/loaders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    return []
""",
                "python/orders/pipeline/tasks.py": """
from python.orders.utils import order_label
from sqlbuild.assets import asset

@asset
def label_orders(ctx):
    return order_label(1)
""",
                "python/exports/assets.py": """
from python._helpers import export_uri
from python.orders.pipeline.tasks import label_orders
from sqlbuild.assets import asset

@asset(depends_on=label_orders)
def orders_export(ctx):
    return {"uri": export_uri("orders")}
""",
                "python/exports/deep/nested/checks.py": """
from python.exports.assets import orders_export
from sqlbuild.checks import check

@check(depends_on=orders_export)
def orders_export_exists(ctx):
    return True
""",
            },
            expected_nodes=(
                ("loader", "raw_orders", "python/orders/ingest/loaders.py"),
                ("task", "label_orders", "python/orders/pipeline/tasks.py"),
                ("asset", "orders_export", "python/exports/assets.py"),
                ("check", "orders_export_exists", "python/exports/deep/nested/checks.py"),
            ),
        ),
        DiscoverPythonRootNodesTestCase(
            description="treats helper-only python root as having no nodes",
            files={
                "python/_helpers.py": "def order_label(value):\n    return str(value)\n",
                "python/orders/utils.py": "REGION = 'emea'\n",
            },
            expected_nodes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_root_when_discovering_python_nodes_then_decorators_define_node_kinds(
    test_case: DiscoverPythonRootNodesTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    result: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=tmp_path)

    discovered: tuple[tuple[str, str, str], ...] = (
        *(("loader", node.name, node.relative_path.as_posix()) for node in result.loaders),
        *(("task", node.name, node.relative_path.as_posix()) for node in result.assets),
        *(("asset", node.name, node.relative_path.as_posix()) for node in result.assets),
        *(("check", node.name, node.relative_path.as_posix()) for node in result.checks),
    )
    assert discovered == test_case.expected_nodes


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverPythonRootImportTestCase(
            description="imports helper packages with relative imports once per discovery pass",
            files=_HELPER_PACKAGE_FILES,
            expected_task_names=("orders",),
            expected_init_count=1,
        ),
        DiscoverPythonRootImportTestCase(
            description="shares one helper module between nodes that import it absolutely",
            files={
                "python/_counter.py": """
from pathlib import Path

Path(__file__).resolve().parents[1].joinpath("init_count.txt").open("a").write("counter\\n")
""",
                "python/tasks/first.py": """
import python._counter
from sqlbuild.assets import asset


@asset
def first_task(ctx):
    return None
""",
                "python/tasks/second.py": """
from python import _counter
from sqlbuild.assets import asset


@asset
def second_task(ctx):
    return None
""",
            },
            expected_task_names=("first_task", "second_task"),
            expected_init_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_root_helpers_when_discovering_then_modules_import_under_package_names(
    test_case: DiscoverPythonRootImportTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.files)

    result: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=tmp_path)

    assert tuple(node.name for node in result.assets) == test_case.expected_task_names
    assert all(node.function.__module__.startswith("python.") for node in result.assets)
    init_lines: list[str] = (tmp_path / "init_count.txt").read_text().splitlines()
    assert len(init_lines) == test_case.expected_init_count


@pytest.mark.parametrize(
    "test_case",
    [
        PythonRootProjectIsolationTestCase(
            description="second project imports its own python helpers",
            first_files={
                "python/_values.py": "STATUS = 'first'\n",
                "python/orders.py": """
from python._values import STATUS
from sqlbuild.assets import asset


@asset
def orders(ctx):
    return STATUS
""",
            },
            second_files={
                "python/_values.py": "STATUS = 'second'\n",
                "python/orders.py": """
from python._values import STATUS
from sqlbuild.assets import asset


@asset
def orders(ctx):
    return STATUS
""",
            },
            expected_first_result="first",
            expected_second_result="second",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_projects_when_discovering_sequentially_then_python_modules_are_isolated(
    test_case: PythonRootProjectIsolationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    first_dir: Path = tmp_path / "first"
    second_dir: Path = tmp_path / "second"
    write_repo_files(first_dir, test_case.first_files)
    write_repo_files(second_dir, test_case.second_files)

    first: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=first_dir)
    second: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=second_dir)

    assert first.assets[0].function(None) == test_case.expected_first_result
    assert second.assets[0].function(None) == test_case.expected_second_result


@pytest.mark.parametrize(
    "test_case",
    [
        PythonRootHelperIdentityTestCase(
            description="editing a relative-import helper changes the dependent node identity",
            files=_HELPER_PACKAGE_FILES,
            edited_path="python/helpers/clean.py",
            original_text="value.strip().lower()",
            edited_text="value.strip().upper()",
            expected_dependency_path="python/helpers/clean.py",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_root_helper_edit_when_rediscovering_then_node_identity_changes(
    test_case: PythonRootHelperIdentityTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.files)
    before_task: DiscoveredAssetFunction = discover_python_node_functions(
        project_dir=tmp_path
    ).assets[0]
    before: PythonNodeIdentity = build_python_node_identity(
        node_type="task",
        node_name=before_task.name,
        function=before_task.function,
        project_dir=tmp_path,
    )
    edited: Path = tmp_path / test_case.edited_path
    edited.write_text(
        edited.read_text(encoding="utf-8").replace(test_case.original_text, test_case.edited_text),
        encoding="utf-8",
    )

    after_task: DiscoveredAssetFunction = discover_python_node_functions(
        project_dir=tmp_path
    ).assets[0]
    after: PythonNodeIdentity = build_python_node_identity(
        node_type="task",
        node_name=after_task.name,
        function=after_task.function,
        project_dir=tmp_path,
    )

    assert before.source_hash == after.source_hash
    assert before.version_hash != after.version_hash
    assert test_case.expected_dependency_path in {
        dependency.source_path for dependency in after.dependencies
    }
    assert test_case.edited_text in after.metadata_json


@pytest.mark.parametrize(
    "test_case",
    [
        RestoredPythonPackageIdentityTestCase(
            description="helper imported through the project package stays a dependency",
            unrelated_files={"site/python/__init__.py": "", "site/python/orders.py": "X = 1\n"},
            project_files=_HELPER_PACKAGE_FILES,
            expected_dependency_path="python/helpers/clean.py",
        ),
        RestoredPythonPackageIdentityTestCase(
            description="helper called from a helper class method stays a dependency",
            unrelated_files={"site/python/__init__.py": "", "site/python/orders.py": "X = 1\n"},
            project_files={
                "python/helpers/__init__.py": "",
                "python/helpers/text.py": """
def strip_status(value):
    return value.strip().lower()
""",
                "python/helpers/cleaner.py": """
from .text import strip_status


class Cleaner:
    def run(self, value):
        return strip_status(value)
""",
                "python/orders.py": """
import python.helpers.cleaner
from sqlbuild.assets import asset


@asset
def orders(ctx):
    return python.helpers.cleaner.Cleaner().run(" Shipped ")
""",
            },
            expected_dependency_path="python/helpers/text.py",
        ),
        RestoredPythonPackageIdentityTestCase(
            description="a class attribute whose attribute lookup loops does not stop identity",
            unrelated_files={"site/python/__init__.py": "", "site/python/orders.py": "X = 1\n"},
            project_files={
                "python/helpers/__init__.py": "",
                "python/helpers/text.py": """
def strip_status(value):
    return value.strip().lower()
""",
                "python/helpers/cleaner.py": """
from .text import strip_status


class Lazy:
    def __getattr__(self, name):
        return self


class Cleaner:
    lazy = Lazy()

    def run(self, value):
        return strip_status(value)
""",
                "python/orders.py": """
import python.helpers.cleaner
from sqlbuild.assets import asset


@asset
def orders(ctx):
    return python.helpers.cleaner.Cleaner().run(" Shipped ")
""",
            },
            expected_dependency_path="python/helpers/text.py",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_python_package_restored_when_building_identity_then_helpers_are_dependencies(
    test_case: RestoredPythonPackageIdentityTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    project_dir: Path = tmp_path / "project"
    write_repo_files(tmp_path, test_case.unrelated_files)
    write_repo_files(project_dir, test_case.project_files)
    monkeypatch.delitem(sys.modules, "python", raising=False)
    monkeypatch.delitem(sys.modules, "python.orders", raising=False)
    monkeypatch.syspath_prepend(str(tmp_path / "site"))
    monkeypatch.setitem(sys.modules, "python", importlib.import_module("python"))
    monkeypatch.setitem(sys.modules, "python.orders", importlib.import_module("python.orders"))

    task: DiscoveredAssetFunction = discover_python_node_functions(project_dir=project_dir).assets[
        0
    ]
    identity: PythonNodeIdentity = build_python_node_identity(
        node_type="task",
        node_name=task.name,
        function=task.function,
        project_dir=project_dir,
    )

    assert test_case.expected_dependency_path in {
        dependency.source_path for dependency in identity.dependencies
    }


@pytest.mark.parametrize(
    "test_case",
    [
        UnrelatedPythonPackageTestCase(
            description="project python package wins over an imported unrelated python package",
            unrelated_files={
                "site/python/__init__.py": "UNRELATED = True\n",
                "site/python/other.py": "VALUE = 'unrelated'\n",
            },
            project_files=_HELPER_PACKAGE_FILES,
            expected_task_names=("orders",),
            expected_task_result="shipped:shipped",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_python_package_when_discovering_then_project_resolves_and_package_returns(
    test_case: UnrelatedPythonPackageTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    project_dir: Path = tmp_path / "project"
    write_repo_files(tmp_path, test_case.unrelated_files)
    write_repo_files(project_dir, test_case.project_files)
    monkeypatch.delitem(sys.modules, "python", raising=False)
    monkeypatch.delitem(sys.modules, "python.other", raising=False)
    monkeypatch.syspath_prepend(str(tmp_path / "site"))
    unrelated: ModuleType = importlib.import_module("python")
    unrelated_other: ModuleType = importlib.import_module("python.other")
    monkeypatch.setitem(sys.modules, "python", unrelated)
    monkeypatch.setitem(sys.modules, "python.other", unrelated_other)

    result: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=project_dir)

    assert tuple(node.name for node in result.assets) == test_case.expected_task_names
    assert result.assets[0].function(None) == test_case.expected_task_result
    assert sys.modules["python"] is unrelated
    assert sys.modules["python.other"] is unrelated_other
