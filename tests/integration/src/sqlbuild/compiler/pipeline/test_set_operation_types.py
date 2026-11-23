"""Compile and execute widened set-operation outputs across dependency upgrades."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

import duckdb
import pytest
from duckdb import sqltypes

from sqlbuild._native import (
    CompiledLineageColumnFact,
    build_compiled_project,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    SetOperationLineageIntegrationTestCase,
    SetOperationTypeIntegrationTestCase,
)

_PROJECT_CONFIG: str = 'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
_WIDENED_TYPES: frozenset[str | None] = frozenset({None, "DOUBLE"})


@pytest.mark.parametrize(
    "test_case",
    (
        SetOperationTypeIntegrationTestCase(
            description="union all never narrows to the first branch",
            query_sql=(
                "SELECT CAST(1 AS INTEGER) AS amount UNION ALL SELECT CAST(2.5 AS DOUBLE) AS amount"
            ),
            allowed_inferred_types=_WIDENED_TYPES,
            expected_rows=((1.0,), (2.5,)),
        ),
        SetOperationTypeIntegrationTestCase(
            description="union all by name in a cte never narrows to the first branch",
            query_sql=(
                "WITH combined AS (SELECT CAST(1 AS INTEGER) AS amount UNION ALL BY NAME "
                "SELECT CAST(2.5 AS DOUBLE) AS amount) SELECT amount FROM combined"
            ),
            allowed_inferred_types=_WIDENED_TYPES,
            expected_rows=((1.0,), (2.5,)),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_mixed_branch_types_when_compiling_then_known_types_match_duckdb(
    test_case: SetOperationTypeIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_CONFIG, encoding="utf-8")
    models: Path = tmp_path / "sql" / "models"
    models.mkdir(parents=True)
    (models / "orders.sql").write_text(
        f"MODEL (description 'Test model orders.', materialized table);\n{test_case.query_sql}",
        encoding="utf-8",
    )

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
    )
    (model,) = project.models
    with duckdb.connect() as connection:
        result: duckdb.DuckDBPyConnection = connection.execute(model.query_sql)
        result_type: object = result.description[0][1]
        rows: list[tuple[object, ...]] = result.fetchall()

    assert project.diagnostics == ()
    assert model.inferred_columns is not None
    assert model.inferred_columns[0].type in test_case.allowed_inferred_types
    assert result_type == sqltypes.DOUBLE
    assert tuple(rows) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        SetOperationLineageIntegrationTestCase(
            description="type recovery keeps resolved cte lineage",
            expected_sources=frozenset({("orders", "amount"), ("refunds", "amount")}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_union_type_recovery_when_compiling_then_cte_lineage_is_preserved(
    test_case: SetOperationLineageIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_CONFIG, encoding="utf-8")
    models: Path = tmp_path / "sql" / "models"
    models.mkdir(parents=True)
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table, contract enforced, columns (amount (type DOUBLE)));\n"
        "SELECT CAST(2.5 AS DOUBLE) AS amount",
        encoding="utf-8",
    )
    (models / "combined_orders.sql").write_text(
        "MODEL (description 'Test model combined_orders.', materialized table, contract enforced, columns (amount (type DOUBLE)));\n"
        'WITH combined AS (SELECT CAST(amount AS DOUBLE) AS amount FROM __ref("orders") '
        'UNION ALL SELECT list_extract([amount], 1) AS amount FROM __ref("refunds")) '
        "SELECT amount FROM combined",
        encoding="utf-8",
    )
    (models / "refunds.sql").write_text(
        "MODEL (description 'Test model refunds.', materialized table, contract enforced, columns (amount (type DOUBLE)));\n"
        "SELECT CAST(1.25 AS DOUBLE) AS amount",
        encoding="utf-8",
    )

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
    )
    models_by_name: dict[str, CompiledModelView] = {model.name: model for model in project.models}
    lineage: Sequence[CompiledLineageColumnFact] | None = models_by_name[
        "combined_orders"
    ].fast_lineage_columns
    assert lineage is not None
    (amount,) = lineage

    assert project.diagnostics == ()
    assert frozenset(
        (source.resource_name, source.column_name) for source in amount.upstream_columns
    ) == (test_case.expected_sources)
