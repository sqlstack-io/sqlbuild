"""Tests for rewriting test SQL onto temporary function copies."""

from __future__ import annotations

import pytest

from sqlbuild._native import FunctionPlanEntry
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.pipeline._helpers.temporary_functions import (
    create_temporary_test_functions,
    rewrite_function_calls,
)
from sqlbuild.executor.pipeline.models import TemporaryTestFunctions
from tests.unit.src.sqlbuild.executor.pipeline._helpers._test_types import (
    FunctionCallRewriteTestCase,
    TemporaryCopyPlacementTestCase,
)
from tests.unit.src.sqlbuild.executor.pipeline._helpers.helpers import (
    SessionSchemaSnowflakeAdapter,
    order_status_function_test_plan,
)

_REPLACEMENTS: dict[str, str] = {
    "analytics.is_completed_order": 'temp.main."is_completed_order"',
    '"warehouse"."analytics"."customer_orders"': 'temp.main."customer_orders"',
}


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionCallRewriteTestCase(
            description="scalar call moves to the temporary copy",
            sql="SELECT analytics.is_completed_order(status) FROM orders",
            expected_sql='SELECT temp.main."is_completed_order"(status) FROM orders',
        ),
        FunctionCallRewriteTestCase(
            description="whitespace before the argument list still matches",
            sql="SELECT analytics.is_completed_order (status)",
            expected_sql='SELECT temp.main."is_completed_order" (status)',
        ),
        FunctionCallRewriteTestCase(
            description="quoted table function target moves to the temporary copy",
            sql='SELECT * FROM "warehouse"."analytics"."customer_orders"(7)',
            expected_sql='SELECT * FROM temp.main."customer_orders"(7)',
        ),
        FunctionCallRewriteTestCase(
            description="longer names and non-call mentions stay unchanged",
            sql=(
                "SELECT staging.analytics.is_completed_order(status), "
                "analytics.is_completed_order_v2(status), "
                "'analytics.is_completed_order' AS label"
            ),
            expected_sql=(
                "SELECT staging.analytics.is_completed_order(status), "
                "analytics.is_completed_order_v2(status), "
                "'analytics.is_completed_order' AS label"
            ),
        ),
        FunctionCallRewriteTestCase(
            description="calls inside string literals and comments stay unchanged",
            sql=(
                "SELECT analytics.is_completed_order(status), "
                "'analytics.is_completed_order(x)' AS label -- analytics.is_completed_order(y)\n"
                "/* analytics.is_completed_order(z) */ FROM orders"
            ),
            expected_sql=(
                'SELECT temp.main."is_completed_order"(status), '
                "'analytics.is_completed_order(x)' AS label -- analytics.is_completed_order(y)\n"
                "/* analytics.is_completed_order(z) */ FROM orders"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deployed_function_calls_when_rewriting_then_only_calls_move_to_temporary_copies(
    test_case: FunctionCallRewriteTestCase,
) -> None:
    rewritten: str = rewrite_function_calls(
        sql=test_case.sql,
        replacements=_REPLACEMENTS,
        syntax=DuckDbAdapter.sql_lexical_syntax.native_mapping,
    )

    assert rewritten == test_case.expected_sql, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryCopyPlacementTestCase(
            description="a session with a current schema holds the copy",
            session_schema=('"DEV_DB"', '"SANDBOX"'),
            expected_target='"DEV_DB"."SANDBOX"."__SQB_TEST__IS_COMPLETED_ORDER"',
        ),
        TemporaryCopyPlacementTestCase(
            description="a session without one puts the copy beside the function",
            session_schema=None,
            expected_target='analytics."__SQB_TEST__IS_COMPLETED_ORDER"',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_session_when_copying_functions_then_copy_goes_to_its_schema_first(
    test_case: TemporaryCopyPlacementTestCase,
) -> None:
    entries: tuple[FunctionPlanEntry, ...] = order_status_function_test_plan(
        body_sql="order_status = 'completed'", test_count=1
    ).function_entries
    adapter: SessionSchemaSnowflakeAdapter = SessionSchemaSnowflakeAdapter(test_case.session_schema)

    copies: TemporaryTestFunctions = create_temporary_test_functions(
        function_entries=entries,
        required_keys=tuple(entry.key for entry in entries),
        adapter=adapter,
        connection=None,
    )

    assert copies.replacements == {"analytics.is_completed_order": test_case.expected_target}, (
        test_case.description
    )
    assert adapter.executed[0].startswith(
        f"CREATE OR REPLACE TEMPORARY FUNCTION {test_case.expected_target}("
    ), test_case.description
