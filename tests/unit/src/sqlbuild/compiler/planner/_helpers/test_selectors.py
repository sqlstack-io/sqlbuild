from __future__ import annotations

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    project_lineage_views,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    BuildPathIndexTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import build_test_project


@pytest.mark.parametrize(
    "test_case",
    [
        BuildPathIndexTestCase(
            description="strips models prefix from relative paths",
            model_paths={
                "stg_orders": "sql/models/staging/stg_orders.sql",
                "fact_orders": "sql/models/marts/fact_orders.sql",
            },
            expected_folders={
                "stg_orders": "staging",
                "fact_orders": "marts",
            },
        ),
        BuildPathIndexTestCase(
            description="handles nested subdirectories",
            model_paths={
                "deep_model": "sql/models/staging/raw/deep_model.sql",
            },
            expected_folders={
                "deep_model": "staging/raw",
            },
        ),
        BuildPathIndexTestCase(
            description="handles models at top level models dir",
            model_paths={
                "top_model": "sql/models/top_model.sql",
            },
            expected_folders={
                "top_model": "",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_paths_when_building_path_index_then_returns_expected_folders(
    test_case: BuildPathIndexTestCase,
) -> None:
    project: CompiledProjectView = build_test_project(
        model_deps={name: () for name in test_case.model_paths},
        model_paths=test_case.model_paths,
    )
    result: dict[CompiledObjectKey, str] = project_lineage_views(project).path_index
    result_by_name: dict[str, str] = {key.name: folder for key, folder in result.items()}

    assert result_by_name == test_case.expected_folders
