"""Multiple source readers share one metadata batch per database."""

from __future__ import annotations

from collections.abc import Callable
from contextlib import AbstractContextManager, nullcontext
from pathlib import Path
from unittest.mock import Mock

import duckdb
import pytest

from sqlbuild._native import (
    SourceEntry,
    build_compiled_project,
    discover_project_inputs,
    get_semantic_source_columns,
    list_source_candidates,
    live_reads,
)
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapter.relations.main.open_inspection_catalog import open_inspection_catalog
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeRelation,
    build_offline_snowflake,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.warehouse._test_types import (
    SemanticSourceBatchCase,
    SemanticSourceSchemaCaseCase,
    SnowflakeSourceCandidateCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.warehouse.helpers import column_names


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticSourceBatchCase(
            description="two unqualified sources use one database batch",
            expected_batches=1,
            expected_sources=frozenset({"orders", "customers"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_missing_source_schemas_when_inspecting_then_metadata_is_batched(
    test_case: SemanticSourceBatchCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "source_batch"\nadapter = "duckdb"\n',
            "sql/sources/orders.yml": "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n  - name: customers\n    description: Test source customers.\n    table: customers\n",
            "sql/models/combined.sql": 'MODEL (description "Test model combined.");\nSELECT o.id FROM __source("orders") o JOIN __source("customers") c ON o.id = c.id',
        },
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=adapter
    )
    listings: Mock = Mock(wraps=adapter.list_relations)
    columns: Mock = Mock(wraps=adapter.get_columns_for_relations)
    monkeypatch.setattr(adapter, "list_relations", listings)
    monkeypatch.setattr(adapter, "get_columns_for_relations", columns)
    with duckdb.connect() as connection:
        connection.execute("CREATE TABLE orders(id INTEGER); CREATE TABLE customers(id INTEGER)")
        result: dict[str, tuple[ColumnInfo, ...]] = get_semantic_source_columns(
            project=project,
            reads=live_reads(adapter=adapter, connection=connection),
            source_read_map={source.name: source.source_entry for source in project.sources},
            columns={},
            selected_keys=frozenset(model.key for model in project.models),
        )
    assert frozenset(result) == test_case.expected_sources
    assert listings.call_count == test_case.expected_batches
    assert columns.call_count == test_case.expected_batches


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticSourceSchemaCaseCase(
            description="uppercase declared schema finds a lowercase duckdb schema",
            open_catalog=False,
            declared_schema="RAW",
            expected_columns={"orders": ("id", "status")},
        ),
        SemanticSourceSchemaCaseCase(
            description="uppercase declared schema finds it through the planning catalog",
            open_catalog=True,
            declared_schema="RAW",
            expected_columns={"orders": ("id", "status")},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_schema_case_differs_when_inspecting_then_columns_are_found(
    test_case: SemanticSourceSchemaCaseCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "source_case"\nadapter = "duckdb"\n',
            "sql/sources/orders.yml": (
                "sources:\n  - name: orders\n    description: Test source orders.\n"
                f"    schema: {test_case.declared_schema}\n    table: orders\n"
            ),
            "sql/models/open_orders.sql": 'MODEL (description "Test model open_orders.");\nSELECT id FROM __source("orders")',
        },
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=adapter
    )
    with duckdb.connect() as connection:
        connection.execute("CREATE SCHEMA raw; CREATE TABLE raw.orders(id INTEGER, status VARCHAR)")
        scope: AbstractContextManager[object] = (
            nullcontext(),
            open_inspection_catalog(adapter=adapter, connection=connection),
        )[test_case.open_catalog]
        with scope:
            result: dict[str, tuple[ColumnInfo, ...]] = get_semantic_source_columns(
                project=project,
                reads=live_reads(adapter=adapter, connection=connection),
                source_read_map={source.name: source.source_entry for source in project.sources},
                columns={},
                selected_keys=frozenset(model.key for model in project.models),
            )

    assert {name: column_names(columns) for name, columns in result.items()} == (
        test_case.expected_columns
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceCandidateCase(
            description="unquoted schema is answered from its SHOW listing",
            declared_schemas=("raw",),
            expected_candidates=(("analytics", "raw", "orders"),),
            expected_query_kinds=("show_tables", "show_views"),
        ),
        SnowflakeSourceCandidateCase(
            description="quoted lowercase schema falls back to a database-wide name lookup",
            declared_schemas=("landing",),
            expected_candidates=(
                ("analytics", "raw", "orders"),
                ("analytics", "landing", "orders"),
            ),
            expected_query_kinds=("show_schemas", "tables"),
        ),
        SnowflakeSourceCandidateCase(
            description="a relation found by SHOW and by the name lookup is listed once",
            declared_schemas=("raw", "landing"),
            expected_candidates=(
                ("analytics", "raw", "orders"),
                ("analytics", "landing", "orders"),
            ),
            expected_query_kinds=(
                "show_tables",
                "show_views",
                "show_schemas",
                "tables",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_source_schema_when_listing_candidates_then_finds_relation(
    test_case: SnowflakeSourceCandidateCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=(
            FakeRelation(database="ANALYTICS", schema="RAW", name="ORDERS"),
            FakeRelation(database="ANALYTICS", schema="landing", name="ORDERS"),
        )
    )

    with open_inspection_catalog(adapter=adapter, connection=connection):
        candidates: tuple[RelationInfo, ...] = list_source_candidates(
            reads=live_reads(adapter=adapter, connection=connection),
            database="analytics",
            entries=tuple(
                SourceEntry(
                    name=f"orders_{schema}", database="analytics", schema=schema, table="orders"
                )
                for schema in test_case.declared_schemas
            ),
        )

    assert tuple(relation.identity for relation in candidates) == test_case.expected_candidates
    assert sorted(query.kind for query in warehouse.queries) == sorted(
        test_case.expected_query_kinds
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
