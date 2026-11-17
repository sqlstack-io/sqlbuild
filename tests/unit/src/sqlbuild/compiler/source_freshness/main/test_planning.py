from __future__ import annotations

from collections.abc import Callable
from datetime import UTC, datetime
from typing import Any

import pytest

from sqlbuild._native import (
    Dialect,
    DirectSourceFreshnessPlanningResult,
    SourceEntry,
    SourceFreshnessAgePolicy,
    SourceFreshnessConfig,
    SourceFreshnessRecord,
    SourceFreshnessRenderers,
    SourceFreshnessStrategy,
    SourceFreshnessValueKind,
    build_direct_source_freshness_planning_result,
    live_reads,
    source_freshness_data_version_hash,
    write_source_freshness_records,
)
from sqlbuild.adapter.contract.models import TableFreshnessMetadata, TableFreshnessRequest
from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.source_freshness.main._test_types import (
    DirectSourceFreshnessAdapterDefaultTestCase,
    DirectSourceFreshnessAgePolicyTestCase,
    DirectSourceFreshnessDuplicateSchemaTestCase,
    DirectSourceFreshnessExpressionTestCase,
    DirectSourceFreshnessLagToleranceTestCase,
    DirectSourceFreshnessManagedSkipTestCase,
    DirectSourceFreshnessMultiSchemaTestCase,
    DirectSourceFreshnessPlanningTestCase,
    DirectSourceFreshnessUnknownTestCase,
)
from tests.unit.src.sqlbuild.compiler.source_freshness.main.helpers import (
    state_table_exists_map,
    write_optional_previous_record,
    write_previous_record_to_schema,
)

RENDER_QUALIFIED_NAME: Callable[..., str | None] = DuckDbAdapter().render_qualified_name
RENDER_FRAMEWORK_TYPE: Callable[[FrameworkType], str] = DuckDbAdapter().render_framework_type


class FreshnessMetadataDuckDbAdapter(DuckDbAdapter):
    def __init__(self, *, data_version: object = 42, value_kind: str = "integer") -> None:
        super().__init__()
        self.data_version: object = data_version
        self.value_kind: str = value_kind
        self.batch_requests: list[tuple[TableFreshnessRequest, ...]] = []

    def supports_table_freshness_metadata(self) -> bool:
        return True

    def get_table_freshness_metadata(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        return TableFreshnessMetadata(
            data_version=self.data_version,
            value_kind=self.value_kind,
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
        )

    def get_tables_freshness_metadata(
        self,
        connection: Any,
        *,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        del connection
        self.batch_requests.append(requests)
        return {
            request: TableFreshnessMetadata(
                data_version=self.data_version,
                value_kind=self.value_kind,
                observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
            )
            for request in requests
        }


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessPlanningTestCase(
            description="classifies first direct source freshness observation as changed",
            previous_data_version=None,
            current_query="SELECT 1 AS data_version",
            expected_changed_count=1,
            expected_unchanged_count=0,
        ),
        DirectSourceFreshnessPlanningTestCase(
            description="classifies matching previous direct source freshness as unchanged",
            previous_data_version="1",
            current_query="SELECT 1 AS data_version",
            expected_changed_count=0,
            expected_unchanged_count=1,
        ),
        DirectSourceFreshnessPlanningTestCase(
            description="classifies different previous direct source freshness as changed",
            previous_data_version="1",
            current_query="SELECT 2 AS data_version",
            expected_changed_count=1,
            expected_unchanged_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_direct_source_freshness_state_when_planning_then_classifies_hash_comparison(
    test_case: DirectSourceFreshnessPlanningTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        connection.execute("CREATE SCHEMA state_schema")
        source: SourceEntry = SourceEntry(
            name="raw.orders",
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy.SQL,
                value_kind=SourceFreshnessValueKind.INTEGER,
                query=test_case.current_query,
            ),
        )
        write_optional_previous_record(
            adapter=adapter,
            connection=connection,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
            data_version=test_case.previous_data_version,
        )

        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(source,),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.changed_identities) == test_case.expected_changed_count
    assert len(result.unchanged_identities) == test_case.expected_unchanged_count
    assert len(result.observed_records) == 1


@pytest.mark.parametrize(
    "test_case",
    (
        DirectSourceFreshnessAgePolicyTestCase(
            description="fresh timestamp passes age policy",
            current_query="SELECT CAST('2026-01-15 11:30:00' AS TIMESTAMP) AS data_version",
            warn_after="1h",
            error_after="2h",
            value_kind=SourceFreshnessValueKind.TIMESTAMP,
            expected_age_status="pass",
        ),
        DirectSourceFreshnessAgePolicyTestCase(
            description="older timestamp warns before error threshold",
            current_query="SELECT CAST('2026-01-15 10:30:00' AS TIMESTAMP) AS data_version",
            warn_after="1h",
            error_after="2h",
            value_kind=SourceFreshnessValueKind.TIMESTAMP,
            expected_age_status="warn",
        ),
        DirectSourceFreshnessAgePolicyTestCase(
            description="old timestamp errors after error threshold",
            current_query="SELECT CAST('2026-01-15 09:30:00' AS TIMESTAMP) AS data_version",
            warn_after="1h",
            error_after="2h",
            value_kind=SourceFreshnessValueKind.TIMESTAMP,
            expected_age_status="error",
        ),
        DirectSourceFreshnessAgePolicyTestCase(
            description="naive timestamp compares against aware observed timestamp",
            current_query="SELECT CAST('2026-01-15 09:30:00' AS TIMESTAMP) AS data_version",
            warn_after="1h",
            error_after="2h",
            value_kind=SourceFreshnessValueKind.TIMESTAMP,
            expected_age_status="error",
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
        ),
        DirectSourceFreshnessAgePolicyTestCase(
            description="non timestamp observation is unknown for age policy",
            current_query="SELECT 42 AS data_version",
            warn_after="1h",
            error_after="2h",
            value_kind=SourceFreshnessValueKind.INTEGER,
            expected_age_status="unknown",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_source_freshness_age_policy_when_planning_then_records_age_status(
    test_case: DirectSourceFreshnessAgePolicyTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        connection.execute("CREATE SCHEMA state_schema")
        source: SourceEntry = SourceEntry(
            name="raw.orders",
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy.SQL,
                value_kind=test_case.value_kind,
                query=test_case.current_query,
                age_policy=SourceFreshnessAgePolicy(
                    warn_after=test_case.warn_after,
                    error_after=test_case.error_after,
                ),
            ),
        )

        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(source,),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=test_case.observed_at,
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.age_statuses) == 1
    assert next(iter(result.age_statuses.values())) == test_case.expected_age_status


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessAgePolicyTestCase(
            description="adapter metadata timestamp can error age policy",
            current_query="",
            warn_after="1h",
            error_after="2h",
            expected_age_status="error",
            value_kind=SourceFreshnessValueKind.TIMESTAMP,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_metadata_age_policy_when_planning_then_records_age_status(
    test_case: DirectSourceFreshnessAgePolicyTestCase,
) -> None:
    adapter: FreshnessMetadataDuckDbAdapter = FreshnessMetadataDuckDbAdapter(
        data_version=datetime(2026, 1, 15, 9, 30, 0),
        value_kind="timestamp",
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        connection.execute("CREATE SCHEMA state_schema")
        source: SourceEntry = SourceEntry(
            name="raw.orders",
            schema="raw",
            table="orders",
            freshness=SourceFreshnessConfig(
                strategy=SourceFreshnessStrategy.ADAPTER,
                age_policy=SourceFreshnessAgePolicy(
                    warn_after=test_case.warn_after,
                    error_after=test_case.error_after,
                ),
            ),
        )

        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(source,),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.age_statuses) == 1
    assert next(iter(result.age_statuses.values())) == test_case.expected_age_status


@pytest.mark.parametrize(
    "test_case",
    (
        DirectSourceFreshnessLagToleranceTestCase(
            description="within timestamp lag tolerance",
            current_query="SELECT CAST('2026-01-15 12:05:00' AS TIMESTAMP) AS data_version",
            expected_changed_count=0,
            expected_unchanged_count=1,
        ),
        DirectSourceFreshnessLagToleranceTestCase(
            description="exactly at timestamp lag tolerance boundary",
            current_query="SELECT CAST('2026-01-15 12:10:00' AS TIMESTAMP) AS data_version",
            expected_changed_count=0,
            expected_unchanged_count=1,
        ),
        DirectSourceFreshnessLagToleranceTestCase(
            description="beyond timestamp lag tolerance",
            current_query="SELECT CAST('2026-01-15 12:11:00' AS TIMESTAMP) AS data_version",
            expected_changed_count=1,
            expected_unchanged_count=0,
        ),
        DirectSourceFreshnessLagToleranceTestCase(
            description="backwards timestamp movement is conservative",
            current_query="SELECT CAST('2026-01-15 11:59:00' AS TIMESTAMP) AS data_version",
            expected_changed_count=1,
            expected_unchanged_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_timestamp_lag_tolerance_when_planning_then_classifies_tolerated_movement(
    test_case: DirectSourceFreshnessLagToleranceTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    previous_data_version: str = "2026-01-15T12:00:00"
    try:
        connection.execute("CREATE SCHEMA state_schema")
        write_source_freshness_records(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema="state_schema",
            records=(
                SourceFreshnessRecord(
                    source_name="raw.orders",
                    target_database=None,
                    target_schema=None,
                    target_name=None,
                    run_id="previous",
                    strategy=SourceFreshnessStrategy.SQL.value,
                    value_kind=SourceFreshnessValueKind.TIMESTAMP.value,
                    data_version=previous_data_version,
                    data_version_hash=source_freshness_data_version_hash(
                        source_name="raw.orders",
                        strategy=SourceFreshnessStrategy.SQL,
                        value_kind=SourceFreshnessValueKind.TIMESTAMP,
                        data_version=previous_data_version,
                    ),
                    observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
                ),
            ),
            renderers=SourceFreshnessRenderers(
                render_qualified_name=RENDER_QUALIFIED_NAME,
                render_framework_type=RENDER_FRAMEWORK_TYPE,
                render_insert_records_sql=adapter.render_insert_source_freshness_records_sql,
            ),
        )

        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(
                SourceEntry(
                    name="raw.orders",
                    freshness=SourceFreshnessConfig(
                        strategy=SourceFreshnessStrategy.SQL,
                        value_kind=SourceFreshnessValueKind.TIMESTAMP,
                        query=test_case.current_query,
                        lag_tolerance="10m",
                    ),
                ),
            ),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 30, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.changed_identities) == test_case.expected_changed_count
    assert len(result.unchanged_identities) == test_case.expected_unchanged_count


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessUnknownTestCase(
            description="classifies unsupported default source freshness as unknown",
            expected_unknown_source_names=("raw.orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unconfigured_source_without_adapter_metadata_when_planning_then_marks_unknown(
    test_case: DirectSourceFreshnessUnknownTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(SourceEntry(name="raw.orders", table="orders"),),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert result.unknown_source_names == test_case.expected_unknown_source_names
    assert result.observed_records == ()


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessExpressionTestCase(
            description="column freshness observes an expression source via subquery",
            expression="SELECT 1 AS id, 7 AS batch_id",
            column="batch_id",
            expected_data_version="7",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_column_freshness_expression_when_planning_then_observes_subquery(
    test_case: DirectSourceFreshnessExpressionTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(
                SourceEntry(
                    name="raw_orders",
                    expression=test_case.expression,
                    freshness=SourceFreshnessConfig(
                        strategy=SourceFreshnessStrategy.COLUMN,
                        value_kind=SourceFreshnessValueKind.INTEGER,
                        column=test_case.column,
                    ),
                ),
            ),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert result.unknown_source_names == ()
    assert len(result.observed_records) == 1
    assert result.observed_records[0].data_version == test_case.expected_data_version


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessAdapterDefaultTestCase(
            description="adapter default freshness observes unconfigured physical sources in batch",
            expected_changed_count=2,
            expected_observed_count=2,
            expected_batch_call_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_metadata_support_when_planning_unconfigured_source_then_observes_default(
    test_case: DirectSourceFreshnessAdapterDefaultTestCase,
) -> None:
    adapter: FreshnessMetadataDuckDbAdapter = FreshnessMetadataDuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(
                SourceEntry(name="raw.orders", schema="raw", table="orders"),
                SourceEntry(name="raw.customers", schema="raw", table="customers"),
            ),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.changed_identities) == test_case.expected_changed_count
    assert len(result.observed_records) == test_case.expected_observed_count
    assert len(adapter.batch_requests) == test_case.expected_batch_call_count
    assert tuple(request.name for request in adapter.batch_requests[0]) == ("orders", "customers")


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessManagedSkipTestCase(
            description="managed sources are skipped during direct planning observation",
            expected_observed_count=0,
            expected_unknown_source_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_managed_source_when_planning_source_freshness_then_skips_observation(
    test_case: DirectSourceFreshnessManagedSkipTestCase,
) -> None:
    adapter: FreshnessMetadataDuckDbAdapter = FreshnessMetadataDuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(SourceEntry(name="raw.orders", schema="raw", table="orders", managed=True),),
            state_database=None,
            state_schemas=("state_schema",),
            observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_schema",),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.observed_records) == test_case.expected_observed_count
    assert result.unknown_source_names == test_case.expected_unknown_source_names


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessMultiSchemaTestCase(
            description="reads previous direct source freshness across multiple state schemas",
            expected_previous_count=2,
            expected_unchanged_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_state_schemas_when_planning_then_merges_previous_records(
    test_case: DirectSourceFreshnessMultiSchemaTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        connection.execute("CREATE SCHEMA state_a")
        connection.execute("CREATE SCHEMA state_b")
        write_previous_record_to_schema(
            adapter=adapter,
            connection=connection,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
            schema="state_a",
            source_name="raw.orders",
            data_version="1",
        )
        write_previous_record_to_schema(
            adapter=adapter,
            connection=connection,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
            schema="state_b",
            source_name="raw.customers",
            data_version="2",
        )
        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(
                SourceEntry(
                    name="raw.orders",
                    freshness=SourceFreshnessConfig(
                        strategy=SourceFreshnessStrategy.SQL,
                        value_kind=SourceFreshnessValueKind.INTEGER,
                        query="SELECT 1 AS data_version",
                    ),
                ),
            ),
            state_database=None,
            state_schemas=("state_a", "state_b"),
            observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_a", "state_b"),
            ),
        )
    finally:
        adapter.close(connection)

    assert len(result.previous_records) == test_case.expected_previous_count
    assert len(result.unchanged_identities) == test_case.expected_unchanged_count


@pytest.mark.parametrize(
    "test_case",
    [
        DirectSourceFreshnessDuplicateSchemaTestCase(
            description="uses newest duplicate source freshness record across state schemas",
            expected_previous_data_version="2",
            expected_changed_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duplicate_state_schema_records_when_planning_then_uses_newest_observation(
    test_case: DirectSourceFreshnessDuplicateSchemaTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    try:
        connection.execute("CREATE SCHEMA state_a")
        connection.execute("CREATE SCHEMA state_b")
        write_previous_record_to_schema(
            adapter=adapter,
            connection=connection,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
            schema="state_a",
            source_name="raw.orders",
            data_version="2",
            observed_at=datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC),
        )
        write_previous_record_to_schema(
            adapter=adapter,
            connection=connection,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
            schema="state_b",
            source_name="raw.orders",
            data_version="1",
            observed_at=datetime(2026, 1, 15, 10, 0, 0, tzinfo=UTC),
        )

        result: DirectSourceFreshnessPlanningResult = build_direct_source_freshness_planning_result(
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection),
            sources=(
                SourceEntry(
                    name="raw.orders",
                    freshness=SourceFreshnessConfig(
                        strategy=SourceFreshnessStrategy.SQL,
                        value_kind=SourceFreshnessValueKind.INTEGER,
                        query="SELECT 2 AS data_version",
                    ),
                ),
            ),
            state_database=None,
            state_schemas=("state_a", "state_b"),
            observed_at=datetime(2026, 1, 15, 12, 5, 0, tzinfo=UTC),
            run_id="planning",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            state_table_exists_by_schema=state_table_exists_map(
                adapter=adapter,
                connection=connection,
                state_database=None,
                state_schemas=("state_a", "state_b"),
            ),
        )
    finally:
        adapter.close(connection)

    assert result.previous_records[0].data_version == test_case.expected_previous_data_version
    assert len(result.changed_identities) == test_case.expected_changed_count
