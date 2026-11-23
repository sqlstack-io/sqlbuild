"""Expression schemas are memoized only within their owning compiler catalog."""

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    project_with,
    semantic_shapes,
)
from sqlbuild._native import run_cli as main
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.lineage.helpers import compiled_project
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    ExpressionBatchCase,
    ExpressionMemoCase,
    RestoredShapeCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import shape_batches


@pytest.mark.parametrize(
    "test_case",
    [ExpressionMemoCase("shared closed expression", "SELECT 1 AS id")],
    ids=lambda case: case.description,
)
def test_given_equal_source_expressions_when_compiling_then_infers_once_and_invalidates_changed_content(
    test_case: ExpressionMemoCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql/models/orders.sql").write_text(
        'MODEL (description "Test model orders.", materialized view); SELECT id FROM __source("raw_orders")'
    )
    (tmp_path / "sql" / "sources").mkdir()
    source: Path = tmp_path / "sql/sources/orders.yml"
    source.write_text(
        f"sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    expression: {test_case.expression}\n  - name: raw_customers\n    description: Test source raw_customers.\n    expression: {test_case.expression}\n"
    )
    _ = shape_batches(reset=True)
    args: list[str] = ["--project-dir", str(tmp_path), "compile", "--json"]
    assert main(args) == 0
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert payload["diagnostics"] == []
    assert shape_batches()[0] == test_case.expected_analysis_calls
    source.write_text(source.read_text().replace("AS id", "AS quantity"))
    assert main(args) == 1
    changed: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert [item["code"] for item in changed["diagnostics"]] == ["B002"]
    assert shape_batches()[0] == test_case.expected_analysis_calls * 2


@pytest.mark.parametrize(
    "test_case",
    [
        ExpressionBatchCase(
            description="distinct expressions share one native batch and later passes reuse it",
            orders_expression="SELECT 1 AS id",
            customers_expression="SELECT 2 AS customer_id",
            expected_batches=(("SELECT 1 AS id", "SELECT 2 AS customer_id"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_distinct_source_expressions_when_compiling_then_infers_them_in_one_batch(
    test_case: ExpressionBatchCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql/models/orders.sql").write_text(
        'MODEL (description "Test model orders.", materialized view); SELECT id FROM __source("raw_orders")'
    )
    (tmp_path / "sql" / "sources").mkdir()
    (tmp_path / "sql/sources/orders.yml").write_text(
        "sources:\n"
        f"  - name: raw_orders\n    description: Test source raw_orders.\n    expression: {test_case.orders_expression}\n"
        f"  - name: raw_customers\n    description: Test source raw_customers.\n    expression: {test_case.customers_expression}\n"
    )
    _ = shape_batches(reset=True)
    assert main(["--project-dir", str(tmp_path), "compile", "--json"]) == 0
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert payload["diagnostics"] == []
    assert shape_batches() == (
        len(test_case.expected_batches),
        sum(len(batch) for batch in test_case.expected_batches),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RestoredShapeCase(
            description="closed and open expression sources",
            expressions={
                "raw_orders": "SELECT 1 AS id, 'placed' AS status",
                "raw_events": "SELECT mystery_fn(1) AS event_id, NULL AS note",
                "raw_open": "SELECT * FROM somewhere",
            },
            expected_shapes={
                "raw_orders": {"id": "INT", "status": "TEXT"},
                "raw_events": {"event_id": "UNKNOWN", "note": "UNKNOWN"},
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_restored_project_without_catalog_when_reading_shapes_then_infers_natively(
    test_case: RestoredShapeCase, tmp_path: Path
) -> None:
    sources: str = "sources:\n" + "".join(
        f"  - name: {name}\n    description: Test source {name}.\n    expression: {expression}\n"
        for name, expression in test_case.expressions.items()
    )
    project: CompiledProjectView = compiled_project(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
            "sql/sources/sources.yml": sources,
        },
    )

    restored: dict[str, dict[str, str]] = semantic_shapes(
        project=project_with(project=project, binding_catalog=None)
    )

    assert {name: restored.get(name) for name in test_case.expected_shapes} == (
        test_case.expected_shapes
    )
    assert "raw_open" not in restored
    assert restored == semantic_shapes(project=project)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
