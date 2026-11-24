from __future__ import annotations

import pytest

from sqlbuild._native import PlannerInputError, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from tests.unit.src.sqlbuild.adapter.contract.main.helpers import (
    ADAPTERS_BY_NAME,
    BinderException,
    CatalogException,
    ProgrammingError,
    RecordingProbeConnection,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.warehouse._test_types import (
    SourceTableProbeTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.warehouse.helpers import (
    check_one_source_table,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceTableProbeTestCase(
            description="listed source needs no probe",
            adapter_name="duckdb",
            source=SourceEntry(name="raw_orders", schema="raw", table="orders"),
            listed_source_names=frozenset({"raw_orders"}),
            probe_errors=(),
            expected_probe_statements=(),
        ),
        SourceTableProbeTestCase(
            description="databricks source without a catalog is probed by its rendered name",
            adapter_name="databricks",
            source=SourceEntry(name="raw_events", schema="raw", table="events"),
            listed_source_names=frozenset(),
            probe_errors=(),
            expected_probe_statements=("SELECT 1 FROM `raw`.`events` WHERE 1=0",),
        ),
        SourceTableProbeTestCase(
            description="sql server cross-database source is probed fully qualified",
            adapter_name="sqlserver",
            source=SourceEntry(name="erp_orders", database="erp", schema="dbo", table="orders"),
            listed_source_names=frozenset(),
            probe_errors=(),
            expected_probe_statements=("SELECT 1 FROM erp.dbo.orders WHERE 1=0",),
        ),
        SourceTableProbeTestCase(
            description="postgres materialized view source exists by probe",
            adapter_name="postgres",
            source=SourceEntry(name="order_totals", schema="reporting", table="order_totals"),
            listed_source_names=frozenset(),
            probe_errors=(),
            expected_probe_statements=("SELECT 1 FROM reporting.order_totals WHERE 1=0",),
        ),
        SourceTableProbeTestCase(
            description="snowflake stream source exists by probe",
            adapter_name="snowflake",
            source=SourceEntry(
                name="order_changes", database="ANALYTICS", schema="RAW", table="ORDER_STREAM"
            ),
            listed_source_names=frozenset(),
            probe_errors=(),
            expected_probe_statements=("SELECT 1 FROM ANALYTICS.RAW.ORDER_STREAM WHERE 1=0",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_the_listing_missed_when_probe_succeeds_then_plan_is_not_blocked(
    test_case: SourceTableProbeTestCase,
) -> None:
    adapter: BaseAdapter = ADAPTERS_BY_NAME[test_case.adapter_name]()
    connection: RecordingProbeConnection = RecordingProbeConnection(errors=test_case.probe_errors)

    check_one_source_table(
        adapter=adapter,
        connection=connection,
        source=test_case.source,
        listed_source_names=test_case.listed_source_names,
    )

    assert tuple(connection.statements) == test_case.expected_probe_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SourceTableProbeTestCase(
            description="duckdb missing table fails with S405",
            adapter_name="duckdb",
            source=SourceEntry(name="raw_payments", schema="raw", table="payments"),
            listed_source_names=frozenset(),
            probe_errors=(
                CatalogException("Catalog Error: Table with name payments does not exist!"),
            ),
            expected_probe_statements=("SELECT 1 FROM raw.payments WHERE 1=0",),
            expected_error_fragments=(
                "1 source table read by selected resources does not exist in the warehouse:",
                "source 'raw_payments' (raw.payments), read by orders",
            ),
            expected_help_fragment="create or load the table, correct its database",
            unexpected_output_fragment="not readable",
        ),
        SourceTableProbeTestCase(
            description="duckdb missing attached catalog fails with S405",
            adapter_name="duckdb",
            source=SourceEntry(name="lake_orders", database="lake", schema="raw", table="orders"),
            listed_source_names=frozenset(),
            probe_errors=(BinderException('Binder Error: Catalog "lake" does not exist!'),),
            expected_probe_statements=("SELECT 1 FROM lake.raw.orders WHERE 1=0",),
            expected_error_fragments=("source 'lake_orders' (lake.raw.orders), read by orders",),
            expected_help_fragment="create or load the table",
        ),
        SourceTableProbeTestCase(
            description="databricks missing schema fails with S405",
            adapter_name="databricks",
            source=SourceEntry(name="lake_orders", database="lake", schema="raw", table="orders"),
            listed_source_names=frozenset(),
            probe_errors=(
                Exception("[SCHEMA_NOT_FOUND] The schema `lake`.`raw` cannot be found."),
            ),
            expected_probe_statements=("SELECT 1 FROM `lake`.`raw`.`orders` WHERE 1=0",),
            expected_error_fragments=("source 'lake_orders' (`lake`.`raw`.`orders`)",),
            expected_help_fragment="create or load the table",
        ),
        SourceTableProbeTestCase(
            description="snowflake missing or unreadable names the role",
            adapter_name="snowflake",
            source=SourceEntry(
                name="raw_payments", database="ANALYTICS", schema="RAW", table="PAYMENTS"
            ),
            listed_source_names=frozenset(),
            probe_errors=(
                ProgrammingError(
                    "Object 'ANALYTICS.RAW.PAYMENTS' does not exist or not authorized.",
                    errno=2003,
                ),
            ),
            role="TRANSFORMER",
            expected_probe_statements=("SELECT 1 FROM ANALYTICS.RAW.PAYMENTS WHERE 1=0",),
            expected_error_fragments=(
                "does not exist in the warehouse or is not readable by the current role:",
                "(ANALYTICS.RAW.PAYMENTS), read by orders; the warehouse reports it does not "
                "exist or is not readable by role TRANSFORMER",
            ),
            expected_help_fragment="grant SELECT on it to the role in use (TRANSFORMER)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_probe_not_found_when_checking_then_raises_s405(
    test_case: SourceTableProbeTestCase,
) -> None:
    adapter: BaseAdapter = ADAPTERS_BY_NAME[test_case.adapter_name]()
    connection: RecordingProbeConnection = RecordingProbeConnection(
        errors=test_case.probe_errors, role=test_case.role
    )

    with pytest.raises(PlannerInputError) as raised:
        check_one_source_table(
            adapter=adapter,
            connection=connection,
            source=test_case.source,
            listed_source_names=test_case.listed_source_names,
        )

    assert raised.value.code == "S405"
    assert all(fragment in raised.value.message for fragment in test_case.expected_error_fragments)
    assert test_case.expected_help_fragment in str(raised.value.help)
    assert "sql/sources/raw.yml:2" in str(raised.value.help)
    assert test_case.unexpected_output_fragment not in raised.value.message + str(raised.value.help)
    assert tuple(connection.statements) == test_case.expected_probe_statements


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
