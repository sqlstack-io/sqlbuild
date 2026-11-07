from __future__ import annotations

import pytest

from sqlbuild._native import FunctionArgument, FunctionReturnColumn
from sqlbuild.adapter.contract.models import FunctionDefinition, TemporaryFunction
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.compiler.compile.types import FunctionLanguage
from tests.unit.src.sqlbuild.adapter.contract.classes.strict_adapter._test_types import (
    TemporaryFunctionRenderingTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionRenderingTestCase(
            description="duckdb copies a sql function into a temporary macro",
            adapter=DuckDbAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="VARCHAR",
            return_columns=(),
            expected_temporary_function=TemporaryFunction(
                target='temp.main."is_completed_order"',
                statements=(
                    'CREATE OR REPLACE TEMP MACRO "is_completed_order"(order_status) AS (\n'
                    "order_status = 'completed'\n)",
                ),
            ),
        ),
        TemporaryFunctionRenderingTestCase(
            description="duckdb copies a table function into a temporary table macro",
            adapter=DuckDbAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="VARCHAR",
            return_columns=(("order_id", "INTEGER"),),
            expected_temporary_function=TemporaryFunction(
                target='temp.main."is_completed_order"',
                statements=(
                    'CREATE OR REPLACE TEMP MACRO "is_completed_order"(order_status) AS TABLE\n'
                    "order_status = 'completed'",
                ),
            ),
        ),
        TemporaryFunctionRenderingTestCase(
            description="duckdb leaves python functions to per-connection registration",
            adapter=DuckDbAdapter(),
            language=FunctionLanguage.PYTHON,
            argument_type="VARCHAR",
            return_columns=(),
            expected_temporary_function=None,
        ),
        TemporaryFunctionRenderingTestCase(
            description="postgres creates the copy in the session pg_temp schema",
            adapter=PostgresAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="TEXT",
            return_columns=(),
            expected_temporary_function=TemporaryFunction(
                target='pg_temp."is_completed_order"',
                statements=(
                    'CREATE OR REPLACE FUNCTION pg_temp."is_completed_order"(order_status TEXT)\n'
                    "RETURNS BOOLEAN\n"
                    "LANGUAGE SQL AS $$\n"
                    "order_status = 'completed'\n"
                    "$$",
                ),
            ),
        ),
        TemporaryFunctionRenderingTestCase(
            description="snowflake creates a prefixed temporary function in the current schema",
            adapter=SnowflakeAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="TEXT",
            return_columns=(),
            expected_temporary_function=TemporaryFunction(
                target='"__SQB_TEST__IS_COMPLETED_ORDER"',
                statements=(
                    "CREATE OR REPLACE TEMPORARY FUNCTION "
                    '"__SQB_TEST__IS_COMPLETED_ORDER"(order_status TEXT)\n'
                    "RETURNS BOOLEAN\n"
                    "LANGUAGE SQL\n"
                    "AS $$\n"
                    "order_status = 'completed'\n"
                    "$$",
                ),
            ),
        ),
        TemporaryFunctionRenderingTestCase(
            description="bigquery has no session to hold a temporary function",
            adapter=BigQueryAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="STRING",
            return_columns=(),
            expected_temporary_function=None,
        ),
        TemporaryFunctionRenderingTestCase(
            description="databricks creates a prefixed session temporary function",
            adapter=DatabricksAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="STRING",
            return_columns=(),
            expected_temporary_function=TemporaryFunction(
                target="`__sqb_test__is_completed_order`",
                statements=(
                    "CREATE OR REPLACE TEMPORARY FUNCTION "
                    "`__sqb_test__is_completed_order`(order_status STRING)\n"
                    "RETURNS BOOLEAN\n"
                    "RETURN order_status = 'completed'",
                ),
            ),
        ),
        TemporaryFunctionRenderingTestCase(
            description="databricks keeps using built python functions",
            adapter=DatabricksAdapter(),
            language=FunctionLanguage.PYTHON,
            argument_type="STRING",
            return_columns=(),
            expected_temporary_function=None,
        ),
        TemporaryFunctionRenderingTestCase(
            description="sql server keeps using the built function",
            adapter=SqlServerAdapter(),
            language=FunctionLanguage.SQL,
            argument_type="TEXT",
            return_columns=(),
            expected_temporary_function=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_function_when_rendering_temporary_copy_then_adapter_returns_session_ddl(
    test_case: TemporaryFunctionRenderingTestCase,
) -> None:
    temporary: TemporaryFunction | None = test_case.adapter.render_create_temporary_function(
        name="is_completed_order",
        definition=FunctionDefinition(
            destination="analytics.is_completed_order",
            arguments=(FunctionArgument(name="order_status", type=test_case.argument_type),),
            returns="BOOLEAN",
            body_sql="order_status = 'completed'",
            return_columns=tuple(
                FunctionReturnColumn(name=name, type=type_name)
                for name, type_name in test_case.return_columns
            ),
            language=test_case.language,
        ),
    )

    assert temporary == test_case.expected_temporary_function, test_case.description
