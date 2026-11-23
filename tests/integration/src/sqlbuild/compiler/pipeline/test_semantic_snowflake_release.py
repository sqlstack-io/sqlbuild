"""Synthetic Snowflake semantic regressions across native dependency upgrades."""

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import ColumnLineageMode, build_compiled_project, discover_project_inputs
from sqlbuild._native import run_cli as main
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    SnowflakeOutputInferenceCase,
    SnowflakeSemanticReleaseCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSemanticReleaseCase(
            "hierarchical pseudo-column",
            'SELECT LEVEL AS depth FROM __source("orders") CONNECT BY LEVEL <= 3',
        ),
        SnowflakeSemanticReleaseCase(
            "multi-argument count",
            'SELECT COUNT(order_id, quantity) AS present FROM __source("orders")',
        ),
        SnowflakeSemanticReleaseCase(
            "flatten argument input scope",
            'SELECT f.value AS item FROM __source("orders") o, LATERAL FLATTEN(input => PARSE_JSON(value):items) f',
        ),
        SnowflakeSemanticReleaseCase(
            "lateral subquery",
            'SELECT g.item FROM __source("orders") o, LATERAL FLATTEN(input => PARSE_JSON(o.value)) f, LATERAL (SELECT f.value AS item) g',
        ),
        SnowflakeSemanticReleaseCase(
            "timestamp subtypes",
            'SELECT CAST(ordered_at AS TIMESTAMP_NTZ) AS ntz, CAST(ordered_at AS TIMESTAMP_LTZ) AS ltz, CAST(ordered_at AS TIMESTAMP_TZ) AS tz FROM __source("orders")',
        ),
        SnowflakeSemanticReleaseCase(
            "comment-wrapped projection",
            'SELECT (/* output identity */ quantity) AS quantity FROM __source("orders")',
        ),
        SnowflakeSemanticReleaseCase(
            "implicit set conversion",
            "SELECT 'pending'::VARCHAR AS value UNION ALL SELECT 1",
            expected_codes=("B215",),
            expected_exit_code=1,
        ),
        SnowflakeSemanticReleaseCase(
            "directional accepted conversion", "SELECT TRUE AS value UNION ALL SELECT 1"
        ),
        SnowflakeSemanticReleaseCase(
            "directional rejected conversion",
            "SELECT 1 AS value UNION ALL SELECT TRUE",
            expected_codes=("B215",),
            expected_exit_code=1,
        ),
        SnowflakeSemanticReleaseCase(
            "left-folded chain", "SELECT TRUE AS value UNION ALL SELECT 1 UNION ALL SELECT 1.5"
        ),
        SnowflakeSemanticReleaseCase(
            "left-folded rejected chain",
            "SELECT 1 AS value UNION ALL SELECT TRUE UNION ALL SELECT 1.5",
            expected_codes=("B215",),
            expected_exit_code=1,
        ),
        SnowflakeSemanticReleaseCase(
            "intersect binds tighter",
            "SELECT TRUE AS value UNION SELECT 1 INTERSECT SELECT FALSE",
            expected_codes=("B215",),
            expected_exit_code=1,
        ),
        SnowflakeSemanticReleaseCase(
            "date difference return type",
            'SELECT DATEDIFF(day, ordered_on, ordered_on) > quantity AS later FROM __source("orders")',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_semantics_when_compiling_then_preserves_native_verdicts(
    test_case: SnowflakeSemanticReleaseCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql" / "sources").mkdir()
    (tmp_path / "sql/sources/orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n    contract: enforced\n    columns:\n"
        "      - {name: order_id, type: INTEGER}\n"
        "      - {name: quantity, type: INTEGER}\n"
        "      - {name: value, type: VARCHAR}\n"
        "      - {name: ordered_on, type: DATE}\n"
        "      - {name: ordered_at, type: TIMESTAMP}\n"
    )
    (tmp_path / "sql/models/report.sql").write_text(
        "MODEL (description 'Test model report.', materialized view, database warehouse, schema analytics);\n"
        + test_case.sql
    )
    assert (
        main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
        == test_case.expected_exit_code
    )
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert (
        tuple(sorted({item["code"] for item in payload["diagnostics"]})) == test_case.expected_codes
    )
    for item in payload["diagnostics"]:
        assert item.get("help")


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeOutputInferenceCase(
            "numeric argument does not imply numeric return", "SELECT HASH(1) AS value"
        ),
        SnowflakeOutputInferenceCase(
            "text argument does not imply text return",
            "SELECT REGEXP_COUNT('orders', 'o') AS value",
        ),
        SnowflakeOutputInferenceCase(
            "directional accumulated output", "SELECT TRUE AS value UNION ALL SELECT 1", "BOOLEAN"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_query_when_inferring_rich_types_then_preserves_proven_or_unknown_types(
    test_case: SnowflakeOutputInferenceCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "snowflake"\n')
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql/models/report.sql").write_text(
        "MODEL (description 'Test model report.', materialized view, database warehouse, schema analytics);\n"
        + test_case.sql
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=SnowflakeAdapter(),
        column_lineage_mode=ColumnLineageMode.RICH,
    )
    assert not project.diagnostics
    assert project.models[0].inferred_columns is not None
    assert project.models[0].inferred_columns[0].type == test_case.expected_type


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
