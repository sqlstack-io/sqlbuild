"""Unit tests for sqlbuild interpolation neutralization and restoration."""

from __future__ import annotations

import pytest

import sqlbuild._native as _native
from sqlbuild._native import InterpolationSite
from tests.unit.src.sqlbuild.lint._helpers._test_types import (
    DialectNeutralizeInterpolationTestCase,
    DollarQuotePreparationTestCase,
    NeutralizeInterpolationTestCase,
    RestoreFailureTestCase,
    RestoreInterpolationTestCase,
    UnicodePreparationTestCase,
)
from tests.unit.src.sqlbuild.lint._helpers.helpers import (
    neutralize_interpolation,
    restore_interpolation,
)


@pytest.mark.parametrize(
    "test_case",
    [
        UnicodePreparationTestCase(label, label, ('__ref("orders")',))
        for label in ("ASCII", "é", "€", "😀", "é € 😀")
    ],
    ids=lambda case: case.description,
)
def test_given_unicode_before_intrinsics_when_preparing_then_lint_preserves_semantic_calls(
    test_case: UnicodePreparationTestCase,
) -> None:
    sql: str = (
        f"SELECT '{test_case.prefix}' AS label, __cursor_start(), __cursor_end() "
        'FROM __EMPTY_FIXTURE(); SELECT * FROM __ref("orders")'
    )
    lint_sql, lint_sites = neutralize_interpolation(body=sql, dialect="duckdb")
    format_sql, format_sites = neutralize_interpolation(
        body=sql, dialect="duckdb", for_formatting=True
    )
    assert tuple(site.original_text for site in lint_sites) == test_case.expected_lint_calls
    assert "__EMPTY_FIXTURE()" in lint_sql
    assert "__cursor_start()" in lint_sql
    assert "__cursor_end()" in lint_sql
    assert lint_sites[0].original_start == sql.index('__ref("orders")')
    assert (
        lint_sql[lint_sites[0].neutralized_start : lint_sites[0].neutralized_end]
        == lint_sites[0].sentinel
    )
    assert len(format_sites) == 4
    assert restore_interpolation(fixed=format_sql, sites=format_sites) == (True, sql)
    assert all(
        sql[site.original_start : site.original_end] == site.original_text for site in format_sites
    )
    ascii_prefix: str = "a" * len(test_case.prefix)
    ascii_sql: str = sql.replace(f"'{test_case.prefix}'", f"'{ascii_prefix}'", 1)
    native: tuple[str, list[tuple[str, int, int, int, int, str]], list[str]] | None = (
        _native.prepare_lint_sql(
            {
                "expanded": ascii_sql,
                "before_expansion": ascii_sql,
                "prior_sites": [],
                "dialect": "duckdb",
            }
        )
    )
    assert native is not None
    assert native[0].replace(f"'{ascii_prefix}'", f"'{test_case.prefix}'", 1) == lint_sql
    assert tuple(InterpolationSite(*site) for site in native[1]) == lint_sites


@pytest.mark.parametrize(
    "test_case",
    [
        NeutralizeInterpolationTestCase(
            description="bare audit parameter becomes a sentinel",
            body="SELECT * FROM t WHERE NOT (@expression)",
            expected_neutralized="SELECT * FROM t WHERE NOT (__sqlclean_interpolation_0__)",
            expected_original_texts=("@expression",),
        ),
        NeutralizeInterpolationTestCase(
            description="macro call including arguments becomes one sentinel",
            body="SELECT @py.cents_to_dollars(amount_cents) AS amount",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS amount",
            expected_original_texts=("@py.cents_to_dollars(amount_cents)",),
        ),
        NeutralizeInterpolationTestCase(
            description="macro call with python valued arguments becomes one sentinel",
            body="SELECT @py.pick(columns=['a', 'b']) AS chosen",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS chosen",
            expected_original_texts=("@py.pick(columns=['a', 'b'])",),
        ),
        NeutralizeInterpolationTestCase(
            description="nested macro parentheses are matched",
            body="SELECT @py.outer(@py.inner(x), y) AS v",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS v",
            expected_original_texts=("@py.outer(@py.inner(x), y)",),
        ),
        NeutralizeInterpolationTestCase(
            description="quoted audit parameter becomes a sentinel",
            body="SELECT @'column_name' AS c",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS c",
            expected_original_texts=("@'column_name'",),
        ),
        NeutralizeInterpolationTestCase(
            description="project variable interpolation becomes a sentinel",
            body="SELECT * FROM @@source_table",
            expected_neutralized="SELECT * FROM __sqlclean_interpolation_0__",
            expected_original_texts=("@@source_table",),
        ),
        NeutralizeInterpolationTestCase(
            description="environment interpolation becomes a sentinel",
            body="SELECT * FROM @@ENV:TARGET_TABLE",
            expected_neutralized="SELECT * FROM __sqlclean_interpolation_0__",
            expected_original_texts=("@@ENV:TARGET_TABLE",),
        ),
        NeutralizeInterpolationTestCase(
            description="context interpolation becomes a sentinel",
            body="SELECT @@CTX:run_id AS run",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS run",
            expected_original_texts=("@@CTX:run_id",),
        ),
        NeutralizeInterpolationTestCase(
            description="template expression becomes a sentinel",
            body="SELECT ${if(eq(a, b), x, y)} AS v",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS v",
            expected_original_texts=("${if(eq(a, b), x, y)}",),
        ),
        NeutralizeInterpolationTestCase(
            description="each site receives a distinct sentinel",
            body="SELECT @a, @a FROM t",
            expected_neutralized="SELECT __sqlclean_interpolation_0__, __sqlclean_interpolation_1__ FROM t",
            expected_original_texts=("@a", "@a"),
        ),
        NeutralizeInterpolationTestCase(
            description="interpolation inside a string literal is left alone",
            body="SELECT '@not_a_macro' AS c",
            expected_neutralized="SELECT '@not_a_macro' AS c",
            expected_original_texts=(),
        ),
        NeutralizeInterpolationTestCase(
            description="SQLBuild reference call becomes one sentinel",
            body='SELECT * FROM __ref("@model")',
            expected_neutralized="SELECT * FROM __sqlclean_interpolation_0__",
            expected_original_texts=('__ref("@model")',),
        ),
        NeutralizeInterpolationTestCase(
            description="SQLBuild table function keeps visible invocation arguments",
            body=(
                'SELECT * FROM __table_fn("events")((SELECT ARRAY_AGG(event_id) FROM event_ids))'
            ),
            expected_neutralized=(
                "SELECT * FROM __sqlclean_interpolation_0__((SELECT ARRAY_AGG(event_id) FROM event_ids))"
            ),
            expected_original_texts=('__table_fn("events")',),
        ),
        NeutralizeInterpolationTestCase(
            description="escaped SQL quote does not hide a later table function",
            body="SELECT 'can''t'; SELECT * FROM __table_fn(\"events\")((SELECT 1))",
            expected_neutralized="SELECT 'can''t'; SELECT * FROM __sqlclean_interpolation_0__((SELECT 1))",
            expected_original_texts=('__table_fn("events")',),
        ),
        NeutralizeInterpolationTestCase(
            description="apostrophe in comment does not hide a later table function",
            body=(
                '/* Snowflake\'s table function */\nSELECT * FROM __table_fn("events")((SELECT 1))'
            ),
            expected_neutralized=(
                "/* Snowflake's table function */\nSELECT * FROM __sqlclean_interpolation_0__((SELECT 1))"
            ),
            expected_original_texts=('__table_fn("events")',),
        ),
        NeutralizeInterpolationTestCase(
            description="SQLBuild fixture CTE name remains ordinary SQL",
            body="SELECT * FROM __expected__model",
            expected_neutralized="SELECT * FROM __expected__model",
            expected_original_texts=(),
        ),
        NeutralizeInterpolationTestCase(
            description="ordinary double-underscore function remains generic SQL",
            body="SELECT __custom(value) FROM items",
            expected_neutralized="SELECT __custom(value) FROM items",
            expected_original_texts=(),
        ),
        NeutralizeInterpolationTestCase(
            description="lint retains semantic intrinsics in either case",
            body="SELECT __cursor_start(), __cursor_end() FROM __empty_fixture(); SELECT * FROM __EMPTY_FIXTURE()",
            expected_neutralized="SELECT __cursor_start(), __cursor_end() FROM __empty_fixture(); SELECT * FROM __EMPTY_FIXTURE()",
            expected_original_texts=(),
        ),
        NeutralizeInterpolationTestCase(
            description="lint matches native reference spelling policy",
            body='SELECT * FROM __REF("orders"); SELECT * FROM __ref ("orders")',
            expected_neutralized='SELECT * FROM __REF("orders"); SELECT * FROM __ref ("orders")',
            expected_original_texts=(),
        ),
        NeutralizeInterpolationTestCase(
            description="body without interpolation is unchanged",
            body="SELECT 1 AS x FROM t",
            expected_neutralized="SELECT 1 AS x FROM t",
            expected_original_texts=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_body_when_neutralizing_then_sentinels_replace_interpolation(
    test_case: NeutralizeInterpolationTestCase,
) -> None:
    neutralized: str
    sites: tuple[InterpolationSite, ...]
    neutralized, sites = neutralize_interpolation(body=test_case.body, dialect="generic")
    assert neutralized == test_case.expected_neutralized
    assert tuple(site.original_text for site in sites) == test_case.expected_original_texts
    native: tuple[str, list[tuple[str, int, int, int, int, str]], list[str]] | None = (
        _native.prepare_lint_sql(
            {
                "expanded": test_case.body,
                "before_expansion": test_case.body,
                "prior_sites": [],
                "dialect": "generic",
            }
        )
    )
    assert native is not None
    assert native[0] == test_case.expected_neutralized
    assert tuple(InterpolationSite(*site) for site in native[1]) == sites


@pytest.mark.parametrize(
    "test_case",
    [
        DialectNeutralizeInterpolationTestCase(
            description="databricks backtick keeps a close paren inside a macro call",
            dialect="databricks",
            body="SELECT @py.m(`a)b` x) AS y",
            expected_neutralized="SELECT __sqlclean_interpolation_0__ AS y",
            expected_original_texts=("@py.m(`a)b` x)",),
        ),
        DialectNeutralizeInterpolationTestCase(
            description="bigquery backtick apostrophe does not hide a later macro",
            dialect="bigquery",
            body='SELECT `customer\'s id` AS customer_id, @py.label("order_id") AS order_id',
            expected_neutralized="SELECT `customer's id` AS customer_id, __sqlclean_interpolation_0__ AS order_id",
            expected_original_texts=('@py.label("order_id")',),
        ),
        DialectNeutralizeInterpolationTestCase(
            description="postgres backtick remains ordinary code",
            dialect="postgres",
            body="SELECT @py.m(`a)b` x) AS y",
            expected_neutralized="SELECT __sqlclean_interpolation_0__b` x) AS y",
            expected_original_texts=("@py.m(`a)",),
        ),
        DialectNeutralizeInterpolationTestCase(
            description="sqlserver backtick remains ordinary code",
            dialect="tsql",
            body="SELECT @py.m(`a)b` x) AS y",
            expected_neutralized="SELECT __sqlclean_interpolation_0__b` x) AS y",
            expected_original_texts=("@py.m(`a)",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_quoting_when_neutralizing_then_python_and_native_sites_agree(
    test_case: DialectNeutralizeInterpolationTestCase,
) -> None:
    neutralized: str
    sites: tuple[InterpolationSite, ...]
    neutralized, sites = neutralize_interpolation(body=test_case.body, dialect=test_case.dialect)
    assert neutralized == test_case.expected_neutralized
    assert tuple(site.original_text for site in sites) == test_case.expected_original_texts
    native: tuple[str, list[tuple[str, int, int, int, int, str]], list[str]] | None = (
        _native.prepare_lint_sql(
            {
                "expanded": test_case.body,
                "before_expansion": test_case.body,
                "prior_sites": [],
                "dialect": test_case.dialect,
            }
        )
    )
    assert native is not None
    assert native[0] == test_case.expected_neutralized
    assert tuple(InterpolationSite(*site) for site in native[1]) == sites


@pytest.mark.parametrize(
    "test_case",
    [
        NeutralizeInterpolationTestCase(
            description="sentinels are unique across sites",
            body="SELECT @a, @b, @@c, ${d} FROM t",
            expected_neutralized=(
                "SELECT __sqlclean_interpolation_0__, __sqlclean_interpolation_1__, __sqlclean_interpolation_2__, __sqlclean_interpolation_3__ FROM t"
            ),
            expected_original_texts=("@a", "@b", "@@c", "${d}"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_many_sites_when_neutralizing_then_every_sentinel_is_distinct(
    test_case: NeutralizeInterpolationTestCase,
) -> None:
    neutralized: str
    sites: tuple[InterpolationSite, ...]
    neutralized, sites = neutralize_interpolation(body=test_case.body, dialect="generic")
    assert neutralized == test_case.expected_neutralized
    sentinels: tuple[str, ...] = tuple(site.sentinel for site in sites)
    assert len(set(sentinels)) == len(test_case.expected_original_texts)


@pytest.mark.parametrize(
    "test_case",
    [
        RestoreInterpolationTestCase(
            description="unchanged neutralized text round-trips to the original body",
            body="SELECT @py.cents_to_dollars(amount) AS a FROM t",
            fixed_neutralized="SELECT __sqlclean_interpolation_0__ AS a FROM t",
            expected_restored="SELECT @py.cents_to_dollars(amount) AS a FROM t",
        ),
        RestoreInterpolationTestCase(
            description="reformatted layout keeps interpolation intact",
            body="select @a as x,\n  @b from t",
            fixed_neutralized="SELECT\n    __sqlclean_interpolation_0__ AS x,\n    __sqlclean_interpolation_1__\nFROM t",
            expected_restored="SELECT\n    @a AS x,\n    @b\nFROM t",
        ),
        RestoreInterpolationTestCase(
            description="body without interpolation is returned unchanged",
            body="SELECT 1 AS x",
            fixed_neutralized="SELECT 1 AS x",
            expected_restored="SELECT 1 AS x",
        ),
        RestoreInterpolationTestCase(
            description="repeated identical interpolation restores both sites",
            body="SELECT @a, @a FROM t",
            fixed_neutralized="SELECT __sqlclean_interpolation_0__, __sqlclean_interpolation_1__ FROM t",
            expected_restored="SELECT @a, @a FROM t",
        ),
        RestoreInterpolationTestCase(
            description="real identifier matching the interpolation name is untouched",
            body="SELECT expression, @expression FROM t",
            fixed_neutralized="SELECT expression, __sqlclean_interpolation_0__ FROM t",
            expected_restored="SELECT expression, @expression FROM t",
        ),
        RestoreInterpolationTestCase(
            description="formatter-normalized table function sentinel restores case insensitively",
            body=(
                "SELECT expanded.order_id FROM orders CROSS JOIN "
                '__table_fn("expand_order")(orders.order_id) AS expanded'
            ),
            fixed_neutralized=(
                "SELECT expanded.order_id FROM orders CROSS JOIN "
                "__SQLCLEAN_INTERPOLATION_0__(orders.order_id) AS expanded"
            ),
            expected_restored=(
                "SELECT expanded.order_id FROM orders CROSS JOIN "
                '__table_fn("expand_order")(orders.order_id) AS expanded'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fixed_text_when_restoring_then_original_interpolation_returns(
    test_case: RestoreInterpolationTestCase,
) -> None:
    sites: tuple[InterpolationSite, ...]
    _neutralized, sites = neutralize_interpolation(body=test_case.body, dialect="generic")
    succeeded, restored = restore_interpolation(fixed=test_case.fixed_neutralized, sites=sites)
    assert succeeded
    assert restored == test_case.expected_restored


@pytest.mark.parametrize(
    "test_case",
    [
        RestoreFailureTestCase(
            description="dropped sentinel fails instead of silently losing interpolation",
            body="SELECT @a FROM t",
            fixed_neutralized="SELECT FROM t",
            expected_message_fragment="contains 0 occurrences",
        ),
        RestoreFailureTestCase(
            description="duplicated sentinel fails instead of duplicating interpolation",
            body="SELECT @a FROM t",
            fixed_neutralized="SELECT __sqlclean_interpolation_0__, __sqlclean_interpolation_0__ FROM t",
            expected_message_fragment="contains 2 occurrences",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_mangled_sentinels_when_restoring_then_failure_is_reported(
    test_case: RestoreFailureTestCase,
) -> None:
    sites: tuple[InterpolationSite, ...]
    _neutralized, sites = neutralize_interpolation(body=test_case.body, dialect="generic")
    succeeded, message = restore_interpolation(fixed=test_case.fixed_neutralized, sites=sites)
    assert not succeeded
    assert test_case.expected_message_fragment in message


@pytest.mark.parametrize(
    "test_case",
    [
        DollarQuotePreparationTestCase(
            "untagged apostrophe",
            "duckdb",
            "$$Customer's order$$",
            ("@py.label($$Customer's order$$)", '__ref("orders")'),
        ),
        DollarQuotePreparationTestCase(
            "tagged apostrophe",
            "postgres",
            "$note$it's$note$",
            ("@py.label($note$it's$note$)", '__ref("orders")'),
        ),
        DollarQuotePreparationTestCase(
            "hidden intrinsic",
            "snowflake",
            "$$__ref('items')$$",
            ("@py.label($$__ref('items')$$)", '__ref("orders")'),
        ),
        DollarQuotePreparationTestCase(
            "hidden macro paren",
            "duckdb",
            "$$@py.m( ')$$",
            ("@py.label($$@py.m( ')$$)", '__ref("orders")'),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dollar_quoted_text_when_neutralizing_then_python_and_native_sites_agree(
    test_case: DollarQuotePreparationTestCase,
) -> None:
    sql: str = (
        f"SELECT {test_case.literal} AS note, @py.label({test_case.literal}) AS label "
        'FROM __ref("orders") o'
    )
    lint_sql, lint_sites = neutralize_interpolation(body=sql, dialect=test_case.dialect)
    format_sql, format_sites = neutralize_interpolation(
        body=sql, dialect=test_case.dialect, for_formatting=True
    )
    native: tuple[str, list[tuple[str, int, int, int, int, str]], list[str]] | None = (
        _native.prepare_lint_sql(
            {
                "expanded": sql,
                "before_expansion": sql,
                "prior_sites": [],
                "dialect": test_case.dialect,
            }
        )
    )

    assert tuple(site.original_text for site in lint_sites) == test_case.expected_sites
    assert tuple(site.original_text for site in format_sites) == test_case.expected_sites
    assert native is not None
    assert native[0] == lint_sql
    assert tuple(InterpolationSite(*site) for site in native[1]) == lint_sites
    assert restore_interpolation(fixed=format_sql, sites=format_sites) == (True, sql)
