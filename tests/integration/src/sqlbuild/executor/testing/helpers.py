"""Test helpers for SQL test executor integration tests."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from sqlbuild._native import (
    ChainStep,
    CompiledObjectKey,
    CompiledResourceType,
    CompilePipelineResult,
    SqlTestPlanEntry,
    render_sql_test_comparison_sql,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.testing.main._execute import execute_sql_test
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.executor.testing.types import SqlTestOutcome
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)
from tests.integration.src.sqlbuild.executor.build.helpers import SharedConnectionDuckDbAdapter
from tests.integration.src.sqlbuild.executor.testing._test_types import (
    SqlTestExecutionTestCase,
)


def build_sql_test_plan_entry(
    *,
    name: str,
    chain_steps: tuple[tuple[str, str, str | None], ...],
) -> SqlTestPlanEntry:
    """Build a SqlTestPlanEntry from (model_name, resolved_sql, expected_cte_sql) tuples."""

    chain: tuple[ChainStep, ...] = tuple(
        ChainStep(
            model_name=step[0],
            resolved_sql=step[1],
            expected_cte_sql=step[2],
        )
        for step in chain_steps
    )
    scope_deps: tuple[CompiledObjectKey, ...] = tuple(
        CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=step[0])
        for step in chain_steps
    )
    return SqlTestPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SQL_TEST, name=name),
        name=name,
        chain=chain,
        scope_deps=scope_deps,
    )


def run_sql_test(
    *,
    test_case: SqlTestExecutionTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> SqlTestExecutionResult:
    """Execute a SQL test case and return the result."""

    entry: SqlTestPlanEntry = build_sql_test_plan_entry(
        name="test_model",
        chain_steps=test_case.chain_steps,
    )
    return execute_sql_test(
        test_entry=entry,
        adapter=adapter,
        connection=connection,
    )


def verify_test_result(
    *,
    result: SqlTestExecutionResult,
    test_case: SqlTestExecutionTestCase,
) -> None:
    """Verify SQL test execution result fields."""

    assert len(result.step_results) == test_case.expected_step_count
    assert (test_case.expected_error_fragment or "") in (result.error_message or "")
    models_by_outcome: dict[SqlTestOutcome, list[str]] = {outcome: [] for outcome in SqlTestOutcome}
    for step_result in result.step_results:
        models_by_outcome[step_result.outcome].append(step_result.model_name)
    failed_models: tuple[str, ...] = tuple(models_by_outcome[SqlTestOutcome.FAIL])
    assert failed_models == test_case.expected_failed_models


_MARKER_CALL: re.Pattern[str] = re.compile(
    r'__(?:ref|source|seed|dbt_ref|udf|table_fn)\(\s*"[^"]*"(?:\s*,\s*"[^"]*")*\s*\)'
)
_FIXTURE_RELATION: str = r'(?:[\w."`]+|\((?s:.*?)\))'


def build_sql_matches_test_body(*, build_sql: str, test_body: str) -> bool:
    """Whether a test body is the build SQL byte for byte, apart from relation markers.

    A marker becomes a fixture or chain CTE name, or an inlined parenthesised fixture query.
    """

    parts: list[str] = _MARKER_CALL.split(build_sql)
    return (
        re.fullmatch(_FIXTURE_RELATION.join(re.escape(part) for part in parts), test_body)
        is not None
    )


def render_project_test_step(
    *, project_dir: Path, test_name: str, model_name: str
) -> tuple[str, str, str]:
    """Compile a DuckDB project and return one step's build SQL, test body and rendered test SQL."""

    adapter: DuckDbAdapter = DuckDbAdapter()
    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=project_dir, adapter=adapter
    )
    return _project_test_step(
        result=result, adapter=adapter, test_name=test_name, model_name=model_name
    )


def render_project_test_step_over_tables(
    *, project_dir: Path, test_name: str, model_name: str, setup_sql: tuple[str, ...]
) -> tuple[str, str, str]:
    """Plan against an in-memory warehouse holding the source tables, then render one step."""

    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        statement: str
        for statement in setup_sql:
            connection.execute(statement)
        result: CompilePipelineResult = run_compile_pipeline_for_project(
            project_dir=project_dir, adapter=SharedConnectionDuckDbAdapter(connection=connection)
        )
    finally:
        adapter.close(connection)
    return _project_test_step(
        result=result, adapter=adapter, test_name=test_name, model_name=model_name
    )


def _project_test_step(
    *, result: CompilePipelineResult, adapter: DuckDbAdapter, test_name: str, model_name: str
) -> tuple[str, str, str]:
    entry: SqlTestPlanEntry = {item.name: item for item in result.plan_output.test_entries}[
        test_name
    ]
    step: ChainStep = {item.model_name: item for item in entry.chain}[model_name]
    build_sql: str = {model.name: model.query_sql for model in result.project.models}[model_name]
    rendered_sql: str = render_sql_test_comparison_sql(
        (entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )[0]
    return build_sql, step.comparison_body_sql or step.resolved_sql, rendered_sql


def build_authored_cte_project_files() -> dict[str, str]:
    """Build a DuckDB project whose models use comments, dollar quotes and nested WITH in CTEs."""

    return {
        "sqlbuild_project.toml": (
            'name = "authored_ctes"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "authored_ctes.duckdb"\n\n'
            "[settings]\n"
            "sql_analysis = true\n"
        ),
        "sql/sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "sql/models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
            "WITH base AS (\n"
            "  -- a comment with an unmatched ) parenthesis\n"
            "  SELECT id AS order_id, amount, 'it''s (fine)' AS note /* ( */\n"
            '  FROM __source("raw_orders")\n'
            "),\n"
            "tagged AS (SELECT order_id, amount, note, $$a ) b$$ AS tag FROM base)\n"
            "SELECT order_id, amount, note, tag FROM tagged\n"
        ),
        "sql/models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized table);\n\n"
            "WITH totals AS (\n"
            '  WITH ranked AS (SELECT order_id, amount, tag FROM __ref("stg_orders"))\n'
            "  SELECT order_id, amount * 2 AS doubled, tag FROM ranked\n"
            "),\n"
            "helper_rows AS (SELECT order_id, doubled, tag FROM totals)\n"
            "SELECT order_id, doubled, tag FROM helper_rows\n"
        ),
        "tests/unit/test_stg_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS id, 10 AS amount),\n"
            "__expected__stg_orders AS (\n"
            "  SELECT 1 AS order_id, 10 AS amount, 'it''s (fine)' AS note, 'a ) b' AS tag\n"
            ")\n"
            "SELECT 1\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 10 AS amount, 'x' AS tag),\n"
            "helper_rows AS (SELECT 1 AS order_id, 20 AS doubled, 'x' AS tag),\n"
            "__expected__orders AS (SELECT order_id, doubled, tag FROM helper_rows)\n"
            "SELECT 1\n"
        ),
    }


def build_shared_cte_name_chain_entry() -> SqlTestPlanEntry:
    """Build a two-model chain whose upstream and tested models both define CTE `final`."""

    upstream: str = "WITH final AS (SELECT id FROM __source__raw) SELECT * FROM final"
    tested: str = "WITH final AS (SELECT id + 1 AS id FROM __ref__stg) SELECT final.id FROM final"
    source_mock: tuple[str, str] = ("__source__raw", "SELECT 1 AS id")
    return SqlTestPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SQL_TEST, name="mart_chain"),
        name="mart_chain",
        chain=(
            ChainStep(
                model_name="stg",
                resolved_sql=upstream,
                lifted_ctes=(source_mock,),
                comparison_body_sql=upstream,
                expected_cte_sql="SELECT 1 AS id",
            ),
            ChainStep(
                model_name="mart",
                resolved_sql=tested,
                lifted_ctes=(source_mock, ("__ref__stg", upstream)),
                comparison_body_sql=tested,
                expected_cte_sql="SELECT 2 AS id",
            ),
        ),
    )


def comparison_rows(*, adapter: DuckDbAdapter, connection: Any, sql: str) -> list[tuple[Any, ...]]:
    """Execute rendered comparison SQL and return its count rows."""

    return list(adapter.execute(connection=connection, sql=sql).fetchall())


def build_terminated_model_project_files() -> dict[str, str]:
    """Build a DuckDB project whose WITH model ends in a terminator and is read downstream."""

    return {
        "sqlbuild_project.toml": (
            'name = "terminated"\nadapter = "duckdb"\n\n[connection]\n'
            'database = "terminated.duckdb"\n\n[settings]\nsql_analysis = true\n'
        ),
        "sql/sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "sql/models/m.sql": (
            "MODEL (description 'Test model m.', materialized table);\n\n"
            'WITH a AS (SELECT id FROM __source("raw_orders"))\n'
            "SELECT id FROM a;\n"
        ),
        "sql/models/m_next.sql": (
            "MODEL (description 'Test model m_next.', materialized table);\n\n"
            'SELECT id + 1 AS id FROM __ref("m")\n'
        ),
        "tests/unit/test_m.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS id),\n"
            "__expected__m AS (SELECT 1 AS id),\n"
            "__expected__m_next AS (SELECT 2 AS id),\n"
            '__assert__m_has_one_row AS (SELECT id FROM __ref("m") WHERE id <> 1)\n'
            "SELECT 1\n"
        ),
    }
