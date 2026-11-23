"""Tests for materialization file discovery."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredMaterializationFile,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    discover_materialization_files,
    discover_provider_classes,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverMaterializationFilesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverMaterializationFilesTestCase(
            description="discovers materialization files from materializations directory",
            files={
                "python/materializations/partition_tracked.py": "def materialize(ctx): pass\n",
                "python/materializations/atomic_swap.py": "def materialize(ctx): pass\n",
            },
            expected_names=("atomic_swap", "partition_tracked"),
        ),
        DiscoverMaterializationFilesTestCase(
            description="returns empty tuple when materializations directory does not exist",
            files={},
            expected_names=(),
        ),
        DiscoverMaterializationFilesTestCase(
            description="skips __init__ files in materializations directory",
            files={
                "python/materializations/__init__.py": "",
                "python/materializations/custom_one.py": "def materialize(ctx): pass\n",
            },
            expected_names=("custom_one",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_dir_when_discovering_materializations_then_returns_expected(
    test_case: DiscoverMaterializationFilesTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    result: tuple[DiscoveredMaterializationFile, ...] = discover_materialization_files(
        project_dir=tmp_path
    )

    actual_names: tuple[str, ...] = tuple(f.name for f in result)
    assert actual_names == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverMaterializationFilesTestCase(
            description="records custom materialization provider usage metadata",
            files={
                "python/providers/marker.py": """
from sqlbuild.providers import Provider


class MarkerProvider(Provider):
    pass
""".strip()
                + "\n",
                "python/materializations/copy_table.py": """
from python.nodes.providers.marker import MarkerProvider


def materialize(ctx, marker_provider: MarkerProvider):
    return None
""".strip()
                + "\n",
            },
            expected_names=("copy_table",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_materialization_provider_parameter_when_discovering_then_records_provider_usage(
    test_case: DiscoverMaterializationFilesTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    result: tuple[DiscoveredMaterializationFile, ...] = discover_materialization_files(
        project_dir=tmp_path,
        providers=providers,
    )

    assert tuple(materialization.name for materialization in result) == test_case.expected_names
    assert len(result[0].provider_usages) == 1
    usage: DiscoveredProviderUsage = result[0].provider_usages[0]
    assert usage.provider_name == "marker_provider"
    assert usage.parameter_name == "marker_provider"
    assert usage.annotation_class_name == "MarkerProvider"
    assert usage.annotation_module == "python.providers.marker"
