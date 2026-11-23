from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.discovery.main.project_adapters import discover_project_adapters
from tests.unit.src.sqlbuild.adapter.discovery.main._test_types import (
    ProjectAdapterDiscoveryErrorTestCase,
    ProjectAdapterDiscoveryTestCase,
)
from tests.unit.src.sqlbuild.adapter.discovery.main.helpers import write_project_files

VALID_DUCKDB_ADAPTER: str = """
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class DuckDbPlusAdapter(DuckDbAdapter):
    adapter_name = "duckdb_plus"
"""

NESTED_DUCKDB_ADAPTER: str = """
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class NestedDuckDbAdapter(DuckDbAdapter):
    adapter_name = "nested_duckdb"
"""


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectAdapterDiscoveryTestCase(
            description="loads public adapter files recursively",
            files={
                "python/adapters/duckdb_plus.py": VALID_DUCKDB_ADAPTER,
                "python/adapters/warehouse/nested.py": NESTED_DUCKDB_ADAPTER,
            },
            expected_adapter_names=("duckdb_plus", "nested_duckdb"),
        ),
        ProjectAdapterDiscoveryTestCase(
            description="skips private files init files and private directories",
            files={
                "python/adapters/duckdb_plus.py": VALID_DUCKDB_ADAPTER,
                "python/adapters/__init__.py": NESTED_DUCKDB_ADAPTER,
                "python/adapters/_private.py": NESTED_DUCKDB_ADAPTER,
                "python/adapters/_shared/nested.py": NESTED_DUCKDB_ADAPTER,
            },
            expected_adapter_names=("duckdb_plus",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_adapter_files_when_discovering_then_returns_expected_adapters(
    tmp_path: Path,
    test_case: ProjectAdapterDiscoveryTestCase,
) -> None:
    write_project_files(project_dir=tmp_path, files=test_case.files)

    adapters: dict[str, type[StrictAdapter]] = discover_project_adapters(
        project_dir=tmp_path,
        reserved_names=test_case.reserved_names,
    )

    assert tuple(sorted(adapters)) == tuple(sorted(test_case.expected_adapter_names))


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectAdapterDiscoveryErrorTestCase(
            description="rejects duplicate adapter names",
            files={
                "python/adapters/one.py": VALID_DUCKDB_ADAPTER,
                "python/adapters/two.py": VALID_DUCKDB_ADAPTER,
            },
            expected_error_fragment="Duplicate project-local adapter_name 'duckdb_plus'",
        ),
        ProjectAdapterDiscoveryErrorTestCase(
            description="rejects built in adapter shadowing",
            files={
                "python/adapters/duckdb.py": VALID_DUCKDB_ADAPTER.replace("duckdb_plus", "duckdb"),
            },
            reserved_names=frozenset({"duckdb"}),
            expected_error_fragment="shadows a built-in adapter name",
        ),
        ProjectAdapterDiscoveryErrorTestCase(
            description="rejects adapter subclass without adapter name",
            files={
                "python/adapters/missing_name.py": """
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class MissingNameAdapter(DuckDbAdapter):
    pass
""",
            },
            expected_error_fragment="must define a non-empty string adapter_name",
        ),
        ProjectAdapterDiscoveryErrorTestCase(
            description="rejects non adapter class with adapter name",
            files={
                "python/adapters/not_adapter.py": """
class NotAdapter:
    adapter_name = "not_adapter"
""",
            },
            expected_error_fragment="defines adapter_name but does not subclass StrictAdapter",
        ),
        ProjectAdapterDiscoveryErrorTestCase(
            description="reports import errors with file path",
            files={"python/adapters/broken.py": "raise RuntimeError('boom')\n"},
            expected_error_fragment="Error importing project-local adapter module",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_project_adapter_files_when_discovering_then_raises_clear_error(
    tmp_path: Path,
    test_case: ProjectAdapterDiscoveryErrorTestCase,
) -> None:
    write_project_files(project_dir=tmp_path, files=test_case.files)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        discover_project_adapters(
            project_dir=tmp_path,
            reserved_names=test_case.reserved_names,
        )
