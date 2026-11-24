"""Compile selection on the native project graph built from the compiled project."""

from pathlib import Path

import pytest

import sqlbuild._native as native_module
from sqlbuild.cli.entry.main.entry import main
from tests.integration.src.sqlbuild.compiler.graph._test_types import (
    NativeGraphSelectionTestCase,
    NativeGraphSelectorErrorTestCase,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import (
    MODEL_NAMES,
    ORDERS_PROJECT_FILES,
    compiled_models,
    write_files,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeGraphSelectionTestCase(
            description="no selection builds one graph for the compiled project",
            select=(),
            expected_models=frozenset({"stg_orders", "orders", "customers"}),
            expected_graph_builds=1,
        ),
        NativeGraphSelectionTestCase(
            description="tag with downstream expansion",
            select=("--select", "tag:daily+"),
            expected_models=frozenset({"stg_orders", "orders"}),
            expected_graph_builds=2,
        ),
        NativeGraphSelectionTestCase(
            description="models-rooted folder",
            select=("--select", "sql/models/marts"),
            expected_models=frozenset({"orders", "customers"}),
            expected_graph_builds=2,
        ),
        NativeGraphSelectionTestCase(
            description="name glob minus an exclusion",
            select=("--select", "*orders", "--exclude", "stg_orders"),
            expected_models=frozenset({"orders"}),
            expected_graph_builds=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compile_selection_when_compiling_then_native_graph_selects_models(
    test_case: NativeGraphSelectionTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_files(project_dir=tmp_path, files=ORDERS_PROJECT_FILES)
    _ = native_module.compile_stage_work_info(reset=True)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "compile", "--no-cache", *test_case.select]
    )

    assert exit_code == 0
    assert compiled_models(stdout=capsys.readouterr().out) == {
        name: name in test_case.expected_models for name in sorted(MODEL_NAMES)
    }
    assert (
        native_module.compile_stage_work_info()["lineage_graph_builds"]
        == test_case.expected_graph_builds
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeGraphSelectorErrorTestCase(
            description="unknown name suggests the nearest model",
            select="ordrs",
            expected_error="error[S007]: unknown selector name 'ordrs'",
        ),
        NativeGraphSelectorErrorTestCase(
            description="folder outside the models root",
            select="marts/",
            expected_error="error[S012]: path selectors require an explicit root",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_compile_selector_when_compiling_then_planner_error_is_reported(
    test_case: NativeGraphSelectorErrorTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_files(project_dir=tmp_path, files=ORDERS_PROJECT_FILES)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "compile", "--select", test_case.select]
    )
    output: str = "".join(capsys.readouterr())

    assert exit_code == 1
    assert test_case.expected_error in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
