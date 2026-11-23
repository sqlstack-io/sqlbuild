from __future__ import annotations

from pathlib import Path

import pytest

import sqlbuild._native as _native
from sqlbuild._native import CompileInputError
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    ValidateHookSqlSyntaxTestCase,
    ValidateSqlSyntaxTestCase,
)

_MODEL_NAME: str = "test_model"
_FILE_PATH: Path = Path("sql/models/test_model.sql")


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateSqlSyntaxTestCase(
            description="accepts simple select",
            query_sql='SELECT id, name FROM __ref("orders")',
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts CTE chain",
            query_sql=(
                'WITH base AS (  SELECT id, name FROM __ref("orders")) SELECT id, name FROM base'
            ),
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts source references",
            query_sql='SELECT id FROM __source("stripe__payments")',
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts dbt ref references",
            query_sql='SELECT id FROM __dbt_ref("stg_orders")',
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts udf references",
            query_sql='SELECT __udf("is_completed_order")(status) AS is_done FROM __ref("orders")',
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts union queries",
            query_sql=('SELECT id FROM __ref("orders") UNION ALL SELECT id FROM __ref("returns")'),
            expected_valid=True,
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts Snowflake grouping sets",
            query_sql=(
                "SELECT category, SUM(amount) FROM orders GROUP BY GROUPING SETS ((category), ())"
            ),
            expected_valid=True,
            dialect="snowflake",
        ),
        ValidateSqlSyntaxTestCase(
            description="accepts Snowflake dynamic brackets after variant paths",
            query_sql="SELECT attributes:labels[TO_VARCHAR(item_id)] FROM orders",
            expected_valid=True,
            dialect="snowflake",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_valid_sql_when_validating_syntax_then_does_not_raise(
    test_case: ValidateSqlSyntaxTestCase,
) -> None:
    _native.validate_sql_syntax(
        test_case.query_sql,
        (_MODEL_NAME, _FILE_PATH),
        (None, test_case.dialect),
    )

    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateSqlSyntaxTestCase(
            description="rejects unclosed parenthesis",
            query_sql="SELECT id FROM (SELECT 1",
            expected_valid=False,
            expected_error_fragment="SQL syntax error in model 'test_model'",
        ),
        ValidateSqlSyntaxTestCase(
            description="rejects malformed select",
            query_sql="SELEC id FROM orders",
            expected_valid=False,
            expected_error_fragment="SQL syntax error in model 'test_model'",
        ),
        ValidateSqlSyntaxTestCase(
            description="rejects incomplete CTE",
            query_sql="WITH base AS ( SELECT id FROM orders",
            expected_valid=False,
            expected_error_fragment="SQL syntax error in model 'test_model'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_sql_when_validating_syntax_then_raises_compile_error(
    test_case: ValidateSqlSyntaxTestCase,
) -> None:
    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        _native.validate_sql_syntax(
            test_case.query_sql,
            (_MODEL_NAME, _FILE_PATH),
            (None, test_case.dialect),
        )

    assert test_case.expected_valid is False


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateHookSqlSyntaxTestCase(
            description="accepts multiple statements as one execution payload",
            hook_sql="DELETE FROM staging; VACUUM staging;",
            dialect="duckdb",
            expected_valid=True,
        ),
        ValidateHookSqlSyntaxTestCase(
            description="uses the adapter dialect for vendor syntax",
            hook_sql="SELECT [value] FROM [orders]",
            dialect="tsql",
            expected_valid=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_polyglot_supported_hook_sql_when_validating_then_does_not_raise(
    test_case: ValidateHookSqlSyntaxTestCase,
) -> None:
    _native.validate_hook_sql_syntax(
        test_case.hook_sql,
        "pre_hooks",
        (_MODEL_NAME, _FILE_PATH),
        (None, test_case.dialect),
    )

    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateHookSqlSyntaxTestCase(
            description="rejects malformed SQL when validation is enabled",
            hook_sql="THIS IS NOT VALID SQL",
            dialect="duckdb",
            expected_valid=False,
            expected_error_fragment="Polyglot could not parse model 'test_model' pre_hooks",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_hook_sql_when_validating_then_raises_compile_error(
    test_case: ValidateHookSqlSyntaxTestCase,
) -> None:
    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        _native.validate_hook_sql_syntax(
            test_case.hook_sql,
            "pre_hooks",
            (_MODEL_NAME, _FILE_PATH),
            (None, test_case.dialect),
        )

    assert test_case.expected_valid is False
