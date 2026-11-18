from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

import pytest

from sqlbuild._native import (
    Dialect,
    SourceEntry,
    SourceFreshnessConfig,
    SourceFreshnessObservation,
    SourceFreshnessObservationError,
    SourceFreshnessStrategy,
    SourceFreshnessValueKind,
    live_reads,
    normalize_source_freshness_data_version,
    observe_configured_source_freshness,
)
from sqlbuild.adapter.contract.models import TableFreshnessMetadata
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.source_freshness.main.configured_observation._test_types import (
    SourceFreshnessObservationErrorTestCase,
    SourceFreshnessObservationTestCase,
    SourceFreshnessStateErrorTestCase,
    UnsupportedTableFreshnessMetadataGuardTestCase,
)


class UnsupportedFreshnessMetadataDuckDbAdapter(DuckDbAdapter):
    metadata_requested: bool

    def __init__(self) -> None:
        super().__init__()
        self.metadata_requested = False

    def supports_table_freshness_metadata(self) -> bool:
        return False

    def get_table_freshness_metadata(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        self.metadata_requested = True
        return TableFreshnessMetadata(
            data_version=datetime(2026, 1, 1, 12, 0, 0),
            value_kind="timestamp",
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SourceFreshnessObservationTestCase(
            description="observes max column data version",
            setup_sql=(
                "CREATE TABLE raw_orders (updated_at INTEGER)",
                "INSERT INTO raw_orders VALUES (1), (3), (2)",
            ),
            source_name="raw_orders",
            table="raw_orders",
            strategy="column",
            column="updated_at",
            value_kind=SourceFreshnessValueKind.INTEGER,
            expected_data_version=3,
        ),
        SourceFreshnessObservationTestCase(
            description="observes sql data version",
            setup_sql=(),
            source_name="raw_orders",
            table=None,
            strategy="sql",
            query="SELECT 'version-1' AS data_version",
            value_kind="string",
            expected_data_version="version-1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_config_when_observing_then_returns_data_version(
    test_case: SourceFreshnessObservationTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    observed_at: datetime = datetime(2026, 1, 1, 12, 0, 0, tzinfo=UTC)
    try:
        statement: str
        for statement in test_case.setup_sql:
            adapter.execute(connection=connection, sql=statement)
        observation: SourceFreshnessObservation = observe_configured_source_freshness(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            source=SourceEntry(
                name=test_case.source_name,
                table=test_case.table,
                freshness=SourceFreshnessConfig(
                    strategy=SourceFreshnessStrategy(test_case.strategy),
                    value_kind=SourceFreshnessValueKind(test_case.value_kind),
                    column=test_case.column,
                    query=test_case.query,
                ),
            ),
            observed_at=observed_at,
        )
    finally:
        adapter.close(connection)

    assert observation.source_name == test_case.source_name
    assert observation.data_version == test_case.expected_data_version
    assert observation.value_kind == SourceFreshnessValueKind(test_case.value_kind)
    assert observation.observed_at == observed_at


@pytest.mark.parametrize(
    "test_case",
    [
        SourceFreshnessObservationErrorTestCase(
            description="raises when sql freshness returns multiple columns",
            setup_sql=(),
            source_name="raw_orders",
            table=None,
            strategy="sql",
            query="SELECT 1 AS left_value, 2 AS right_value",
            value_kind=SourceFreshnessValueKind.INTEGER,
            expected_error_fragment="must return exactly one column",
        ),
        SourceFreshnessObservationErrorTestCase(
            description="raises when sql freshness returns zero rows",
            setup_sql=(),
            source_name="raw_orders",
            table=None,
            strategy="sql",
            query="SELECT 1 AS data_version WHERE FALSE",
            value_kind=SourceFreshnessValueKind.INTEGER,
            expected_error_fragment="must return exactly one row",
        ),
        SourceFreshnessObservationErrorTestCase(
            description="raises when sql freshness returns null",
            setup_sql=(),
            source_name="raw_orders",
            table=None,
            strategy="sql",
            query="SELECT NULL AS data_version",
            value_kind=SourceFreshnessValueKind.INTEGER,
            expected_error_fragment="data_version cannot be null",
        ),
        SourceFreshnessObservationErrorTestCase(
            description="raises when adapter metadata is unsupported",
            setup_sql=(),
            source_name="raw_orders",
            table="raw_orders",
            strategy="adapter",
            value_kind=None,
            expected_error_fragment="does not support table freshness metadata",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_source_freshness_result_when_observing_then_raises_clear_error(
    test_case: SourceFreshnessObservationErrorTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        statement: str
        for statement in test_case.setup_sql:
            adapter.execute(connection=connection, sql=statement)
        source: SourceEntry = SourceEntry(
            name=test_case.source_name,
            table=test_case.table,
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy(test_case.strategy),
                value_kind=test_case.value_kind,
                column=test_case.column,
                query=test_case.query,
            ),
        )
        with pytest.raises(
            SourceFreshnessObservationError, match=test_case.expected_error_fragment
        ):
            observe_configured_source_freshness(
                dialect=Dialect(adapter),
                reads=live_reads(adapter=adapter, connection=connection),
                source=source,
                observed_at=datetime(2026, 1, 1, 12, 0, 0, tzinfo=UTC),
            )
    finally:
        adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        UnsupportedTableFreshnessMetadataGuardTestCase(
            description="unsupported adapter guard prevents metadata lookup",
            source_name="raw_orders",
            table="raw_orders",
            expected_error_fragment="does not support table freshness metadata",
            expected_metadata_requested=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_freshness_is_unsupported_when_observing_then_metadata_is_not_requested(
    test_case: UnsupportedTableFreshnessMetadataGuardTestCase,
) -> None:
    adapter: UnsupportedFreshnessMetadataDuckDbAdapter = UnsupportedFreshnessMetadataDuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        source: SourceEntry = SourceEntry(
            name=test_case.source_name,
            table=test_case.table,
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy.ADAPTER,
                value_kind=None,
            ),
        )

        with pytest.raises(
            SourceFreshnessObservationError, match=test_case.expected_error_fragment
        ):
            observe_configured_source_freshness(
                dialect=Dialect(adapter),
                reads=live_reads(adapter=adapter, connection=connection),
                source=source,
                observed_at=datetime(2026, 1, 1, 12, 0, 0, tzinfo=UTC),
            )
    finally:
        adapter.close(connection)

    assert adapter.metadata_requested is test_case.expected_metadata_requested


@pytest.mark.parametrize(
    "test_case",
    [
        SourceFreshnessStateErrorTestCase(
            description="rejects non datetime timestamp value",
            value_kind="timestamp",
            data_version="2026-01-01T12:00:00Z",
            expected_error_fragment="must be datetime values",
        ),
        SourceFreshnessStateErrorTestCase(
            description="rejects bool integer value",
            value_kind="integer",
            data_version=True,
            expected_error_fragment="must be integer values",
        ),
        SourceFreshnessStateErrorTestCase(
            description="rejects non integer value",
            value_kind="integer",
            data_version="123",
            expected_error_fragment="must be integer values",
        ),
        SourceFreshnessStateErrorTestCase(
            description="rejects non string value",
            value_kind="string",
            data_version=123,
            expected_error_fragment="must be string values",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_source_freshness_value_when_normalizing_then_raises_clear_error(
    test_case: SourceFreshnessStateErrorTestCase,
) -> None:
    with pytest.raises(SourceFreshnessObservationError, match=test_case.expected_error_fragment):
        normalize_source_freshness_data_version(
            value=test_case.data_version,
            value_kind=SourceFreshnessValueKind(test_case.value_kind),
        )
