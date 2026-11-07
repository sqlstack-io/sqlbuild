"""Tests for adapter source-loader type mappings."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import BuiltinAdapter, LoaderLogicalType
from sqlbuild.adapter.discovery.main.builtins import builtin_adapter_classes
from tests.unit.src.sqlbuild.adapter.contract._test_types import (
    AdapterIdentifierRenderingTestCase,
    AdapterLoaderJsonRowsTestCase,
    AdapterLoaderRowsEmptySelectTestCase,
    AdapterLoaderRowsSelectTestCase,
    AdapterLoaderTypeMappingTestCase,
    AdapterLoaderValueLiteralTestCase,
    AdapterSourceExpressionRenderingTestCase,
)

_DEFAULT_EXPECTED_TYPES: dict[LoaderLogicalType, str] = {
    LoaderLogicalType.BOOLEAN: "BOOLEAN",
    LoaderLogicalType.INTEGER: "BIGINT",
    LoaderLogicalType.FLOAT: "DOUBLE",
    LoaderLogicalType.STRING: "VARCHAR",
    LoaderLogicalType.TIMESTAMP: "TIMESTAMP",
    LoaderLogicalType.DATE: "DATE",
    LoaderLogicalType.JSON: "JSON",
}


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterLoaderTypeMappingTestCase(
            description="duckdb maps every loader logical type",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            expected_types=_DEFAULT_EXPECTED_TYPES,
        ),
        AdapterLoaderTypeMappingTestCase(
            description="motherduck maps every loader logical type",
            adapter_name=BuiltinAdapter.MOTHERDUCK.value,
            expected_types=_DEFAULT_EXPECTED_TYPES,
        ),
        AdapterLoaderTypeMappingTestCase(
            description="bigquery maps every loader logical type",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            expected_types={
                **_DEFAULT_EXPECTED_TYPES,
                LoaderLogicalType.BOOLEAN: "BOOL",
                LoaderLogicalType.INTEGER: "INT64",
                LoaderLogicalType.FLOAT: "FLOAT64",
                LoaderLogicalType.STRING: "STRING",
            },
        ),
        AdapterLoaderTypeMappingTestCase(
            description="snowflake maps json to variant",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            expected_types={**_DEFAULT_EXPECTED_TYPES, LoaderLogicalType.JSON: "VARIANT"},
        ),
        AdapterLoaderTypeMappingTestCase(
            description="databricks maps strings and json to string",
            adapter_name=BuiltinAdapter.DATABRICKS.value,
            expected_types={
                **_DEFAULT_EXPECTED_TYPES,
                LoaderLogicalType.STRING: "STRING",
                LoaderLogicalType.JSON: "STRING",
            },
        ),
        AdapterLoaderTypeMappingTestCase(
            description="postgres maps text float and json to postgres types",
            adapter_name=BuiltinAdapter.POSTGRES.value,
            expected_types={
                **_DEFAULT_EXPECTED_TYPES,
                LoaderLogicalType.FLOAT: "DOUBLE PRECISION",
                LoaderLogicalType.STRING: "TEXT",
                LoaderLogicalType.JSON: "JSONB",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_loader_types_then_maps_every_logical_type(
    test_case: AdapterLoaderTypeMappingTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    logical_type: LoaderLogicalType
    for logical_type in LoaderLogicalType:
        assert (
            adapter.render_loader_logical_type(logical_type)
            == test_case.expected_types[logical_type]
        )


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterIdentifierRenderingTestCase(
            description="duckdb renders double-quoted loader identifiers",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            raw_identifier='order "id"',
            expected_identifier='"order ""id"""',
        ),
        AdapterIdentifierRenderingTestCase(
            description="motherduck renders double-quoted loader identifiers",
            adapter_name=BuiltinAdapter.MOTHERDUCK.value,
            raw_identifier='order "id"',
            expected_identifier='"order ""id"""',
        ),
        AdapterIdentifierRenderingTestCase(
            description="postgres renders double-quoted loader identifiers",
            adapter_name=BuiltinAdapter.POSTGRES.value,
            raw_identifier='order "id"',
            expected_identifier='"order ""id"""',
        ),
        AdapterIdentifierRenderingTestCase(
            description="snowflake renders double-quoted loader identifiers",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            raw_identifier='order "id"',
            expected_identifier='"ORDER ""ID"""',
        ),
        AdapterIdentifierRenderingTestCase(
            description="bigquery renders backtick-quoted loader identifiers",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            raw_identifier="order `id`",
            expected_identifier=r"`order \`id\``",
        ),
        AdapterIdentifierRenderingTestCase(
            description="databricks renders backtick-quoted loader identifiers",
            adapter_name=BuiltinAdapter.DATABRICKS.value,
            raw_identifier="order `id`",
            expected_identifier="`order ``id```",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_identifier_then_uses_adapter_quotes(
    test_case: AdapterIdentifierRenderingTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    assert adapter.render_identifier(test_case.raw_identifier) == test_case.expected_identifier


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterLoaderValueLiteralTestCase(
            description="duckdb renders json as quoted string literal",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal="'{\"source\": \"loader''s api\"}'",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="motherduck renders json as quoted string literal",
            adapter_name=BuiltinAdapter.MOTHERDUCK.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal="'{\"source\": \"loader''s api\"}'",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="snowflake renders json with parse_json",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal="PARSE_JSON('{\"source\": \"loader''s api\"}')",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="postgres renders json with jsonb cast",
            adapter_name=BuiltinAdapter.POSTGRES.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal="'{\"source\": \"loader''s api\"}'::JSONB",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="bigquery renders json with json literal prefix",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal=r"""JSON '{"source": "loader\'s api"}'""",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="databricks renders json as quoted string literal",
            adapter_name=BuiltinAdapter.DATABRICKS.value,
            value={"source": "loader's api"},
            logical_type=LoaderLogicalType.JSON,
            expected_literal=r"""'{"source": "loader\'s api"}'""",
        ),
        AdapterLoaderValueLiteralTestCase(
            description="duckdb escapes string literals",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            value="customer's order",
            logical_type=LoaderLogicalType.STRING,
            expected_literal="'customer''s order'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_loader_value_then_returns_expected_literal(
    test_case: AdapterLoaderValueLiteralTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    assert (
        adapter.render_loader_value_literal(
            value=test_case.value,
            logical_type=test_case.logical_type,
        )
        == test_case.expected_literal
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterLoaderRowsSelectTestCase(
            description="duckdb renders values row select",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            expected_fragments=("FROM (VALUES", 'AS __loader_rows("id", "status")'),
        ),
        AdapterLoaderRowsSelectTestCase(
            description="motherduck renders values row select",
            adapter_name=BuiltinAdapter.MOTHERDUCK.value,
            expected_fragments=("FROM (VALUES", 'AS __loader_rows("id", "status")'),
        ),
        AdapterLoaderRowsSelectTestCase(
            description="postgres renders values row select",
            adapter_name=BuiltinAdapter.POSTGRES.value,
            expected_fragments=("FROM (VALUES", 'AS __loader_rows("id", "status")'),
        ),
        AdapterLoaderRowsSelectTestCase(
            description="snowflake renders values row select",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            expected_fragments=("FROM (VALUES", 'AS __loader_rows("ID", "STATUS")'),
        ),
        AdapterLoaderRowsSelectTestCase(
            description="databricks renders values row select",
            adapter_name=BuiltinAdapter.DATABRICKS.value,
            expected_fragments=("FROM (VALUES", "AS __loader_rows(`id`, `status`)"),
        ),
        AdapterLoaderRowsSelectTestCase(
            description="bigquery renders union row select",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            expected_fragments=(
                "SELECT CAST(1 AS INT64) AS `id`, CAST('placed' AS STRING) AS `status`",
                "UNION ALL",
                "SELECT CAST(2 AS INT64) AS `id`, CAST('shipped' AS STRING) AS `status`",
            ),
            forbidden_fragments=("FROM (VALUES", "__loader_rows"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_loader_rows_then_returns_adapter_sql(
    test_case: AdapterLoaderRowsSelectTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    sql: str = adapter.render_loader_rows_select(
        rows=({"id": 1, "status": "placed"}, {"id": 2, "status": "shipped"}),
        column_names=("id", "status"),
        column_sql_types={
            "id": adapter.render_loader_logical_type(LoaderLogicalType.INTEGER),
            "status": adapter.render_loader_logical_type(LoaderLogicalType.STRING),
        },
        inferred_types={"id": LoaderLogicalType.INTEGER, "status": LoaderLogicalType.STRING},
    )

    assert all(fragment in sql for fragment in test_case.expected_fragments)
    assert not any(fragment in sql for fragment in test_case.forbidden_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterLoaderJsonRowsTestCase(
            description="snowflake parses json in the select list, not inside values",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            rows=({"id": 1, "payload": {"sku": "a-1"}}, {"id": 2, "payload": None}),
            expected_sql=(
                'SELECT CAST("ID" AS BIGINT) AS "ID", '
                'CAST(PARSE_JSON("PAYLOAD") AS VARIANT) AS "PAYLOAD" '
                'FROM (VALUES (1, \'{"sku": "a-1"}\'), (2, NULL)) '
                'AS __loader_rows("ID", "PAYLOAD")'
            ),
        ),
        AdapterLoaderJsonRowsTestCase(
            description="snowflake keeps json escapes and backslashes in string constants",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            rows=({"id": 1, "payload": {"note": 'say "hi"\\now'}},),
            expected_sql=(
                'SELECT CAST("ID" AS BIGINT) AS "ID", '
                'CAST(PARSE_JSON("PAYLOAD") AS VARIANT) AS "PAYLOAD" '
                'FROM (VALUES (1, \'{"note": "say \\\\"hi\\\\"\\\\\\\\now"}\')) '
                'AS __loader_rows("ID", "PAYLOAD")'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_json_rows_when_rendering_loader_rows_then_values_hold_only_constants(
    test_case: AdapterLoaderJsonRowsTestCase,
) -> None:
    adapter: BaseAdapter = builtin_adapter_classes()[test_case.adapter_name]()

    sql: str = adapter.render_loader_rows_select(
        rows=test_case.rows,
        column_names=("id", "payload"),
        column_sql_types={
            "id": adapter.render_loader_logical_type(LoaderLogicalType.INTEGER),
            "payload": adapter.render_loader_logical_type(LoaderLogicalType.JSON),
        },
        inferred_types={"id": LoaderLogicalType.INTEGER, "payload": LoaderLogicalType.JSON},
    )

    assert sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterLoaderRowsEmptySelectTestCase(
            description="bigquery renders empty loader rows with string default",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            expected_sql="SELECT CAST(NULL AS STRING) AS `all_null` WHERE 1 = 0",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_when_rendering_empty_loader_rows_then_returns_expected_sql(
    test_case: AdapterLoaderRowsEmptySelectTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    sql: str = adapter.render_loader_rows_select(
        rows=(),
        column_names=("all_null",),
        column_sql_types={},
        inferred_types={},
    )

    assert sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterSourceExpressionRenderingTestCase(
            description="duckdb renders source expression shape explicitly",
            adapter_name=BuiltinAdapter.DUCKDB.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INTEGER) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                '(SELECT * EXCLUDE ("id"), CAST(id AS INTEGER) AS id FROM raw.orders)'
            ),
        ),
        AdapterSourceExpressionRenderingTestCase(
            description="motherduck renders source expression shape explicitly",
            adapter_name=BuiltinAdapter.MOTHERDUCK.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INTEGER) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                '(SELECT * EXCLUDE ("id"), CAST(id AS INTEGER) AS id FROM raw.orders)'
            ),
        ),
        AdapterSourceExpressionRenderingTestCase(
            description="postgres renders source expression shape explicitly",
            adapter_name=BuiltinAdapter.POSTGRES.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INTEGER) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                '(SELECT * EXCLUDE ("id"), CAST(id AS INTEGER) AS id FROM raw.orders)'
            ),
        ),
        AdapterSourceExpressionRenderingTestCase(
            description="snowflake renders source expression shape explicitly",
            adapter_name=BuiltinAdapter.SNOWFLAKE.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INTEGER) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                '(SELECT * EXCLUDE ("ID"), CAST(id AS INTEGER) AS id FROM raw.orders)'
            ),
        ),
        AdapterSourceExpressionRenderingTestCase(
            description="databricks renders source expression shape explicitly",
            adapter_name=BuiltinAdapter.DATABRICKS.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INTEGER) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                "(SELECT * EXCEPT (`id`), CAST(id AS INTEGER) AS id FROM raw.orders)"
            ),
        ),
        AdapterSourceExpressionRenderingTestCase(
            description="bigquery renders source expression shape and normalized cast types",
            adapter_name=BuiltinAdapter.BIGQUERY.value,
            expected_relation="(SELECT 1 AS id)",
            expected_cast_subquery=(
                "(SELECT CAST(id AS INT64) AS id FROM (SELECT 1 AS id) AS __source_expression)"
            ),
            expected_relation_cast_subquery=(
                "(SELECT * EXCEPT (`id`), CAST(id AS INT64) AS id FROM raw.orders)"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_source_expression_then_returns_adapter_sql(
    test_case: AdapterSourceExpressionRenderingTestCase,
) -> None:
    adapter_class: type[BaseAdapter] = builtin_adapter_classes()[test_case.adapter_name]
    adapter: BaseAdapter = adapter_class()

    relation: str = adapter.render_source_expression_relation(expression=" SELECT 1 AS id; ")
    cast_projection: str = adapter.render_source_expression_cast(
        expression="id",
        target_type="INTEGER",
        alias="id",
    )
    cast_subquery: str = adapter.render_source_expression_cast_subquery(
        source_relation=relation,
        projections=(cast_projection,),
    )
    relation_cast_subquery: str = adapter.render_source_relation_cast_subquery(
        source_relation="raw.orders",
        cast_projections=(cast_projection,),
        cast_column_names=("id",),
        all_columns_cast=False,
    )

    assert relation == test_case.expected_relation
    assert cast_subquery == test_case.expected_cast_subquery
    assert relation_cast_subquery == test_case.expected_relation_cast_subquery
