import pytest

from sqlbuild.adapter.contract.types import MaintainedOperation
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from tests.unit.src.sqlbuild.adapters.helpers import MetadataConnection, execute_through_cursor
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeMaintainedAlterTestCase,
    SnowflakeMaintainedCreateTestCase,
    SnowflakeMaintainedOperationTestCase,
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


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMaintainedCreateTestCase(
            description="a lag, the default incremental mode and an unquoted warehouse",
            target_lag="'1 hour'",
            refresh_mode="incremental",
            warehouse="transform_wh",
            expected_sql=(
                "CREATE OR REPLACE DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS "
                "TARGET_LAG = '1 hour' WAREHOUSE = transform_wh REFRESH_MODE = INCREMENTAL "
                "INITIALIZE = ON_CREATE AS SELECT 1 AS orders"
            ),
        ),
        SnowflakeMaintainedCreateTestCase(
            description="refreshed for its readers, in adaptive mode",
            target_lag="DOWNSTREAM",
            refresh_mode="adaptive",
            warehouse="TRANSFORM_WH",
            expected_sql=(
                "CREATE OR REPLACE DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS "
                "TARGET_LAG = DOWNSTREAM WAREHOUSE = TRANSFORM_WH REFRESH_MODE = ADAPTIVE "
                "INITIALIZE = ON_CREATE AS SELECT 1 AS orders"
            ),
        ),
        SnowflakeMaintainedCreateTestCase(
            description="a warehouse name that needs quoting",
            target_lag="'5 minutes'",
            refresh_mode="full",
            warehouse="transform-wh",
            expected_sql=(
                "CREATE OR REPLACE DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS "
                "TARGET_LAG = '5 minutes' WAREHOUSE = \"transform-wh\" REFRESH_MODE = FULL "
                "INITIALIZE = ON_CREATE AS SELECT 1 AS orders"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_maintained_settings_when_rendering_create_then_mode_and_initialize_are_explicit(
    test_case: SnowflakeMaintainedCreateTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_create_maintained(
        destination="ANALYTICS.MARTS.DAILY_TOTALS",
        sql="SELECT 1 AS orders",
        target_lag=test_case.target_lag,
        refresh_mode=test_case.refresh_mode,
        warehouse=test_case.warehouse,
    )

    assert statements == (test_case.expected_sql,)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMaintainedAlterTestCase(
            description="an unset refresh warehouse alters only the lag",
            target_lag="'2 hours'",
            warehouse=None,
            expected_sql=(
                "ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SET TARGET_LAG = '2 hours'",
            ),
        ),
        SnowflakeMaintainedAlterTestCase(
            description="a declared refresh warehouse is set too",
            target_lag="DOWNSTREAM",
            warehouse="reporting_wh",
            expected_sql=(
                "ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SET TARGET_LAG = DOWNSTREAM",
                "ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SET WAREHOUSE = reporting_wh",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_drifted_settings_when_rendering_alter_then_settings_change_in_place(
    test_case: SnowflakeMaintainedAlterTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_alter_maintained(
        destination="ANALYTICS.MARTS.DAILY_TOTALS",
        target_lag=test_case.target_lag,
        warehouse=test_case.warehouse,
    )

    assert statements == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMaintainedOperationTestCase(
            description="suspend",
            operation=MaintainedOperation.SUSPEND,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SUSPEND",
        ),
        SnowflakeMaintainedOperationTestCase(
            description="resume",
            operation=MaintainedOperation.RESUME,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS RESUME",
        ),
        SnowflakeMaintainedOperationTestCase(
            description="refresh",
            operation=MaintainedOperation.REFRESH,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS REFRESH",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_operation_when_rendering_then_one_alter_dynamic_table_statement(
    test_case: SnowflakeMaintainedOperationTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_maintained_operation(
        destination="ANALYTICS.MARTS.DAILY_TOTALS", operation=test_case.operation
    )

    assert statements == (test_case.expected_sql,)
