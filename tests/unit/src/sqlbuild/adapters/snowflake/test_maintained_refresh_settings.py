import pytest

from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from tests.unit.src.sqlbuild.adapters.helpers import MetadataConnection, execute_through_cursor
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeMaintainedRefreshTestCase,
)

_SHOW_DYNAMIC_TABLES_COLUMNS: tuple[str, ...] = ("name", "target_lag", "warehouse")


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMaintainedRefreshTestCase(
            description="a dynamic table lists its lag and refresh warehouse",
            rows=(("DAILY_TOTALS", "1 hour", "TRANSFORM_WH"),),
            expected_settings=("1 hour", "TRANSFORM_WH"),
        ),
        SnowflakeMaintainedRefreshTestCase(
            description="a neighbour matched by the LIKE wildcard is not the model",
            rows=(("DAILY_TOTALS_ARCHIVE", "DOWNSTREAM", "TRANSFORM_WH"),),
            expected_settings=None,
        ),
        SnowflakeMaintainedRefreshTestCase(
            description="a relation that is no dynamic table lists nothing",
            rows=(),
            expected_settings=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dynamic_table_listing_when_reading_refresh_settings_then_reads_one_show(
    test_case: SnowflakeMaintainedRefreshTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: MetadataConnection = MetadataConnection(
        test_case.rows, _SHOW_DYNAMIC_TABLES_COLUMNS
    )
    monkeypatch.setattr(adapter, "execute", execute_through_cursor)

    settings: tuple[str, str] | None = adapter.maintained_refresh_settings(
        connection=connection, database="ANALYTICS", schema="MARTS", name="daily_totals"
    )

    assert settings == test_case.expected_settings
    assert len(connection.executed) == 1
    assert connection.executed[0].startswith(
        'SHOW DYNAMIC TABLES LIKE \'DAILY_TOTALS\' IN SCHEMA "ANALYTICS"."MARTS"'
    )
