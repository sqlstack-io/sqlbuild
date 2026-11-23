"""Completed star shapes survive partial compact-cache reuse."""

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import ShapeCacheCase


@pytest.mark.parametrize(
    "test_case",
    [
        ShapeCacheCase(
            "derived star", 'SELECT * FROM (SELECT id, quantity FROM __source("raw_orders")) q'
        ),
        ShapeCacheCase(
            "CTE star",
            'WITH q AS (SELECT id, quantity FROM __source("raw_orders")) SELECT * FROM q',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_completed_shape_when_an_unrelated_model_changes_then_cached_columns_and_lineage_survive(
    test_case: ShapeCacheCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sql" / "sources").mkdir()
    (tmp_path / "sql/sources/orders.yml").write_text(
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    table: orders\n    contract: enforced\n    columns:\n      - name: id\n        type: INTEGER\n      - name: quantity\n        type: INTEGER\n"
    )
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql/models/order_inputs.sql").write_text(
        'MODEL (description "Test model order_inputs.", materialized view); SELECT id, quantity FROM __source("raw_orders")'
    )
    (tmp_path / "sql/models/orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view);\n"
        + test_case.sql.replace('__source("raw_orders")', '__ref("order_inputs")')
    )
    edited: Path = tmp_path / "sql/models/customers.sql"
    edited.write_text(
        "MODEL (description 'Test model customers.', materialized view); SELECT 1 AS id"
    )
    args: list[str] = ["--project-dir", str(tmp_path), "compile", "--json"]
    assert main(args) == 0
    cold: dict[str, Any] = json.loads(capsys.readouterr().out)
    cold_models: dict[str, dict[str, Any]] = {
        model["name"]: model for model in cold["resources"]["models"]
    }
    expected: dict[str, Any] = cold_models["orders"]
    assert expected["column_count"] == test_case.expected_columns
    assert expected["lineage"]["has_star"] is test_case.expected_star
    edited.write_text(edited.read_text() + "\n-- unrelated edit\n")
    assert main(args) == 0
    changed: dict[str, Any] = json.loads(capsys.readouterr().out)
    changed_models: dict[str, dict[str, Any]] = {
        model["name"]: model for model in changed["resources"]["models"]
    }
    actual: dict[str, Any] = changed_models["orders"]
    assert actual == expected
    assert main([*args, "--no-cache"]) == 0
    oracle: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert changed["resources"] == oracle["resources"]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
