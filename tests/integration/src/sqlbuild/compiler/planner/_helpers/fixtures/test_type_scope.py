from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledResourceType,
    Dialect,
    RelationFixtureCompletion,
    RelationFixturePlanningContext,
    build_compiled_project,
    build_relation_fixture_completion,
    build_relation_fixture_context,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.planner._helpers.fixtures._test_types import (
    FixtureTypeScopeTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner._helpers.fixtures.helpers import (
    unrelated_source_entries,
)


@pytest.mark.parametrize(
    "test_case",
    (
        FixtureTypeScopeTestCase(description="small metadata map", unrelated_relations=10),
        FixtureTypeScopeTestCase(description="large metadata map", unrelated_relations=200),
    ),
    ids=lambda case: case.description,
)
def test_given_unrelated_project_types_when_completing_fixture_then_eager_work_is_chain_local(
    test_case: FixtureTypeScopeTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "fixture_scope"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": (
                'MODEL (description "Order facts.");\n'
                'SELECT order_id, status FROM __source("raw_orders")'
            ),
            "sql/sources/raw_orders.yml": (
                "sources:\n  - name: raw_orders\n    description: Order input.\n"
                "    expression: SELECT 1 AS order_id, 'open' AS status\n"
                "    contract: enforced\n    columns:\n"
                "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
                "      - name: status\n        type: VARCHAR\n        nullable: true\n"
                + unrelated_source_entries(count=test_case.unrelated_relations)
            ),
        },
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=adapter
    )
    context: RelationFixturePlanningContext = build_relation_fixture_context(project=project)
    unrelated_key: tuple[CompiledResourceType, str] = (
        CompiledResourceType.SOURCE,
        "unrelated_orders_0",
    )
    fixture_key: tuple[CompiledResourceType, str] = (CompiledResourceType.SOURCE, "raw_orders")
    completed: RelationFixtureCompletion = build_relation_fixture_completion(
        project=project,
        dialect=Dialect(adapter),
        request=(
            ("orders",),
            ((CompiledResourceType.SOURCE, {"raw_orders": "SELECT 1 AS order_id"}),),
        ),
        planning_context=context,
    )
    assert completed.diagnostics == ()
    assert 'CAST(NULL AS VARCHAR) AS "status"' in completed.fixture_sql_by_key[fixture_key]
    assert context.expected_types(unrelated_key) == {"order_id": "INTEGER"}
    assert len(completed.expected_types) == test_case.expected_eager_relations
    assert (
        sum(len(columns) for columns in completed.expected_types.values())
        == test_case.expected_eager_columns
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
