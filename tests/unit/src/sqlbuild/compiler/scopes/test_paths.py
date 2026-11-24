"""Tests for project-relative scope path normalization."""

from __future__ import annotations

import pytest

from sqlbuild._native import InvalidScopePathError, normalize_scope_path
from tests.unit.src.sqlbuild.compiler.scopes._test_types import PathNormalizationCase


@pytest.mark.parametrize(
    "test_case",
    [
        PathNormalizationCase(
            description="posix_redundant_components",
            path="sql/models/./staging/orders/../stg_orders.sql",
            expected_path="sql/models/staging/stg_orders.sql",
        ),
        PathNormalizationCase(
            description="windows_separators",
            path=r"models\staging\orders\stg_orders.sql",
            expected_path="sql/models/staging/orders/stg_orders.sql",
        ),
        PathNormalizationCase(
            description="repeated_separators",
            path="sql/models//staging///stg_orders.sql",
            expected_path="sql/models/staging/stg_orders.sql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_posix_or_windows_path_when_normalizing_then_returns_canonical_posix_path(
    test_case: PathNormalizationCase,
) -> None:
    assert normalize_scope_path(path=test_case.path) == test_case.expected_path


@pytest.mark.parametrize(
    "test_case",
    [
        PathNormalizationCase("absolute", "/models/orders.sql", "project-relative"),
        PathNormalizationCase("windows_drive", "C:\\models\\orders.sql", "project-relative"),
        PathNormalizationCase("escaping", "sql/models/../../orders.sql", "escapes the project"),
    ],
    ids=lambda case: case.description,
)
def test_given_path_outside_project_when_normalizing_then_rejects_it(
    test_case: PathNormalizationCase,
) -> None:
    with pytest.raises(InvalidScopePathError, match=test_case.expected_path):
        normalize_scope_path(path=test_case.path)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
