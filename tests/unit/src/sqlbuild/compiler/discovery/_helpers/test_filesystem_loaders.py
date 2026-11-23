"""Tests for source loader discovery."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredLoaderFunction,
    PythonNodeDiscoveryError,
    discover_python_node_functions,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverLoaderFunctionsTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverLoaderFunctionsTestCase(
            description="discovers decorated source loaders from the python directory",
            files={
                "python/nodes/loaders/github.py": """
from sqlbuild.loaders import loader

@loader
def github_events(ctx):
    return []
""",
                "python/nodes/loaders/stripe.py": """
from sqlbuild.loaders import loader

@loader(destination="raw.customers")
def stripe_customers(ctx):
    return []
""",
            },
            expected_names=("github_events", "stripe_customers"),
            expected_targets=(None, "raw.customers"),
            expected_dependency_counts=(0, 0),
            expected_write_strategies=(None, None),
            expected_cursor_columns=(None, None),
            expected_unique_keys=((), ()),
            expected_column_names=((), ()),
            expected_contracts=(None, None),
        ),
        DiscoverLoaderFunctionsTestCase(
            description="discovers intermediate loader write and schema metadata",
            files={
                "python/nodes/loaders/events.py": """
from sqlbuild.loaders import loader

@loader(
    destination="staging.events",
    write_strategy="merge",
    cursor_column="updated_at",
    unique_key=["event_id", "updated_at"],
    columns=[
        {"name": "event_id", "type": "BIGINT"},
        {"name": "updated_at", "type": "TIMESTAMP"},
    ],
    contract="enforced",
)
def events(ctx):
    return []
""",
            },
            expected_names=("events",),
            expected_targets=("staging.events",),
            expected_dependency_counts=(0,),
            expected_write_strategies=("merge",),
            expected_cursor_columns=("updated_at",),
            expected_unique_keys=(("event_id", "updated_at"),),
            expected_column_names=(("event_id", "updated_at"),),
            expected_contracts=("enforced",),
        ),
        DiscoverLoaderFunctionsTestCase(
            description="discovers loader dependencies from decorator metadata",
            files={
                "python/nodes/loaders/events.py": """
from sqlbuild.loaders import loader

@loader
def fetch_events(ctx):
    return []

@loader(depends_on=[fetch_events])
def enriched_events(ctx):
    return []
""",
            },
            expected_names=("enriched_events", "fetch_events"),
            expected_targets=(None, None),
            expected_dependency_counts=(1, 0),
            expected_write_strategies=(None, None),
            expected_cursor_columns=(None, None),
            expected_unique_keys=((), ()),
            expected_column_names=((), ()),
            expected_contracts=(None, None),
        ),
        DiscoverLoaderFunctionsTestCase(
            description="discovers explicit loader names and dependencies",
            files={
                "python/nodes/loaders/events.py": """
from sqlbuild.loaders import loader

@loader(name="fetch_events")
def make_fetch(ctx):
    return []

@loader(name="enriched_events", depends_on=[make_fetch])
def make_enriched(ctx):
    return []
""",
            },
            expected_names=("enriched_events", "fetch_events"),
            expected_targets=(None, None),
            expected_dependency_counts=(1, 0),
            expected_write_strategies=(None, None),
            expected_cursor_columns=(None, None),
            expected_unique_keys=((), ()),
            expected_column_names=((), ()),
            expected_contracts=(None, None),
        ),
        DiscoverLoaderFunctionsTestCase(
            description="returns empty tuple when python directory does not exist",
            files={},
            expected_names=(),
            expected_targets=(),
            expected_dependency_counts=(),
            expected_write_strategies=(),
            expected_cursor_columns=(),
            expected_unique_keys=(),
            expected_column_names=(),
            expected_contracts=(),
        ),
        DiscoverLoaderFunctionsTestCase(
            description="ignores undecorated functions and init files",
            files={
                "python/nodes/loaders/__init__.py": "",
                "python/nodes/loaders/helpers.py": "def helper(): return None\n",
                "python/nodes/loaders/orders.py": """
from sqlbuild.loaders import loader

@loader
def orders(ctx):
    return []
""",
            },
            expected_names=("orders",),
            expected_targets=(None,),
            expected_dependency_counts=(0,),
            expected_write_strategies=(None,),
            expected_cursor_columns=(None,),
            expected_unique_keys=((),),
            expected_column_names=((),),
            expected_contracts=(None,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_dir_when_discovering_loaders_then_returns_expected(
    test_case: DiscoverLoaderFunctionsTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    result: tuple[DiscoveredLoaderFunction, ...] = tuple(
        discover_python_node_functions(project_dir=tmp_path).loaders
    )

    assert tuple(loader.name for loader in result) == test_case.expected_names
    assert tuple(loader.destination for loader in result) == test_case.expected_targets
    assert (
        tuple(len(loader.depends_on) for loader in result) == test_case.expected_dependency_counts
    )
    assert (
        tuple(getattr(loader.write_strategy, "value", None) for loader in result)
        == test_case.expected_write_strategies
    )
    assert tuple(loader.cursor_column for loader in result) == test_case.expected_cursor_columns
    assert tuple(loader.unique_key for loader in result) == test_case.expected_unique_keys
    actual_column_names: list[tuple[str, ...]] = []
    for loader in result:
        column_names: list[str] = []
        for column in loader.columns:
            column_names.append(column.name)
        actual_column_names.append(tuple(column_names))
    assert tuple(actual_column_names) == test_case.expected_column_names
    assert tuple(loader.contract for loader in result) == test_case.expected_contracts


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverLoaderFunctionsTestCase(
            description="raises clear error when loader file import fails",
            files={"python/nodes/loaders/broken.py": "import missing_loader_dependency\n"},
            expected_names=(),
            expected_targets=(),
            expected_dependency_counts=(),
            expected_write_strategies=(),
            expected_cursor_columns=(),
            expected_unique_keys=(),
            expected_column_names=(),
            expected_contracts=(),
            expected_error_fragment="Failed to import Python node file python/nodes/loaders/broken.py",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_import_error_when_discovering_loaders_then_raises_clear_error(
    test_case: DiscoverLoaderFunctionsTestCase,
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
        DiscoverLoaderFunctionsTestCase(
            description="loader descriptions come from the decorator or the docstring",
            files={
                "python/nodes/loaders/orders.py": '''
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    """Orders from the storefront export."""
    return []

@loader(description="Refunds from the payments export")
def raw_refunds(ctx):
    """Ignored because the decorator wins."""
    return []

@loader
def raw_returns(ctx):
    return []
''',
            },
            expected_names=("raw_orders", "raw_refunds", "raw_returns"),
            expected_descriptions=(
                "Orders from the storefront export.",
                "Refunds from the payments export",
                None,
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_descriptions_when_discovering_then_decorator_wins_over_docstring(
    test_case: DiscoverLoaderFunctionsTestCase, tmp_path: Path
) -> None:
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    result: tuple[DiscoveredLoaderFunction, ...] = tuple(
        sorted(discover_python_node_functions(project_dir=tmp_path).loaders, key=lambda x: x.name)
    )

    assert tuple(loader.name for loader in result) == test_case.expected_names
    assert tuple(loader.description for loader in result) == test_case.expected_descriptions
