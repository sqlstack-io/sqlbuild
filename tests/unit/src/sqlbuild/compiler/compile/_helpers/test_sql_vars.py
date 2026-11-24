"""Tests for SQL project variable interpolation."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CompileInputError, substitute_sql_vars_with_spans
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    SubstituteSqlVarsErrorTestCase,
    SubstituteSqlVarsTestCase,
)

_FILE_PATH: Path = Path("sql/models/test_model.sql")


@pytest.mark.parametrize(
    "test_case",
    [
        SubstituteSqlVarsTestCase(
            description="skips dollar-quoted text and positional parameters",
            sql="SELECT $tag$ it's $$ -- @@status $tag$, price$1$, $1, @@revision",
            effective_vars={"revision": 7, "status": "ready"},
            expected_sql="SELECT $tag$ it's $$ -- ready $tag$, price$1$, $1, 7",
        ),
        SubstituteSqlVarsTestCase(
            description="keeps deferred placeholders and commented tokens",
            sql="-- @@revision\nSELECT @@revision, '@@status', @@@window_start",
            effective_vars={"revision": 7, "status": "ready"},
            expected_sql="-- @@revision\nSELECT 7, 'ready', @@@window_start",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces project var in SQL expression",
            sql="SELECT * FROM orders WHERE region = @@target_region",
            effective_vars={"target_region": "us-east-1"},
            expected_sql="SELECT * FROM orders WHERE region = us-east-1",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces multiple vars",
            sql="SELECT * FROM @@schema.@@table",
            effective_vars={"schema": "staging", "table": "orders"},
            expected_sql="SELECT * FROM staging.orders",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces env var in quoted SQL string",
            sql="SELECT '@@ENV:USER_NAME' AS loaded_by",
            effective_vars={},
            environment_variables={"USER_NAME": "runner"},
            expected_sql="SELECT 'runner' AS loaded_by",
        ),
        SubstituteSqlVarsTestCase(
            description="does not replace macro call with parens",
            sql="SELECT @py.my_macro('arg') FROM orders",
            effective_vars={"my_macro": "should_not_appear"},
            expected_sql="SELECT @py.my_macro('arg') FROM orders",
        ),
        SubstituteSqlVarsTestCase(
            description="does not replace single at placeholder syntax",
            sql="SELECT @name FROM orders",
            effective_vars={"name": "value"},
            expected_sql="SELECT @name FROM orders",
        ),
        SubstituteSqlVarsTestCase(
            description="returns sql unchanged when no interpolation tokens present",
            sql="SELECT 1 FROM orders",
            effective_vars={"name": "value"},
            expected_sql="SELECT 1 FROM orders",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces var inside single-quoted string",
            sql="SELECT '@@name' FROM orders",
            effective_vars={"name": "value"},
            expected_sql="SELECT 'value' FROM orders",
        ),
        SubstituteSqlVarsTestCase(
            description="skips interpolation inside line comment",
            sql="-- @@name\nSELECT 1",
            effective_vars={"name": "value"},
            expected_sql="-- @@name\nSELECT 1",
        ),
        SubstituteSqlVarsTestCase(
            description="skips interpolation inside block comment",
            sql="/* @@name */ SELECT 1",
            effective_vars={"name": "value"},
            expected_sql="/* @@name */ SELECT 1",
        ),
        SubstituteSqlVarsTestCase(
            description="preserves deferred engine placeholder",
            sql="SELECT * FROM orders WHERE ds >= @@@partition_start",
            effective_vars={"partition_start": "should_not_appear"},
            expected_sql="SELECT * FROM orders WHERE ds >= @@@partition_start",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces context value when allowed",
            sql="GRANT SELECT ON @@CTX:destination.qualified TO role analytics",
            effective_vars={},
            context_values={"destination.qualified": "analytics.marts.orders"},
            expected_sql="GRANT SELECT ON analytics.marts.orders TO role analytics",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces context value inside quoted SQL string when allowed",
            sql="SELECT '@@CTX:model.name' AS model_name",
            effective_vars={},
            context_values={"model.name": "orders"},
            expected_sql="SELECT 'orders' AS model_name",
        ),
        SubstituteSqlVarsTestCase(
            description="replaces context components inside a qualified identifier",
            sql=(
                "CREATE FUNCTION @@CTX:destination.database."
                "@@CTX:destination.schema.reconstruct_book()"
            ),
            effective_vars={},
            context_values={
                "destination.database": "analytics",
                "destination.schema": "commerce",
            },
            expected_sql="CREATE FUNCTION analytics.commerce.reconstruct_book()",
        ),
        SubstituteSqlVarsTestCase(
            description="renders scalar json var values as SQL text",
            sql="SELECT @@limit AS limit, @@enabled AS enabled, '@@optional' AS optional",
            effective_vars={"limit": 10, "enabled": True, "optional": None},
            expected_sql="SELECT 10 AS limit, true AS enabled, '' AS optional",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_and_vars_when_substituting_then_returns_expected(
    test_case: SubstituteSqlVarsTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    target_name: str
    environment_value: str
    for target_name, environment_value in test_case.environment_variables.items():
        monkeypatch.setenv(target_name, environment_value)

    result: str
    result, _spans = substitute_sql_vars_with_spans(
        sql=test_case.sql,
        file_path=_FILE_PATH,
        effective_vars=test_case.effective_vars,
        context_values=test_case.context_values,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SubstituteSqlVarsErrorTestCase(
            description="raises on unknown var",
            sql="SELECT @@unknown_var FROM orders",
            effective_vars={"other": "value"},
            expected_error_fragment="unknown project variable '@@unknown_var'",
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises on CTX token",
            sql="SELECT @@CTX:model.name FROM orders",
            effective_vars={},
            expected_error_fragment="does not allow @@CTX templates",
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises on unknown env var",
            sql="SELECT '@@ENV:SQLBUILD_MISSING_ENV'",
            effective_vars={},
            expected_error_fragment="unknown environment variable '@@ENV:SQLBUILD_MISSING_ENV'",
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises on unknown allowed CTX token",
            sql="SELECT @@CTX:destination.qualified",
            effective_vars={},
            context_values={"model.name": "orders"},
            expected_error_fragment="references unknown CTX key 'destination.qualified'",
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises on unavailable allowed CTX token",
            sql="SELECT @@CTX:destination.database",
            effective_vars={},
            context_values={"destination.database": None},
            expected_error_fragment=(
                "references CTX key 'destination.database' but no value is available"
            ),
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises with preview on structured object var interpolation",
            sql="SELECT @@grants",
            effective_vars={"grants": {"role": "analyst"}},
            expected_error_fragment=(
                r"SQL variable '@@grants' is an object and cannot be interpolated as text: "
                r'\{"role":"analyst"\}. Use a macro to consume structured vars\.'
            ),
        ),
        SubstituteSqlVarsErrorTestCase(
            description="raises with preview on structured array var interpolation",
            sql="SELECT @@roles",
            effective_vars={"roles": ["analyst", "reporter"]},
            expected_error_fragment=(
                r"SQL variable '@@roles' is an array and cannot be interpolated as text: "
                r'\["analyst","reporter"\]. Use a macro to consume structured vars\.'
            ),
        ),
        SubstituteSqlVarsErrorTestCase(
            description="deferred placeholder preserves unclosed quote diagnostics",
            sql="SELECT @@@partition_start, 'unterminated",
            effective_vars={},
            expected_error_fragment="SQL interpolation contains an unclosed quoted string",
        ),
        SubstituteSqlVarsErrorTestCase(
            description="deferred placeholder preserves unclosed comment diagnostics",
            sql="SELECT @@@partition_start /* unterminated",
            effective_vars={},
            expected_error_fragment="SQL interpolation contains an unclosed block comment",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_var_when_substituting_then_raises(
    test_case: SubstituteSqlVarsErrorTestCase,
) -> None:
    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        substitute_sql_vars_with_spans(
            sql=test_case.sql,
            file_path=_FILE_PATH,
            effective_vars=test_case.effective_vars,
            context_values=test_case.context_values,
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
