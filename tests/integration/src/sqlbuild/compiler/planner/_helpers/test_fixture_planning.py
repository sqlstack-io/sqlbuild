from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledResourceType,
    Dialect,
    DiscoveredProjectInputs,
    RelationFixtureCompletion,
    RelationFixturePlanningContext,
    SqlTestPlanEntry,
    SqlTestSelection,
    build_compiled_project,
    build_relation_fixture_completion,
    build_relation_fixture_context,
    build_selected_test_entries,
    discover_project_inputs,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.planner._helpers._test_types import (
    CaseFoldedFixtureFillTestCase,
    FixturePlanningIntegrationTestCase,
    FixtureSchemaIntegrationTestCase,
)

_ADAPTERS: dict[str, Callable[[], BaseAdapter]] = {
    "duckdb": DuckDbAdapter,
    "snowflake": SnowflakeAdapter,
}


@pytest.mark.parametrize(
    "test_case",
    (
        FixturePlanningIntegrationTestCase(
            description="partial source fixture",
            expected_sql_fragment='CAST(NULL AS VARCHAR) AS "status"',
        ),
        FixturePlanningIntegrationTestCase(
            description="qualified star through alias",
            query_sql='SELECT o.* FROM __source("raw_orders") AS o',
            expected_sql_fragment='CAST(NULL AS VARCHAR) AS "status"',
        ),
        FixturePlanningIntegrationTestCase(
            description="CTE and quoted column reads",
            query_sql='WITH selected AS (SELECT "order_id", "status" FROM __source("raw_orders")) SELECT * FROM selected',
            expected_sql_fragment='CAST(NULL AS VARCHAR) AS "status"',
        ),
        FixturePlanningIntegrationTestCase(
            description="marker-looking strings and comments",
            query_sql='SELECT order_id, status, \'__ref("unused")\' AS note FROM __source("raw_orders") /* __source("missing") */',
            expected_sql_fragment='CAST(NULL AS VARCHAR) AS "status"',
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_partial_fixture_when_compiling_and_planning_then_completes_required_columns(
    test_case: FixturePlanningIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                'name = "fixture_cache_demo"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = ":memory:"\n'
            ),
            "sql/models/orders.sql": (
                f'MODEL (description "Test model orders.");\n\n{test_case.query_sql}\n'
            ),
            "sql/sources/raw_orders.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id, 'open' AS status\n"
                "    columns:\n"
                "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
                "      - name: status\n        type: VARCHAR\n        nullable: true\n"
            ),
            "tests/unit/test_orders.sql": (
                "TEST();\n\n"
                "WITH\n"
                "__source__raw_orders AS (SELECT 1 AS order_id),\n"
                "__expected__orders AS (\n"
                "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
                ")\n"
                "SELECT 1\n"
            ),
        },
    )
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    adapter: DuckDbAdapter = DuckDbAdapter()

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=adapter,
    )
    entries, warnings = build_selected_test_entries(
        sql_test_selection=SqlTestSelection(),
        project=project,
        dialect=Dialect(adapter),
        selected_keys=frozenset((project.models[0].key,)),
    )

    assert warnings == []
    assert len(entries) == 1
    entry: SqlTestPlanEntry = entries[0]
    assert test_case.expected_sql_fragment in entry.chain[0].resolved_sql


@pytest.mark.parametrize(
    "test_case",
    (
        CaseFoldedFixtureFillTestCase(
            description="duckdb fills each omitted column once",
            adapter_name="duckdb",
            project_toml='adapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n',
            expected_single_fragments=('CAST(NULL AS VARCHAR) AS "status"',),
        ),
        CaseFoldedFixtureFillTestCase(
            description="snowflake fills each omitted column once",
            adapter_name="snowflake",
            project_toml='adapter = "snowflake"\n',
            expected_single_fragments=('CAST(NULL AS VARCHAR) AS "STATUS"',),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_upper_case_reads_through_star_import_when_planning_then_fills_each_column_once(
    test_case: CaseFoldedFixtureFillTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": f'name = "fixture_fill_demo"\n{test_case.project_toml}',
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw__orders\n    description: Raw order records\n"
                "    database: example_db\n"
                "    schema: raw\n"
                "    table: orders\n"
                "    columns:\n"
                "      - name: order_id\n        type: VARCHAR\n"
                "      - name: customer_id\n        type: VARCHAR\n"
                "      - name: status\n        type: VARCHAR\n"
            ),
            "sql/models/orders.sql": (
                'MODEL (description "Orders with their status.", database example_db, '
                "schema analytics);\n\n"
                'WITH raw__orders AS (SELECT * FROM __source("raw__orders")),\n'
                "orders AS (SELECT ORDER_ID, STATUS FROM raw__orders)\n"
                "SELECT ORDER_ID AS order_id, STATUS AS status FROM orders\n"
            ),
            "tests/unit/test_orders.sql": (
                "TEST();\n\n"
                "WITH\n"
                "__source__raw__orders AS (SELECT CAST('o1' AS VARCHAR) AS order_id),\n"
                "__expected__orders AS (SELECT CAST('o1' AS VARCHAR) AS order_id)\n"
                "SELECT 1\n"
            ),
        },
    )
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    adapter: BaseAdapter = _ADAPTERS[test_case.adapter_name]()

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=adapter,
    )
    entries, warnings = build_selected_test_entries(
        sql_test_selection=SqlTestSelection(),
        project=project,
        dialect=Dialect(adapter),
        selected_keys=frozenset((project.models[0].key,)),
    )

    assert warnings == []
    assert len(entries) == 1
    resolved_sql: str = entries[0].chain[0].resolved_sql
    for fragment in test_case.expected_single_fragments:
        assert resolved_sql.count(fragment) == 1, resolved_sql


@pytest.mark.parametrize(
    "test_case",
    (
        FixturePlanningIntegrationTestCase(
            description="supplied null-only column receives its authoritative type",
            expected_sql_fragment=(
                'CAST("__sqlbuild_partial_fixture".status AS VARCHAR) AS status'
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_untyped_null_fixture_when_planning_then_applies_authoritative_type(
    test_case: FixturePlanningIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                'name = "fixture_type_demo"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = ":memory:"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.');\n\nSELECT TRY_CAST(status AS DOUBLE) AS parsed_status "
                'FROM __source("raw_orders")\n'
            ),
            "sql/sources/raw_orders.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT '1.5' AS status\n"
                "    contract: enforced\n"
                "    columns:\n"
                "      - name: status\n        type: VARCHAR\n        nullable: true\n"
            ),
            "tests/unit/test_orders.sql": (
                "TEST();\n\n"
                "WITH\n"
                "__source__raw_orders AS (SELECT NULL AS status WHERE FALSE),\n"
                "__expected__orders AS (\n"
                "  SELECT CAST(NULL AS DOUBLE) AS parsed_status WHERE FALSE\n"
                ")\n"
                "SELECT 1\n"
            ),
        },
    )
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    adapter: DuckDbAdapter = DuckDbAdapter()

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=adapter,
    )
    entries, warnings = build_selected_test_entries(
        sql_test_selection=SqlTestSelection(),
        project=project,
        dialect=Dialect(adapter),
        selected_keys=frozenset((project.models[0].key,)),
    )

    assert warnings == []
    assert len(entries) == 1
    entry: SqlTestPlanEntry = entries[0]
    assert test_case.expected_sql_fragment in entry.chain[0].resolved_sql


@pytest.mark.parametrize(
    "test_case",
    (
        FixtureSchemaIntegrationTestCase(
            description="enforced source rejects unknown supplied columns",
            contract="    contract: enforced\n",
            expected_authoritative=True,
            expected_diagnostics=("mock source 'raw_orders' supplies unknown columns: extra",),
        ),
        FixtureSchemaIntegrationTestCase(
            description="open source permits unknown supplied columns",
            contract="",
            expected_authoritative=False,
            expected_diagnostics=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_source_contract_when_completing_fixture_then_respects_authoritative_names(
    test_case: FixtureSchemaIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "fixture_schema_demo"\nadapter = "duckdb"\n[connection]\ndatabase = ":memory:"\n',
            "sql/models/orders.sql": 'MODEL (description "Order facts."); SELECT order_id FROM __source("raw_orders")',
            "sql/sources/raw_orders.yml": (
                "sources:\n  - name: raw_orders\n    description: Raw order facts.\n"
                "    expression: SELECT 1 AS order_id\n"
                f"{test_case.contract}"
                "    columns:\n      - name: order_id\n        type: INTEGER\n"
            ),
        },
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=adapter
    )
    context: RelationFixturePlanningContext = build_relation_fixture_context(project=project)
    key: tuple[CompiledResourceType, str] = (CompiledResourceType.SOURCE, "raw_orders")
    completed: RelationFixtureCompletion = build_relation_fixture_completion(
        project=project,
        dialect=Dialect(adapter),
        request=(
            ("orders",),
            ((CompiledResourceType.SOURCE, {"raw_orders": "SELECT 1 AS ORDER_ID, 2 AS extra"}),),
        ),
        planning_context=context,
    )
    assert context.authoritative_names(key) is test_case.expected_authoritative
    assert context.expected_types(key) == {"order_id": "INTEGER"}
    assert (
        tuple(diagnostic.message for diagnostic in completed.diagnostics)
        == test_case.expected_diagnostics
    )
    assert completed.fixture_sql_by_key[key] == "SELECT 1 AS ORDER_ID, 2 AS extra"


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
