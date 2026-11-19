import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import DynamicListing, DynamicOperation
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from tests.unit.src.sqlbuild.adapters.helpers import MetadataConnection, execute_through_cursor
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeChangeTrackingTestCase,
    SnowflakeDynamicAlterTestCase,
    SnowflakeDynamicCreateTestCase,
    SnowflakeDynamicOperationTestCase,
    SnowflakeDynamicRefreshTestCase,
)

_SHOW_DYNAMIC_TABLES_COLUMNS: tuple[str, ...] = (
    "name",
    "target_lag",
    "warehouse",
    "scheduling_state",
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeDynamicRefreshTestCase(
            description="a dynamic table lists its lag, refresh warehouse and schedule state",
            rows=(("DAILY_TOTALS", "1 hour", "TRANSFORM_WH", "SUSPENDED"),),
            expected_settings=DynamicListing("1 hour", "TRANSFORM_WH", "SUSPENDED"),
        ),
        SnowflakeDynamicRefreshTestCase(
            description="a neighbour matched by the LIKE wildcard is not the model",
            rows=(("DAILY_TOTALS_ARCHIVE", "DOWNSTREAM", "TRANSFORM_WH", "ACTIVE"),),
            expected_settings=None,
        ),
        SnowflakeDynamicRefreshTestCase(
            description="a relation that is no dynamic table lists nothing",
            rows=(),
            expected_settings=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dynamic_table_listing_when_reading_it_then_reads_one_show(
    test_case: SnowflakeDynamicRefreshTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: MetadataConnection = MetadataConnection(
        test_case.rows, _SHOW_DYNAMIC_TABLES_COLUMNS
    )
    monkeypatch.setattr(adapter, "execute", execute_through_cursor)

    settings: DynamicListing | None = adapter.dynamic_listing(
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
        SnowflakeDynamicCreateTestCase(
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
        SnowflakeDynamicCreateTestCase(
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
        SnowflakeDynamicCreateTestCase(
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
def test_given_dynamic_settings_when_rendering_create_then_mode_and_initialize_are_explicit(
    test_case: SnowflakeDynamicCreateTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_create_dynamic(
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
        SnowflakeDynamicAlterTestCase(
            description="an unset refresh warehouse alters only the lag",
            target_lag="'2 hours'",
            warehouse=None,
            expected_sql=(
                "ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SET TARGET_LAG = '2 hours'",
            ),
        ),
        SnowflakeDynamicAlterTestCase(
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
    test_case: SnowflakeDynamicAlterTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_alter_dynamic(
        destination="ANALYTICS.MARTS.DAILY_TOTALS",
        target_lag=test_case.target_lag,
        warehouse=test_case.warehouse,
    )

    assert statements == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeDynamicOperationTestCase(
            description="suspend",
            operation=DynamicOperation.SUSPEND,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS SUSPEND",
        ),
        SnowflakeDynamicOperationTestCase(
            description="resume",
            operation=DynamicOperation.RESUME,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS RESUME",
        ),
        SnowflakeDynamicOperationTestCase(
            description="refresh",
            operation=DynamicOperation.REFRESH,
            expected_sql="ALTER DYNAMIC TABLE ANALYTICS.MARTS.DAILY_TOTALS REFRESH",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_operation_when_rendering_then_one_alter_dynamic_table_statement(
    test_case: SnowflakeDynamicOperationTestCase,
) -> None:
    statements: tuple[str, ...] = SnowflakeAdapter().render_dynamic_operation(
        destination="ANALYTICS.MARTS.DAILY_TOTALS", operation=test_case.operation
    )

    assert statements == (test_case.expected_sql,)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeChangeTrackingTestCase(
            description="a table a dynamic model reads tracks changes from creation",
            config={"table_type": "transient", "change_tracking": True},
            expected_sql=(
                "CREATE OR REPLACE TRANSIENT TABLE ANALYTICS.STAGING.ORDERS "
                "CHANGE_TRACKING = TRUE AS SELECT 1 AS order_id"
            ),
        ),
        SnowflakeChangeTrackingTestCase(
            description="a permanent table read by none keeps the plain form",
            config={"table_type": "permanent", "change_tracking": False},
            expected_sql=(
                "CREATE OR REPLACE TABLE ANALYTICS.STAGING.ORDERS AS SELECT 1 AS order_id"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_config_when_creating_table_then_change_tracking_follows_it(
    test_case: SnowflakeChangeTrackingTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: MetadataConnection = MetadataConnection(())
    monkeypatch.setattr(adapter, "execute", execute_through_cursor)

    adapter.create_table_as(
        connection=connection,
        destination="ANALYTICS.STAGING.ORDERS",
        sql="SELECT 1 AS order_id",
        config=test_case.config,
        statement_recorder=StatementRecorder(),
    )

    assert [sql.removesuffix(" ()") for sql in connection.executed] == [test_case.expected_sql]
