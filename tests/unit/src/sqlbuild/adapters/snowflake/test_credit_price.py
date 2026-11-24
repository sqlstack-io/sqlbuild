from __future__ import annotations

from collections import Counter
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path

import duckdb
import pytest

from sqlbuild.adapters.snowflake._helpers.cost import list_price, resolve_credit_price
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.snowflake.constants import RATE_SHEET_SQL
from sqlbuild.adapters.snowflake.models import CreditPrice
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    AddedCostColumnsTestCase,
    CreditPriceCacheTestCase,
    CreditPriceTestCase,
    ListPriceTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.helpers import FakeCostAccount

_DAY: datetime = datetime(2026, 11, 24, 9, tzinfo=UTC)


@pytest.mark.parametrize(
    "test_case",
    [
        CreditPriceTestCase(
            description="the rate sheet's price, when the role can read it",
            account_row=("ORDERS01", "AWS_EU_CENTRAL_1"),
            rate_sheet_row=(Decimal("2.73"), "USD"),
            rate_sheet_error=False,
            expected_price="2.73",
            expected_source="rate_sheet",
        ),
        CreditPriceTestCase(
            description="no permission on the rate sheet falls through to the region's list price",
            account_row=("ORDERS02", "PUBLIC.AWS_EU_CENTRAL_1"),
            rate_sheet_row=None,
            rate_sheet_error=True,
            expected_price="3.90",
            expected_source="list_price",
        ),
        CreditPriceTestCase(
            description="a rate sheet in another currency isn't used",
            account_row=("ORDERS03", "AWS_US_WEST_2"),
            rate_sheet_row=(Decimal("2.10"), "EUR"),
            rate_sheet_error=False,
            expected_price="3.00",
            expected_source="list_price",
        ),
        CreditPriceTestCase(
            description="a region missing from the table is priced as US East",
            account_row=("ORDERS04", "AWS_ME_CENTRAL_1"),
            rate_sheet_row=None,
            rate_sheet_error=False,
            expected_price="3.00",
            expected_source="list_price",
        ),
        CreditPriceTestCase(
            description="an account that can't be read gets the US East list price",
            account_row=None,
            rate_sheet_row=None,
            rate_sheet_error=True,
            expected_price="3.00",
            expected_source="list_price",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_account_when_pricing_a_credit_then_rate_sheet_then_list_price(
    test_case: CreditPriceTestCase, tmp_path: Path
) -> None:
    account: FakeCostAccount = FakeCostAccount(
        account_row=test_case.account_row,
        rate_sheet_row=test_case.rate_sheet_row,
        rate_sheet_error=test_case.rate_sheet_error,
    )

    price: CreditPrice = resolve_credit_price(connection=account, cache_dir=tmp_path, today=_DAY)

    assert (price.usd_per_credit, price.source) == (
        Decimal(test_case.expected_price),
        test_case.expected_source,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ListPriceTestCase(
            description="Enterprise in London",
            region="AWS_EU_WEST_2",
            edition="ENTERPRISE",
            expected_price="4.00",
        ),
        ListPriceTestCase(
            description="Standard in Tokyo",
            region="AWS_AP_NORTHEAST_1",
            edition="STANDARD",
            expected_price="2.85",
        ),
        ListPriceTestCase(
            description="Business Critical on Azure West Europe",
            region="AZURE_WESTEUROPE",
            edition="BUSINESS_CRITICAL",
            expected_price="5.20",
        ),
        ListPriceTestCase(
            description="an unknown edition reads as Enterprise",
            region="GCP_US_CENTRAL1",
            edition="VPS",
            expected_price="3.00",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_region_and_edition_when_reading_the_list_price_then_matches_the_table(
    test_case: ListPriceTestCase,
) -> None:
    price: CreditPrice = list_price(region=test_case.region, edition=test_case.edition)

    assert (price.usd_per_credit, price.source) == (Decimal(test_case.expected_price), "list_price")


@pytest.mark.parametrize(
    "test_case",
    [
        CreditPriceCacheTestCase(
            description="a second lookup the same day reuses the first price",
            first_rate="2.40",
            second_rate="9.99",
            expected_price="2.40",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_price_kept_for_the_day_when_pricing_again_then_the_rate_sheet_is_not_read(
    test_case: CreditPriceCacheTestCase, tmp_path: Path
) -> None:
    first: FakeCostAccount = FakeCostAccount(
        account_row=("ORDERS05", "AWS_US_EAST_1"),
        rate_sheet_row=(Decimal(test_case.first_rate), "USD"),
        rate_sheet_error=False,
    )
    again: FakeCostAccount = FakeCostAccount(
        account_row=("ORDERS05", "AWS_US_EAST_1"),
        rate_sheet_row=(Decimal(test_case.second_rate), "USD"),
        rate_sheet_error=False,
    )
    _ = resolve_credit_price(connection=first, cache_dir=tmp_path, today=_DAY)

    price: CreditPrice = resolve_credit_price(connection=again, cache_dir=tmp_path, today=_DAY)

    assert price.usd_per_credit == Decimal(test_case.expected_price)
    assert RATE_SHEET_SQL not in again.statements


@pytest.mark.parametrize(
    "test_case",
    [
        AddedCostColumnsTestCase(
            description="one ALTER ... ADD COLUMN IF NOT EXISTS per price column",
            database="analytics",
            schema="marts",
            expected_statements=[
                "ALTER TABLE analytics.marts._sqlbuild_resource_costs "
                "ADD COLUMN IF NOT EXISTS usd_per_credit VARCHAR",
                "ALTER TABLE analytics.marts._sqlbuild_resource_costs "
                "ADD COLUMN IF NOT EXISTS usd_per_credit_source VARCHAR",
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_when_rendering_added_cost_columns_then_one_alter_each(
    test_case: AddedCostColumnsTestCase,
) -> None:
    statements: list[str] = SnowflakeAdapter().render_add_resource_costs_columns_sql(
        database=test_case.database, schema=test_case.schema
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        AddedCostColumnsTestCase(
            description="a new table is created with both price columns",
            database="analytics",
            schema="marts",
            expected_statements=[
                "estimated_usd VARCHAR, usd_per_credit VARCHAR, usd_per_credit_source VARCHAR, "
                "status VARCHAR NOT NULL"
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_new_table_when_creating_resource_costs_then_it_has_the_price_columns(
    test_case: AddedCostColumnsTestCase,
) -> None:
    sql: str = SnowflakeAdapter().render_create_resource_costs_table_sql(
        database=test_case.database, schema=test_case.schema
    )

    assert test_case.expected_statements[0] in sql


@pytest.mark.parametrize(
    "test_case",
    [
        AddedCostColumnsTestCase(
            description="an older table gains both columns once, however often they are added",
            database=None,
            schema="main",
            expected_statements=["usd_per_credit", "usd_per_credit_source"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_from_before_prices_when_adding_columns_twice_then_both_exist_once(
    test_case: AddedCostColumnsTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    create: str = adapter.render_create_resource_costs_table_sql(
        database=test_case.database, schema=test_case.schema
    )
    older: str = create.replace(" usd_per_credit VARCHAR,", "").replace(
        " usd_per_credit_source VARCHAR,", ""
    )
    connection: duckdb.DuckDBPyConnection = duckdb.connect()
    _ = connection.execute(f"CREATE SCHEMA IF NOT EXISTS {test_case.schema}")
    _ = connection.execute(older.split(") DATA_RETENTION")[0] + ")")
    statements: list[str] = adapter.render_add_resource_costs_columns_sql(
        database=test_case.database, schema=test_case.schema
    )

    for statement in statements + statements:
        _ = connection.execute(statement)

    described: list[tuple[object, ...]] = connection.execute(
        f"DESCRIBE {test_case.schema}._sqlbuild_resource_costs"
    ).fetchall()
    names: Counter[str] = Counter(str(row[0]) for row in described)
    assert {column: names[column] for column in test_case.expected_statements} == dict.fromkeys(
        test_case.expected_statements, 1
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
