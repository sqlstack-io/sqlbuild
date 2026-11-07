"""BigQuery string literals, quoted identifiers and column type changes use GoogleSQL syntax."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from tests.unit.src.sqlbuild.adapter.contract.typed_sql_rendering.helpers import typed_sql_value
from tests.unit.src.sqlbuild.adapters.bigquery._test_types import (
    BigQueryGoogleSqlRenderingTestCase,
    BigQueryStringLiteralTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryStringLiteralTestCase(
            description="single quote is backslash-escaped",
            value="it's",
            expected_literal=r"'it\'s'",
        ),
        BigQueryStringLiteralTestCase(
            description="backslash is escaped before the quote",
            value="C:\\orders\\'",
            expected_literal=r"'C:\\orders\\\''",
        ),
        BigQueryStringLiteralTestCase(
            description="line breaks become escape sequences",
            value="first\r\nsecond",
            expected_literal=r"'first\r\nsecond'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_special_characters_when_rendering_string_literal_then_uses_backslash_escapes(
    test_case: BigQueryStringLiteralTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    assert adapter.render_typed_scalar(value=typed_sql_value(test_case.value)) == (
        test_case.expected_literal
    )
    assert adapter.render_loader_value_literal(value=test_case.value, logical_type=None) == (
        test_case.expected_literal
    )


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryGoogleSqlRenderingTestCase(
            description="json text keeps its backslashes escaped",
            render=lambda adapter: adapter.render_typed_object(
                value=typed_sql_value({"note": 'say "hi"'})
            ),
            expected_sql=r"""JSON '{"note":"say \\"hi\\""}'""",
        ),
        BigQueryGoogleSqlRenderingTestCase(
            description="quoted identifier escapes backticks and backslashes",
            render=lambda adapter: adapter.render_identifier("order`s\\total"),
            expected_sql=r"`order\`s\\total`",
        ),
        BigQueryGoogleSqlRenderingTestCase(
            description="quoted identifier keeps an apostrophe as stored view definitions spell it",
            render=lambda adapter: adapter.render_identifier("customer's id"),
            expected_sql="`customer's id`",
        ),
        BigQueryGoogleSqlRenderingTestCase(
            description="column type change uses set data type",
            render=lambda adapter: adapter.render_alter_column_types(
                destination="`analytics.orders`",
                columns=(
                    ColumnInfo(name="amount", type="NUMERIC"),
                    ColumnInfo(name="note", type="STRING"),
                ),
            ),
            expected_sql=(
                "ALTER TABLE `analytics.orders` ALTER COLUMN `amount` SET DATA TYPE NUMERIC",
                "ALTER TABLE `analytics.orders` ALTER COLUMN `note` SET DATA TYPE STRING",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_statement_parts_when_rendering_then_uses_googlesql_syntax(
    test_case: BigQueryGoogleSqlRenderingTestCase,
) -> None:
    assert test_case.render(BigQueryAdapter()) == test_case.expected_sql


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
