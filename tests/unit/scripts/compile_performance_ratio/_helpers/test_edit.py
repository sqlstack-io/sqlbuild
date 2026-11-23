"""Tests for the generator-independent one-model benchmark edit."""

from pathlib import Path

import pytest

from scripts.compile_performance_ratio._helpers.edit import apply_one_model_edit
from scripts.compile_performance_ratio.exceptions import BenchmarkEditError
from tests.unit.scripts.compile_performance_ratio._helpers._test_types import (
    OneModelEditErrorTestCase,
    OneModelEditTestCase,
)
from tests.unit.scripts.compile_performance_ratio._helpers.helpers import (
    read_model_files,
    write_model_files,
)

_HEADER: str = 'MODEL (description "Orders.");\n\n'
_WITH_QUERY: str = "WITH base AS (SELECT 1 AS id)\nSELECT id FROM base\n"


@pytest.mark.parametrize(
    "test_case",
    (
        OneModelEditTestCase(
            description="the middle model by path gets a comment before its query",
            model_files={
                "sql/models/a/orders_a.sql": _HEADER + "SELECT 1 AS id\n",
                "sql/models/b/orders_b.sql": _HEADER + _WITH_QUERY,
                "sql/models/c/orders_c.sql": _HEADER + "SELECT 3 AS id\n",
            },
            revisions=1,
            expected_paths=("sql/models/b/orders_b.sql",),
            expected_files={
                "sql/models/a/orders_a.sql": _HEADER + "SELECT 1 AS id\n",
                "sql/models/b/orders_b.sql": _HEADER + "-- Benchmark edit 0.\n" + _WITH_QUERY,
                "sql/models/c/orders_c.sql": _HEADER + "SELECT 3 AS id\n",
            },
        ),
        OneModelEditTestCase(
            description="each revision adds a distinct comment so every run is a new edit",
            model_files={"sql/models/orders.sql": _HEADER + "SELECT 1 AS id\n"},
            revisions=2,
            expected_paths=("sql/models/orders.sql", "sql/models/orders.sql"),
            expected_files={
                "sql/models/orders.sql": _HEADER
                + "-- Benchmark edit 0.\n-- Benchmark edit 1.\nSELECT 1 AS id\n"
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_benchmark_project_when_editing_one_model_then_changes_only_the_middle_query(
    test_case: OneModelEditTestCase, tmp_path: Path
) -> None:
    write_model_files(project_dir=tmp_path, model_files=test_case.model_files)

    edited: tuple[str, ...] = tuple(
        apply_one_model_edit(project_dir=tmp_path, revision=revision)
        .relative_to(tmp_path)
        .as_posix()
        for revision in range(test_case.revisions)
    )

    assert edited == test_case.expected_paths
    assert read_model_files(project_dir=tmp_path) == test_case.expected_files


@pytest.mark.parametrize(
    "test_case",
    (
        OneModelEditErrorTestCase(
            description="a project without models is reported",
            model_files={},
            expected_message="has no model files to edit",
        ),
        OneModelEditErrorTestCase(
            description="a model without a query start is reported",
            model_files={"sql/models/orders.sql": _HEADER + "  select 1 as id\n"},
            expected_message="has no line starting a WITH or SELECT query to edit",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_without_editable_query_when_editing_one_model_then_raises(
    test_case: OneModelEditErrorTestCase, tmp_path: Path
) -> None:
    write_model_files(project_dir=tmp_path, model_files=test_case.model_files)

    with pytest.raises(BenchmarkEditError, match=test_case.expected_message):
        _ = apply_one_model_edit(project_dir=tmp_path, revision=0)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
