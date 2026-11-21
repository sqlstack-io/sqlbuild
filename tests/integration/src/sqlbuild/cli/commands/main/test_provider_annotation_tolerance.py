"""Provider parameters whose annotations cannot be resolved keep compiling."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredPythonNodeFunctions,
    discover_provider_classes,
    discover_python_node_functions,
)
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    ProviderAnnotationToleranceTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import write_provider_asset_project


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderAnnotationToleranceTestCase(
            description="provider class annotation is recorded",
            annotation="OrdersProvider",
            expected_exit_code=0,
            expected_annotation_class_name="OrdersProvider",
        ),
        ProviderAnnotationToleranceTestCase(
            description="annotation that get_type_hints rejects with TypeError counts as unannotated",
            annotation='"list[int](3)"',
            expected_exit_code=0,
            expected_annotation_class_name=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_provider_parameter_annotation_when_compiling_then_usage_is_recorded_without_failing(
    test_case: ProviderAnnotationToleranceTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_provider_asset_project(project_dir=tmp_path, annotation=test_case.annotation)

    discovered: DiscoveredPythonNodeFunctions = discover_python_node_functions(
        project_dir=tmp_path, providers=discover_provider_classes(project_dir=tmp_path)
    )
    exit_code: int = main(["--project-dir", str(tmp_path), "compile"])

    assert tuple(usage.annotation_class_name for usage in discovered.assets[0].provider_usages) == (
        test_case.expected_annotation_class_name,
    )
    assert exit_code == test_case.expected_exit_code, capsys.readouterr()
