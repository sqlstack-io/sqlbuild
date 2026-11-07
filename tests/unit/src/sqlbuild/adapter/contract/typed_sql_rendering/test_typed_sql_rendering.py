from __future__ import annotations

from decimal import Decimal

import pytest

from sqlbuild._native import SqlValue
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapter.contract.typed_sql_rendering._test_types import (
    AdapterTypedSqlRenderingTestCase,
    InvalidTypedArrayTestCase,
    NativeArrayRenderingTestCase,
)
from tests.unit.src.sqlbuild.adapter.contract.typed_sql_rendering.helpers import typed_sql_value


@pytest.mark.parametrize(
    "test_case",
    (
        AdapterTypedSqlRenderingTestCase(
            description="duckdb",
            adapter_factory=DuckDbAdapter,
            expected_string="'O''Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='json(\'{"amount":2.4700,"label":"O\'\'Brien"}\')',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="motherduck",
            adapter_factory=MotherDuckAdapter,
            expected_string="'O''Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='json(\'{"amount":2.4700,"label":"O\'\'Brien"}\')',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="postgres",
            adapter_factory=PostgresAdapter,
            expected_string="'O''Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='\'{"amount":2.4700,"label":"O\'\'Brien"}\'::JSONB',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="snowflake",
            adapter_factory=SnowflakeAdapter,
            expected_string="'O''Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='PARSE_JSON(\'{"amount":2.4700,"label":"O\'\'Brien"}\')',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="bigquery",
            adapter_factory=BigQueryAdapter,
            expected_string="'O\\'Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="NUMERIC '2.4700'",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='JSON \'{"amount":2.4700,"label":"O\\\'Brien"}\'',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="databricks",
            adapter_factory=DatabricksAdapter,
            expected_string="'O\\'Brien'",
            expected_true="TRUE",
            expected_false="FALSE",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="('GB', 'FR')",
            expected_object='parse_json(\'{"amount":2.4700,"label":"O\\\'Brien"}\')',
        ),
        AdapterTypedSqlRenderingTestCase(
            description="sqlserver",
            adapter_factory=SqlServerAdapter,
            expected_string="N'O''Brien'",
            expected_true="1",
            expected_false="0",
            expected_integer="-7",
            expected_float="1.25",
            expected_decimal="2.4700",
            expected_null="NULL",
            expected_value_list="(N'GB', N'FR')",
            expected_object='JSON_QUERY(N\'{"amount":2.4700,"label":"O\'\'Brien"}\')',
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_values_when_rendering_then_adapter_uses_expected_dialect_sql(
    test_case: AdapterTypedSqlRenderingTestCase,
) -> None:
    adapter: StrictAdapter = test_case.adapter_factory()

    assert (
        adapter.render_typed_scalar(value=typed_sql_value("O'Brien")) == test_case.expected_string
    )
    assert adapter.render_typed_scalar(value=typed_sql_value(True)) == test_case.expected_true
    assert adapter.render_typed_scalar(value=typed_sql_value(False)) == test_case.expected_false
    assert adapter.render_typed_scalar(value=typed_sql_value(-7)) == test_case.expected_integer
    assert adapter.render_typed_scalar(value=typed_sql_value(1.25)) == test_case.expected_float
    assert adapter.render_typed_scalar(value=typed_sql_value(Decimal("2.4700"))) == (
        test_case.expected_decimal
    )
    assert adapter.render_typed_scalar(value=typed_sql_value(None)) == test_case.expected_null
    assert adapter.render_typed_value_list(value=typed_sql_value(["GB", "FR"])) == (
        test_case.expected_value_list
    )
    value: SqlValue = typed_sql_value({"label": "O'Brien", "amount": Decimal("2.4700")})
    assert adapter.render_typed_object(value=value) == test_case.expected_object


@pytest.mark.parametrize(
    "test_case",
    (
        NativeArrayRenderingTestCase(
            description="duckdb",
            adapter_factory=DuckDbAdapter,
            raw_value=["GB", "FR"],
            expected_sql="['GB', 'FR']",
        ),
        NativeArrayRenderingTestCase(
            description="motherduck",
            adapter_factory=MotherDuckAdapter,
            raw_value=["GB", "FR"],
            expected_sql="['GB', 'FR']",
        ),
        NativeArrayRenderingTestCase(
            description="postgres",
            adapter_factory=PostgresAdapter,
            raw_value=["GB", "FR"],
            expected_sql="ARRAY['GB', 'FR']",
        ),
        NativeArrayRenderingTestCase(
            description="snowflake",
            adapter_factory=SnowflakeAdapter,
            raw_value=["GB", "FR"],
            expected_sql="ARRAY_CONSTRUCT('GB', 'FR')",
        ),
        NativeArrayRenderingTestCase(
            description="bigquery",
            adapter_factory=BigQueryAdapter,
            raw_value=["GB", "FR"],
            expected_sql="['GB', 'FR']",
        ),
        NativeArrayRenderingTestCase(
            description="databricks",
            adapter_factory=DatabricksAdapter,
            raw_value=["GB", "FR"],
            expected_sql="array('GB', 'FR')",
        ),
        NativeArrayRenderingTestCase(
            description="postgres nested rectangular",
            adapter_factory=PostgresAdapter,
            raw_value=[[1, 2], [3, 4]],
            expected_sql="ARRAY[ARRAY[1, 2], ARRAY[3, 4]]",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_supported_typed_collection_when_rendering_then_uses_native_array(
    test_case: NativeArrayRenderingTestCase,
) -> None:
    adapter: StrictAdapter = test_case.adapter_factory()

    assert adapter.render_typed_array(value=typed_sql_value(test_case.raw_value)) == (
        test_case.expected_sql
    )


@pytest.mark.parametrize(
    "test_case",
    (
        InvalidTypedArrayTestCase(
            description="postgres rejects ragged nested arrays",
            adapter_factory=PostgresAdapter,
            raw_value=[[1, 2], [3]],
            expected_error="requires rectangular nested arrays",
        ),
        InvalidTypedArrayTestCase(
            description="bigquery rejects nested arrays",
            adapter_factory=BigQueryAdapter,
            raw_value=[[1, 2], [3, 4]],
            expected_error="does not support nested arrays",
        ),
        InvalidTypedArrayTestCase(
            description="sqlserver rejects arrays",
            adapter_factory=SqlServerAdapter,
            raw_value=["GB", "FR"],
            expected_error="does not support typed SQL array rendering",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unsupported_typed_array_when_rendering_then_raises_clear_error(
    test_case: InvalidTypedArrayTestCase,
) -> None:
    adapter: StrictAdapter = test_case.adapter_factory()

    with pytest.raises(AdapterUserError, match=test_case.expected_error):
        adapter.render_typed_array(value=typed_sql_value(test_case.raw_value))


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
