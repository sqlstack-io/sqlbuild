"""Tests for Python-node identity helpers."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from types import ModuleType

import pytest

from sqlbuild._native import PythonNodeIdentity
from sqlbuild.compiler.python_nodes.main.identity import build_python_node_identity
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonNodeIdentityChangeTestCase,
    PythonNodeIdentityTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    load_python_identity_module,
    write_python_identity_repo,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeIdentityTestCase(
            description="includes same-file helper dependency",
            repo_files={
                "python/nodes/tasks/orders.py": """
def normalize_order(value):
    return value.strip().lower()


def build_orders(ctx):
    return normalize_order(" Pending ")
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("normalize_order",),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def normalize_order(value):", "source_text"),
        ),
        PythonNodeIdentityTestCase(
            description="includes transitive imported first-party helper dependencies",
            repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers.cleaning import normalize_orders


def build_orders(ctx):
    return normalize_orders(ctx)
""".strip()
                + "\n",
                "python/nodes/helpers/cleaning.py": """
from python.nodes.helpers.status import clean_status


def normalize_orders(ctx):
    return clean_status(" Pending ")
""".strip()
                + "\n",
                "python/nodes/helpers/status.py": """
def clean_status(value):
    return value.strip().lower()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("normalize_orders", "clean_status"),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=(
                "def normalize_orders(ctx):",
                "def clean_status(value):",
                "source_text",
            ),
        ),
        PythonNodeIdentityTestCase(
            description="excludes helpers loaded from venv under git root",
            repo_files={
                "python/nodes/tasks/orders.py": """
import vendor_helpers


def build_orders(ctx):
    return vendor_helpers.normalize_order(" Pending ")
""".strip()
                + "\n",
                ".venv/lib/python3.12/site-packages/vendor_helpers.py": """
def normalize_order(value):
    return value.strip().lower()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=(),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("dependencies",),
            unexpected_metadata_fragments=("def external(value):", "def normalize_order(value):"),
            extra_sys_paths=(".venv/lib/python3.12/site-packages",),
        ),
        PythonNodeIdentityTestCase(
            description="excludes helpers loaded from external third-party path",
            repo_files={
                "python/nodes/tasks/orders.py": """
import external_vendor_helpers


def build_orders(ctx):
    return external_vendor_helpers.normalize_order(" Pending ")
""".strip()
                + "\n",
                "../third_party/external_vendor_helpers.py": """
def normalize_order(value):
    return value.strip().lower()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=(),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("dependencies",),
            unexpected_metadata_fragments=("def normalize_order(value):",),
            extra_sys_paths=("../third_party",),
        ),
        PythonNodeIdentityTestCase(
            description="ignores stdlib builtins and dynamic references without crashing",
            repo_files={
                "python/nodes/tasks/orders.py": """
import json


def normalize_order(value):
    return value.strip().lower()


def build_orders(ctx):
    helper_name = "normalize_order"
    payload = json.dumps({"count": len([" Pending "])})
    return globals()[helper_name](str(payload))
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=(),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("dependencies",),
            unexpected_metadata_fragments=("def normalize_order(value):", "json.dumps"),
        ),
        PythonNodeIdentityTestCase(
            description="terminates cyclic helper traversal",
            repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers.cycle import helper_a


def build_orders(ctx):
    return helper_a(" Pending ")
""".strip()
                + "\n",
                "python/nodes/helpers/cycle.py": """
def helper_a(value):
    return helper_b(value)


def helper_b(value):
    return helper_a(value)
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("helper_a", "helper_b"),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def helper_a(value):", "def helper_b(value):"),
        ),
        PythonNodeIdentityTestCase(
            description="includes first-party module attribute helper dependency",
            repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers import cleaning


def build_orders(ctx):
    return cleaning.normalize_order(" Pending ")
""".strip()
                + "\n",
                "python/nodes/helpers/cleaning.py": """
def normalize_order(value):
    return value.strip().lower()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("normalize_order",),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def normalize_order(value):", "source_text"),
        ),
        PythonNodeIdentityTestCase(
            description="includes first-party class constructor dependency",
            repo_files={
                "python/nodes/tasks/orders.py": """
class Cleaner:
    def clean(self, value):
        return value.strip().lower()


def build_orders(ctx):
    return Cleaner().clean(" Pending ")
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("Cleaner",),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("class Cleaner:", "source_text"),
        ),
        PythonNodeIdentityTestCase(
            description="includes first-party class method dependency",
            repo_files={
                "python/nodes/tasks/orders.py": """
class Cleaner:
    @staticmethod
    def clean(value):
        return value.strip().lower()


def build_orders(ctx):
    return Cleaner.clean(" Pending ")
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("Cleaner.clean",),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def clean(value):", "source_text"),
        ),
        PythonNodeIdentityTestCase(
            description="sorts dependencies deterministically by source path and qualname",
            repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers.z_helpers import z_clean
from python.nodes.helpers.a_helpers import a_clean


def build_orders(ctx):
    return z_clean(a_clean(" Pending "))
""".strip()
                + "\n",
                "python/nodes/helpers/z_helpers.py": """
def z_clean(value):
    return value.strip()
""".strip()
                + "\n",
                "python/nodes/helpers/a_helpers.py": """
def a_clean(value):
    return value.lower()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=("a_clean", "z_clean"),
            expected_definition_fragments=("def build_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def a_clean(value):", "def z_clean(value):"),
        ),
        PythonNodeIdentityTestCase(
            description="stores decorator config in definition payload",
            repo_files={
                "python/nodes/tasks/orders.py": """
def build_orders(ctx):
    return None
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_source_path="python/nodes/tasks/orders.py",
            expected_dependency_qualnames=(),
            expected_definition_fragments=("orders", "daily", "source_text"),
            expected_metadata_fragments=("dependencies",),
            decorator_config={"tags": ["orders", "daily"]},
        ),
        PythonNodeIdentityTestCase(
            description="includes Python hook helper dependencies",
            repo_files={
                "sql/hooks/python/orders.py": """
from sqlbuild.hooks import hook


def audit_message():
    return "orders ready"


@hook(name="before_orders")
def before_orders(ctx):
    '''Test hook before_orders.'''
    return audit_message()
""".strip()
                + "\n",
            },
            entry_module_path="sql/hooks/python/orders.py",
            function_name="before_orders",
            node_type="hook",
            expected_source_path="sql/hooks/python/orders.py",
            expected_dependency_qualnames=("audit_message",),
            expected_definition_fragments=("def before_orders(ctx):", "source_text"),
            expected_metadata_fragments=("def audit_message():", "source_text"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_when_building_identity_then_includes_expected_first_party_sources(
    test_case: PythonNodeIdentityTestCase,
    tmp_path: Path,
) -> None:
    write_python_identity_repo(project_dir=tmp_path, repo_files=test_case.repo_files)
    module: ModuleType = load_python_identity_module(
        project_dir=tmp_path,
        module_path=test_case.entry_module_path,
        extra_sys_paths=test_case.extra_sys_paths,
    )
    function: Callable[..., object] = getattr(module, test_case.function_name)

    identity: PythonNodeIdentity = build_python_node_identity(
        node_type=test_case.node_type,
        node_name=test_case.function_name,
        function=function,
        project_dir=tmp_path,
        decorator_config=test_case.decorator_config,
    )

    assert identity.node_type == test_case.node_type
    assert identity.node_name == test_case.function_name
    assert identity.source_path == test_case.expected_source_path
    assert tuple(dependency.qualname for dependency in identity.dependencies) == (
        test_case.expected_dependency_qualnames
    )
    fragment: str
    for fragment in test_case.expected_definition_fragments:
        assert fragment in identity.definition_json
    for fragment in test_case.expected_metadata_fragments:
        assert fragment in identity.metadata_json
    for fragment in test_case.unexpected_metadata_fragments:
        assert fragment not in identity.metadata_json


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeIdentityChangeTestCase(
            description="nested function source changes update body and version identity",
            before_repo_files={
                "python/nodes/tasks/orders.py": """
def build_orders(ctx):
    def normalize_order(value):
        return value.strip().lower()

    return normalize_order(" Pending ")
""".strip()
                + "\n",
            },
            after_repo_files={
                "python/nodes/tasks/orders.py": """
def build_orders(ctx):
    def normalize_order(value):
        return value.strip().upper()

    return normalize_order(" Pending ")
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_definition_hash_changed=True,
            expected_version_hash_changed=True,
        ),
        PythonNodeIdentityChangeTestCase(
            description="first-party helper source changes update version identity only",
            before_repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers.cleaning import normalize_order


def build_orders(ctx):
    return normalize_order(" Pending ")
""".strip()
                + "\n",
                "python/nodes/helpers/cleaning.py": """
def normalize_order(value):
    return value.strip().lower()
""".strip()
                + "\n",
            },
            after_repo_files={
                "python/nodes/tasks/orders.py": """
from python.nodes.helpers.cleaning import normalize_order


def build_orders(ctx):
    return normalize_order(" Pending ")
""".strip()
                + "\n",
                "python/nodes/helpers/cleaning.py": """
def normalize_order(value):
    return value.strip().upper()
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_definition_hash_changed=False,
            expected_version_hash_changed=True,
        ),
        PythonNodeIdentityChangeTestCase(
            description="decorator config changes update body and version identity",
            before_repo_files={
                "python/nodes/tasks/orders.py": """
def build_orders(ctx):
    return None
""".strip()
                + "\n",
            },
            after_repo_files={
                "python/nodes/tasks/orders.py": """
def build_orders(ctx):
    return None
""".strip()
                + "\n",
            },
            entry_module_path="python/nodes/tasks/orders.py",
            function_name="build_orders",
            node_type="asset",
            expected_definition_hash_changed=True,
            expected_version_hash_changed=True,
            before_decorator_config={"tags": ["orders"]},
            after_decorator_config={"tags": ["orders", "daily"]},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_source_change_when_building_identity_then_hashes_change_as_expected(
    test_case: PythonNodeIdentityChangeTestCase,
    tmp_path: Path,
) -> None:
    before_dir: Path = tmp_path / "before"
    after_dir: Path = tmp_path / "after"
    before_dir.mkdir()
    after_dir.mkdir()
    write_python_identity_repo(project_dir=before_dir, repo_files=test_case.before_repo_files)
    write_python_identity_repo(project_dir=after_dir, repo_files=test_case.after_repo_files)
    before_module: ModuleType = load_python_identity_module(
        project_dir=before_dir,
        module_path=test_case.entry_module_path,
    )
    after_module: ModuleType = load_python_identity_module(
        project_dir=after_dir,
        module_path=test_case.entry_module_path,
    )

    before_identity: PythonNodeIdentity = build_python_node_identity(
        node_type=test_case.node_type,
        node_name=test_case.function_name,
        function=getattr(before_module, test_case.function_name),
        project_dir=before_dir,
        decorator_config=test_case.before_decorator_config,
    )
    after_identity: PythonNodeIdentity = build_python_node_identity(
        node_type=test_case.node_type,
        node_name=test_case.function_name,
        function=getattr(after_module, test_case.function_name),
        project_dir=after_dir,
        decorator_config=test_case.after_decorator_config,
    )

    assert (
        before_identity.definition_hash != after_identity.definition_hash
    ) is test_case.expected_definition_hash_changed
    assert (
        before_identity.version_hash != after_identity.version_hash
    ) is test_case.expected_version_hash_changed
