"""Tests for source loader row rendering helpers."""

from __future__ import annotations

from datetime import date, datetime
from decimal import Decimal

import duckdb
import pytest

from sqlbuild._native import SourceColumnEntry
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load._helpers.rows import (
    build_rows_sql,
    iter_loader_row_batches,
    iter_normalized_loader_rows,
    update_loader_rows_schema,
)
from sqlbuild.executor.load.models import LoaderRowsSchema
from tests.unit.src.sqlbuild.executor.load._test_types import (
    LoaderRowsBatchTestCase,
    LoaderRowsErrorTestCase,
    LoaderRowsExecutableSqlTestCase,
    LoaderRowsNormalizeErrorTestCase,
    LoaderRowsNormalizeTestCase,
    LoaderRowsSchemaTestCase,
    LoaderRowsSqlTestCase,
)
from tests.unit.src.sqlbuild.executor.load.helpers import LoaderContextTestAdapter


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsSqlTestCase(
            description="infers adapter SQL types for undeclared Python values",
            rows=(
                {
                    "extra_bool": True,
                    "extra_int": 7,
                    "extra_float": 2.5,
                    "extra_decimal": Decimal("3.25"),
                    "extra_string": "placed",
                    "extra_timestamp": datetime(2026, 5, 21, 12, 30),
                    "extra_date": date(2026, 5, 21),
                    "extra_json": {"source": "loader"},
                    "extra_json_list": ["a", "b"],
                    "nullable_then_string": None,
                },
                {"nullable_then_string": "resolved"},
            ),
            expected_sql_fragments=(
                'CAST("extra_bool" AS BOOLEAN) AS "extra_bool"',
                'CAST("extra_int" AS BIGINT) AS "extra_int"',
                'CAST("extra_float" AS DOUBLE) AS "extra_float"',
                'CAST("extra_decimal" AS DECIMAL(3, 2)) AS "extra_decimal"',
                'CAST("extra_string" AS VARCHAR) AS "extra_string"',
                'CAST("extra_timestamp" AS TIMESTAMP) AS "extra_timestamp"',
                'CAST("extra_date" AS DATE) AS "extra_date"',
                'CAST("extra_json" AS JSON) AS "extra_json"',
                'CAST("extra_json_list" AS JSON) AS "extra_json_list"',
                'CAST("nullable_then_string" AS VARCHAR) AS "nullable_then_string"',
            ),
        ),
        LoaderRowsSqlTestCase(
            description="falls back all null inferred column to uncast projection",
            rows=({"all_null": None},),
            expected_sql_fragments=(
                'SELECT "all_null" FROM (VALUES (NULL)) AS __loader_rows("all_null")',
            ),
        ),
        LoaderRowsSqlTestCase(
            description="creates empty declared row query with declared types and order",
            rows=(),
            columns=(
                SourceColumnEntry(name="id", type="INTEGER"),
                SourceColumnEntry(name="status", type="VARCHAR"),
            ),
            expected_sql_fragments=(
                'CAST(NULL AS INTEGER) AS "id", CAST(NULL AS VARCHAR) AS "status"',
                "WHERE 1 = 0",
            ),
        ),
        LoaderRowsSqlTestCase(
            description="preserves declared order before extra first-seen order",
            rows=({"z_extra": 1, "a_extra": 2, "id": 3},),
            columns=(SourceColumnEntry(name="id", type="INTEGER"),),
            expected_sql_fragments=('AS __loader_rows("id", "z_extra", "a_extra")',),
        ),
        LoaderRowsSqlTestCase(
            description="uses declared source column type before inferred value type",
            rows=({"declared_text": 123},),
            columns=(SourceColumnEntry(name="declared_text", type="VARCHAR"),),
            expected_sql_fragments=('CAST("declared_text" AS VARCHAR) AS "declared_text"',),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_rows_when_building_sql_then_infers_adapter_sql_types(
    test_case: LoaderRowsSqlTestCase,
) -> None:
    sql: str = build_rows_sql(
        adapter=LoaderContextTestAdapter(),
        rows=test_case.rows,
        columns=test_case.columns,
    )

    assert all(fragment in sql for fragment in test_case.expected_sql_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsExecutableSqlTestCase(
            description="quotes reserved and special column names in DuckDB loader SQL",
            rows=({"order": 1, "customer id": "c1"},),
            expected_rows=((1, "c1"),),
            expected_sql_fragments=(
                'CAST("order" AS BIGINT) AS "order"',
                'CAST("customer id" AS VARCHAR) AS "customer id"',
                'AS __loader_rows("order", "customer id")',
            ),
        ),
        LoaderRowsExecutableSqlTestCase(
            description="escapes embedded identifier quotes in DuckDB loader SQL",
            rows=({'quote "col"': 2},),
            expected_rows=((2,),),
            expected_sql_fragments=(
                'CAST("quote ""col""" AS BIGINT) AS "quote ""col"""',
                'AS __loader_rows("quote ""col""")',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reserved_loader_row_columns_when_executing_sql_then_quotes_identifiers(
    test_case: LoaderRowsExecutableSqlTestCase,
) -> None:
    sql: str = build_rows_sql(adapter=DuckDbAdapter(), rows=test_case.rows, columns=())

    result: tuple[tuple[object, ...], ...] = tuple(
        duckdb.connect(":memory:").execute(sql).fetchall()
    )

    assert result == test_case.expected_rows
    assert all(fragment in sql for fragment in test_case.expected_sql_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsNormalizeTestCase(
            description="accepts generator rows",
            value=({"id": value} for value in (1, 2)),
            expected_rows=({"id": 1}, {"id": 2}),
        ),
        LoaderRowsNormalizeTestCase(
            description="accepts list rows",
            value=[{"id": 1}],
            expected_rows=({"id": 1},),
        ),
        LoaderRowsNormalizeTestCase(
            description="accepts tuple iterable rows",
            value=({"id": 1}, {"id": 2}),
            expected_rows=({"id": 1}, {"id": 2}),
        ),
        LoaderRowsNormalizeTestCase(
            description="coerces mapping keys to strings",
            value=[{1: "one", "two": 2}],
            expected_rows=({"1": "one", "two": 2},),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_return_value_when_normalizing_then_returns_expected_rows(
    test_case: LoaderRowsNormalizeTestCase,
) -> None:
    assert tuple(iter_normalized_loader_rows(test_case.value)) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsNormalizeErrorTestCase(
            description="rejects string return value",
            value="not rows",
            expected_error_fragment="list or iterable of dict rows",
        ),
        LoaderRowsNormalizeErrorTestCase(
            description="rejects bytes return value",
            value=b"not rows",
            expected_error_fragment="list or iterable of dict rows",
        ),
        LoaderRowsNormalizeErrorTestCase(
            description="rejects non-iterable return value",
            value=42,
            expected_error_fragment="list or iterable of dict rows",
        ),
        LoaderRowsNormalizeErrorTestCase(
            description="rejects non-dict row",
            value=[("id", 1)],
            expected_error_fragment="only dict rows",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_loader_return_value_when_normalizing_then_raises(
    test_case: LoaderRowsNormalizeErrorTestCase,
) -> None:
    with pytest.raises(ExecutorInputError) as exc_info:
        tuple(iter_normalized_loader_rows(test_case.value))

    assert test_case.expected_error_fragment in str(exc_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsBatchTestCase(
            description="splits generator rows into fixed-size batches",
            value=({"id": value} for value in (1, 2, 3)),
            batch_size=2,
            expected_batches=(({"id": 1}, {"id": 2}), ({"id": 3},)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_return_value_when_batching_then_yields_expected_batches(
    test_case: LoaderRowsBatchTestCase,
) -> None:
    assert (
        tuple(iter_loader_row_batches(value=test_case.value, batch_size=test_case.batch_size))
        == test_case.expected_batches
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsSchemaTestCase(
            description="tracks late extra columns after declared columns",
            rows=({"id": 1, "late_flag": True},),
            columns=(SourceColumnEntry(name="id", type="INTEGER"),),
            column_names=("id",),
            expected_column_names=("id", "late_flag"),
            expected_added_columns=(("late_flag", "BOOLEAN"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_row_batch_when_updating_schema_then_tracks_added_columns(
    test_case: LoaderRowsSchemaTestCase,
) -> None:
    schema: LoaderRowsSchema = update_loader_rows_schema(
        adapter=LoaderContextTestAdapter(),
        rows=test_case.rows,
        columns=test_case.columns,
        column_names=test_case.column_names,
        inferred_types={},
    )

    assert schema.column_names == test_case.expected_column_names
    assert tuple((column.name, column.type) for column in schema.added_columns) == (
        test_case.expected_added_columns
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRowsErrorTestCase(
            description="raises when one returned column has conflicting inferred types",
            rows=(
                {"id": 1, "status": "placed"},
                {"id": "two", "status": "shipped"},
            ),
            expected_error_fragment="conflicting types for column 'id'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_rows_with_conflicting_types_when_building_sql_then_raises(
    test_case: LoaderRowsErrorTestCase,
) -> None:
    with pytest.raises(ExecutorInputError) as exc_info:
        build_rows_sql(
            adapter=LoaderContextTestAdapter(),
            rows=test_case.rows,
            columns=(),
        )

    assert test_case.expected_error_fragment in str(exc_info.value)
