from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone
from decimal import Decimal

import pytest

from sqlbuild._native import FunctionArgument, FunctionReturnColumn, InferredNullability
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    ExpressionInferenceProfile,
    QueryResult,
    RelationInfo,
    RowDiffColumnResult,
    RowDiffResult,
    RowDiffSampleCell,
    RowDiffSampleRow,
    SchemaDiffResult,
    TableFreshnessMetadata,
    TableFreshnessRequest,
)
from sqlbuild.adapter.contract.types import (
    CursorKind,
    FunctionNullabilityRule,
    TableFreshnessStatus,
)
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.bigquery.classes.bigquery_connection import _BigQueryConnection
from sqlbuild.adapters.bigquery.classes.bigquery_cursor import _BigQueryCursor
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.adapters.bigquery._test_types import (
    BigQueryConnectErrorTestCase,
    BigQueryDmlLifecycleTestCase,
    BigQueryExecutionErrorTestCase,
    BigQueryExpressionInferenceProfileTestCase,
    BigQueryKeyedCursorMergeTestCase,
    BigQueryLoaderDecimalRowTestCase,
    BigQueryMergeExclusionTestCase,
    BigQueryPruneSqlTestCase,
    BigQueryQueryTestCase,
    BigQueryRelationAgeMetadataTestCase,
    BigQueryRenderCloneTestCase,
    BigQueryRenderCursorBoundLiteralTestCase,
    BigQueryRenderDeleteInsertTestCase,
    BigQueryRenderDropViewTestCase,
    BigQueryRenderPythonFunctionTestCase,
    BigQueryRenderQualifiedNameTestCase,
    BigQueryRenderSchemaTestCase,
    BigQueryRenderTableFunctionTestCase,
    BigQueryRowDiffTestCase,
    BigQuerySampleRowsTestCase,
    BigQuerySchemaDiffTestCase,
    BigQuerySchemaExistsTestCase,
    BigQueryTableFreshnessBatchTestCase,
    BigQueryTableFreshnessWildcardTestCase,
)
from tests.unit.src.sqlbuild.adapters.bigquery.helpers import (
    FakeBigQueryBadRequest,
    FakeBigQueryClient,
    FakeBigQueryFailingClient,
    FakeBigQueryRows,
    build_fake_bigquery_schema_client,
    build_row_diff_execute,
    build_sample_rows_execute,
    fake_row_diff_describe_relation,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryExpressionInferenceProfileTestCase(
            description="returns BigQuery inference rules",
            expected_sql_analysis_dialect="bigquery",
            expected_identifier_limit=1024,
            expected_rule_results={
                "IF": InferredNullability.NON_NULL,
                "LOWER": InferredNullability.NON_NULL,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_adapter_when_getting_inference_profile_then_returns_expected_rules(
    test_case: BigQueryExpressionInferenceProfileTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    profile: ExpressionInferenceProfile = adapter.expression_inference_profile()

    assert profile.sql_analysis_dialect == test_case.expected_sql_analysis_dialect
    assert adapter.maximum_identifier_length() == test_case.expected_identifier_limit
    if_rule: FunctionNullabilityRule | None = profile.function_nullability_rule("IF")
    lower_rule: FunctionNullabilityRule | None = profile.function_nullability_rule("LOWER")
    assert if_rule is not None
    assert lower_rule is not None
    assert (
        if_rule(
            (
                InferredNullability.UNKNOWN,
                InferredNullability.NON_NULL,
                InferredNullability.NON_NULL,
            )
        )
        == test_case.expected_rule_results["IF"]
    )
    assert lower_rule((InferredNullability.NON_NULL,)) == test_case.expected_rule_results["LOWER"]


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryPruneSqlTestCase(
            description="renders fingerprint pruning with correlated exists",
            database="example-project",
            schema="analytics",
            retain_versions=5,
            expected_fragments=(
                "DELETE FROM `example-project.analytics._sqlbuild_fingerprints` "
                "AS target WHERE EXISTS",
                "ROW_NUMBER() OVER",
                "PARTITION BY node_type, node_name",
                "ORDER BY ts DESC, run_id DESC",
                "__sqlbuild_history_rank > 5",
                "target.node_type = stale.node_type",
                "target.node_name = stale.node_name",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_when_rendering_prune_then_bigquery_uses_history_rank(
    test_case: BigQueryPruneSqlTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    sql: str = adapter.render_prune_fingerprint_history_sql(
        database=test_case.database,
        schema=test_case.schema,
        retain_versions=test_case.retain_versions,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryPruneSqlTestCase(
            description="renders source freshness pruning with null-safe full identity",
            database="example-project",
            schema="analytics",
            retain_versions=3,
            expected_fragments=(
                "DELETE FROM `example-project.analytics._sqlbuild_source_freshness` "
                "AS target WHERE EXISTS",
                "ROW_NUMBER() OVER",
                "PARTITION BY source_name, target_database, target_schema, target_name",
                "ORDER BY observed_at DESC, run_id DESC",
                "__sqlbuild_history_rank > 3",
                "target.target_database IS NOT DISTINCT FROM stale.target_database",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_table_when_rendering_prune_then_bigquery_uses_history_rank(
    test_case: BigQueryPruneSqlTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    sql: str = adapter.render_prune_source_freshness_history_sql(
        database=test_case.database,
        schema=test_case.schema,
        retain_versions=test_case.retain_versions,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryTableFreshnessBatchTestCase(
            description="uses table storage metadata for multiple tables",
            location="US",
            expected_data_versions=(
                datetime(2026, 1, 2, 3, 4, 5),
                datetime(2026, 1, 3, 4, 5, 6),
            ),
            expected_query_fragments=(
                "FROM `example-project.region-us`.INFORMATION_SCHEMA.TABLE_STORAGE",
                "storage_last_modified_time",
                "ORDERS",
                "CUSTOMERS",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_physical_tables_when_getting_freshness_metadata_then_bigquery_uses_table_storage(
    test_case: BigQueryTableFreshnessBatchTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    requests: tuple[TableFreshnessRequest, ...] = (
        TableFreshnessRequest(database="example-project", schema="raw", name="ORDERS"),
        TableFreshnessRequest(database="example-project", schema="raw", name="CUSTOMERS"),
    )
    client: FakeBigQueryClient = FakeBigQueryClient(
        rows=FakeBigQueryRows(
            columns=("table_schema", "table_name", "storage_last_modified_time"),
            rows=(
                ("raw", "ORDERS", test_case.expected_data_versions[0]),
                ("raw", "CUSTOMERS", test_case.expected_data_versions[1]),
            ),
        )
    )
    connection: _BigQueryConnection = _BigQueryConnection(
        client=client,
        location=test_case.location,
    )

    metadata_by_request: dict[TableFreshnessRequest, TableFreshnessMetadata] = (
        adapter.get_tables_freshness_metadata(connection=connection, requests=requests)
    )

    assert (
        tuple(metadata_by_request[request].data_version for request in requests)
        == test_case.expected_data_versions
    )
    assert all(metadata.value_kind == "timestamp" for metadata in metadata_by_request.values())
    assert len(client.queries) == 1
    sql, location = client.queries[0]
    assert location == test_case.location
    for fragment in test_case.expected_query_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryTableFreshnessWildcardTestCase(
            description="reports wildcard table metadata freshness unavailable",
            table_name="events_*",
            expected_error_fragment="does not support wildcard tables",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_wildcard_table_when_getting_freshness_metadata_then_bigquery_reports_unavailable(
    test_case: BigQueryTableFreshnessWildcardTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    request: TableFreshnessRequest = TableFreshnessRequest(
        database="example-project",
        schema="raw",
        name=test_case.table_name,
    )
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryClient(),
        location="US",
    )

    metadata: TableFreshnessMetadata = adapter.get_tables_freshness_metadata(
        connection=connection, requests=(request,)
    )[request]

    assert metadata.status == TableFreshnessStatus.UNAVAILABLE
    assert test_case.expected_error_fragment in str(metadata.message)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryQueryTestCase(
            description="returns limited rows and truncation flag",
            sql="SELECT id, name FROM demo",
            limit=2,
            expected_columns=("id", "name"),
            expected_rows=((1, "a"), (2, "b")),
            expected_truncated=True,
            expected_row_count=2,
        ),
        BigQueryQueryTestCase(
            description="returns all rows when limit is none",
            sql="SELECT id, name FROM demo",
            limit=None,
            expected_columns=("id", "name"),
            expected_rows=((1, "a"), (2, "b"), (3, "c")),
            expected_truncated=False,
            expected_row_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_rows_when_querying_then_returns_normalized_result(
    test_case: BigQueryQueryTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryClient(
            rows=FakeBigQueryRows(
                columns=test_case.expected_columns,
                rows=((1, "a"), (2, "b"), (3, "c")),
            )
        ),
        location="europe-west2",
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-bigquery-query"), dispatcher_scope(dispatcher):
        result: QueryResult = adapter.query(
            connection=connection, sql=test_case.sql, limit=test_case.limit
        )

    assert result.columns == test_case.expected_columns
    assert result.rows == test_case.expected_rows
    assert result.truncated is test_case.expected_truncated
    assert connection.client.queries == [(test_case.sql, "europe-west2")]
    assert events[-1].event_type == "statement_completed"
    assert events[-1].payload["row_count"] == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryDmlLifecycleTestCase(
            description="connection INSERT reports affected rows",
            sql="INSERT INTO private_target SELECT 1",
            statement_type="INSERT",
            affected_rows=3,
            expected_affected_rows=3,
        ),
        BigQueryDmlLifecycleTestCase(
            description="connection UPDATE omits unknown affected rows",
            sql="UPDATE private_target SET value = 1",
            statement_type="UPDATE",
            affected_rows=None,
            expected_affected_rows=None,
        ),
        BigQueryDmlLifecycleTestCase(
            description="connection DML excludes boolean affected rows",
            sql="UPDATE private_target SET value = 2",
            statement_type="UPDATE",
            affected_rows=True,
            expected_affected_rows=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_dml_job_when_connection_executes_then_terminal_reports_safe_affected_rows(
    test_case: BigQueryDmlLifecycleTestCase,
) -> None:
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryClient(
            rows=FakeBigQueryRows(columns=(), rows=()),
            statement_type=test_case.statement_type,
            num_dml_affected_rows=test_case.affected_rows,
        ),
        location="europe-west2",
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-bigquery-dml"), dispatcher_scope(dispatcher):
        cursor: _BigQueryCursor = connection.execute(test_case.sql)

    assert cursor.fetchall() == []
    assert tuple(event.event_type for event in events) == (
        "statement_started",
        "statement_submitted",
        "statement_completed",
    )
    assert events[-1].payload["job_id"] == "job-current"
    assert events[-1].payload.get("affected_rows") == test_case.expected_affected_rows
    assert "row_count" not in events[-1].payload
    assert test_case.sql not in repr(events)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryDmlLifecycleTestCase(
            description="adapter INSERT query reports affected rows",
            sql="INSERT INTO private_target SELECT 1",
            statement_type="INSERT",
            affected_rows=7,
            expected_affected_rows=7,
        ),
        BigQueryDmlLifecycleTestCase(
            description="adapter UPDATE query omits unknown affected rows",
            sql="UPDATE private_target SET value = 1",
            statement_type="UPDATE",
            affected_rows=None,
            expected_affected_rows=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_dml_job_when_adapter_queries_then_result_and_terminal_are_preserved(
    test_case: BigQueryDmlLifecycleTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryClient(
            rows=FakeBigQueryRows(columns=(), rows=()),
            statement_type=test_case.statement_type,
            num_dml_affected_rows=test_case.affected_rows,
        ),
        location="europe-west2",
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-bigquery-dml-query"), dispatcher_scope(dispatcher):
        result: QueryResult = adapter.query(connection=connection, sql=test_case.sql, limit=None)

    assert result == QueryResult()
    assert tuple(event.event_type for event in events) == (
        "statement_started",
        "statement_submitted",
        "statement_completed",
    )
    assert events[-1].payload["job_id"] == "job-current"
    assert events[-1].payload.get("affected_rows") == test_case.expected_affected_rows
    assert "row_count" not in events[-1].payload
    assert test_case.sql not in repr(events)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderSchemaTestCase(
            description="includes configured location in create schema",
            database="example-project",
            schema="dev",
            location="europe-west2",
            expected_sql=(
                "CREATE SCHEMA IF NOT EXISTS `example-project.dev` OPTIONS(location = 'europe-west2')"
            ),
        ),
        BigQueryRenderSchemaTestCase(
            description="omits location when none configured",
            database="example-project",
            schema="dev",
            location=None,
            expected_sql="CREATE SCHEMA IF NOT EXISTS `example-project.dev`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_schema_target_when_rendering_then_returns_create_schema_sql(
    test_case: BigQueryRenderSchemaTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    adapter._location = test_case.location

    statements: tuple[str, ...] = adapter.render_create_schema(
        database=test_case.database,
        schema=test_case.schema,
    )

    assert statements == (test_case.expected_sql,)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderQualifiedNameTestCase(
            description="quotes three-part BigQuery relation path",
            database="example-project",
            schema="dev",
            name="stg_customers",
            expected_qualified_name="`example-project.dev.stg_customers`",
        ),
        BigQueryRenderQualifiedNameTestCase(
            description="quotes two-part BigQuery relation path",
            database=None,
            schema="dev",
            name="stg_customers",
            expected_qualified_name="`dev.stg_customers`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_relation_parts_when_rendering_then_quotes_qualified_name(
    test_case: BigQueryRenderQualifiedNameTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    qualified_name: str | None = adapter.render_qualified_name(
        database=test_case.database,
        schema=test_case.schema,
        name=test_case.name,
    )

    assert qualified_name == test_case.expected_qualified_name


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderPythonFunctionTestCase(
            description="renders Python UDF DDL with runtime entry point and packages",
            expected_sql=(
                "CREATE OR REPLACE FUNCTION `demo.udfs.is_positive_int`(a_string STRING)\n"
                "RETURNS INT64\n"
                "LANGUAGE python\n"
                "OPTIONS(\n"
                '  runtime_version = "python-3.11",\n'
                '  entry_point = "main",\n'
                "  packages = ['numpy', 'pandas==1.5.0']\n"
                ")\n"
                "AS r'''\n"
                "def main(a_string):\n"
                "    return 1 if a_string else 0\n"
                "'''"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_function_when_rendering_then_bigquery_returns_expected_ddl(
    test_case: BigQueryRenderPythonFunctionTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    statements: tuple[str, ...] = adapter.render_create_function(
        destination="demo.udfs.is_positive_int",
        arguments=(FunctionArgument(name="a_string", type="STRING"),),
        returns="INT64",
        body_sql="def main(a_string):\n    return 1 if a_string else 0",
        language=FunctionLanguage.PYTHON,
        runtime_version="3.11",
        entry_point="main",
        packages=("numpy", "pandas==1.5.0"),
    )

    assert statements == (test_case.expected_sql,)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderTableFunctionTestCase(
            description="renders table function DDL with inferred BigQuery return schema",
            expected_sql=(
                "CREATE OR REPLACE TABLE FUNCTION `demo.analytics.customer_orders`"
                "(p_customer_id INT64)\n"
                "AS (\nSELECT order_id FROM `demo.analytics.fact_orders`\n"
                "WHERE customer_id = p_customer_id\n)"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_when_rendering_then_bigquery_returns_expected_ddl(
    test_case: BigQueryRenderTableFunctionTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    statements: tuple[str, ...] = adapter.render_create_function(
        destination="demo.analytics.customer_orders",
        arguments=(FunctionArgument(name="p_customer_id", type="INT64"),),
        returns="TABLE",
        body_sql=(
            "SELECT order_id FROM `demo.analytics.fact_orders`\nWHERE customer_id = p_customer_id"
        ),
        return_columns=(FunctionReturnColumn(name="order_id", type="INT64"),),
    )

    assert statements == (test_case.expected_sql,)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderTableFunctionTestCase(
            description="quotes fully qualified function calls with hyphenated project ids",
            expected_sql=(
                "`project-d5f92072.europe_west2.customer_orders`(1)|"
                "`project-d5f92072.europe_west2.is_completed_order`(status)"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_function_call_when_rendering_then_quotes_qualified_target(
    test_case: BigQueryRenderTableFunctionTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    rendered_call: str = "|".join(
        (
            adapter.render_table_function_call(
                target="project-d5f92072.europe_west2.customer_orders",
                call_suffix_sql="(1)",
            ),
            adapter.render_udf_call(
                target="project-d5f92072.europe_west2.is_completed_order",
                call_suffix_sql="(status)",
            ),
        )
    )

    assert rendered_call == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySchemaExistsTestCase(
            description="returns true when dataset exists",
            missing_dataset=False,
            expected_exists=True,
        ),
        BigQuerySchemaExistsTestCase(
            description="returns false when dataset is missing",
            missing_dataset=True,
            expected_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_dataset_state_when_checking_schema_exists_then_returns_expected(
    test_case: BigQuerySchemaExistsTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    client: FakeBigQueryClient = build_fake_bigquery_schema_client(
        missing_dataset=test_case.missing_dataset
    )
    connection: _BigQueryConnection = _BigQueryConnection(client=client, location="europe-west2")

    exists: bool = adapter.schema_exists(
        connection=connection,
        database="example-project",
        schema="dev",
    )

    assert exists is test_case.expected_exists
    assert client.dataset_ids == ["example-project.dev"]


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryConnectErrorTestCase(
            description="requires explicit connection project",
            config={"location": "europe-west2"},
            expected_error_fragment="BigQuery connection requires non-empty 'project'",
        ),
        BigQueryConnectErrorTestCase(
            description="rejects blank connection project",
            config={"project": " ", "location": "europe-west2"},
            expected_error_fragment="BigQuery connection requires non-empty 'project'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_bigquery_project_when_connecting_then_raises_clear_error(
    test_case: BigQueryConnectErrorTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        adapter.connect(test_case.config)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryExecutionErrorTestCase(
            description="includes BigQuery error details from failed jobs",
            error_message="400 GET https://bigquery.googleapis.com/bigquery/v2/projects/demo",
            error_details=[{"message": "Unrecognized name: missing_column at [1:8]"}],
            expected_error_fragment="missing_column",
            expected_error_code="A104",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_job_failure_when_executing_then_includes_error_details(
    test_case: BigQueryExecutionErrorTestCase,
) -> None:
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryFailingClient(
            query_error=FakeBigQueryBadRequest(
                test_case.error_message,
                errors=test_case.error_details,
            )
        ),
        location="europe-west2",
    )
    adapter: BigQueryAdapter = BigQueryAdapter()

    with pytest.raises(AdapterUserError, match=test_case.expected_error_fragment) as error:
        adapter.execute(connection=connection, sql="SELECT missing_column")

    assert error.value.code == test_case.expected_error_code


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryExecutionErrorTestCase(
            description="query failure retains submitted BigQuery job ID",
            error_message="query result failed",
            error_details=[{"message": "query result failed"}],
            expected_error_fragment="query result failed",
            expected_error_code="A104",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_submitted_bigquery_job_when_query_result_fails_then_terminal_retains_job_id(
    test_case: BigQueryExecutionErrorTestCase,
) -> None:
    connection: _BigQueryConnection = _BigQueryConnection(
        client=FakeBigQueryFailingClient(
            query_error=FakeBigQueryBadRequest(
                test_case.error_message,
                errors=test_case.error_details,
            )
        ),
        location="europe-west2",
    )
    adapter: BigQueryAdapter = BigQueryAdapter()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-bigquery-query-failure"),
        dispatcher_scope(dispatcher),
        pytest.raises(FakeBigQueryBadRequest, match=test_case.expected_error_fragment),
    ):
        adapter.query(connection=connection, sql="SELECT missing_column", limit=None)

    assert tuple(event.event_type for event in events) == (
        "statement_started",
        "statement_submitted",
        "statement_failed",
    )
    assert events[-1].payload["job_id"] == "job-current"


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderCursorBoundLiteralTestCase(
            description="renders timestamp cursor bounds as typed literals",
            value="2024-01-15T00:00:00",
            cursor_type=CursorKind.TIMESTAMP,
            expected_literal="TIMESTAMP '2024-01-15T00:00:00'",
        ),
        BigQueryRenderCursorBoundLiteralTestCase(
            description="renders integer cursor bounds without quotes",
            value="42",
            cursor_type=CursorKind.INTEGER,
            expected_literal="42",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_bounds_when_rendering_then_bigquery_returns_expected_literal(
    test_case: BigQueryRenderCursorBoundLiteralTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    result: str = adapter.render_cursor_bound_literal(
        value=test_case.value, cursor_type=test_case.cursor_type
    )

    assert result == test_case.expected_literal


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderCloneTestCase(
            description="renders zero copy table clone by default",
            source="example-project.prod.fact_orders",
            target="example-project.dev.fact_orders",
            hard_copy=False,
            expected_statements=(
                "CREATE TABLE `example-project.dev.fact_orders` "
                "CLONE `example-project.prod.fact_orders`",
            ),
            expected_supports_zero_copy=True,
        ),
        BigQueryRenderCloneTestCase(
            description="renders CTAS when hard copy is requested",
            source="`example-project.prod.fact_orders`",
            target="`example-project.dev.fact_orders`",
            hard_copy=True,
            expected_statements=(
                "CREATE OR REPLACE TABLE `example-project.dev.fact_orders` AS "
                "SELECT * FROM `example-project.prod.fact_orders`",
            ),
            expected_supports_zero_copy=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_clone_request_when_rendering_then_bigquery_uses_expected_clone_sql(
    test_case: BigQueryRenderCloneTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    statements: tuple[str, ...] = adapter.render_clone(
        origin=test_case.source,
        destination=test_case.target,
        hard_copy=test_case.hard_copy,
    )

    assert adapter.supports_zero_copy_clone() is test_case.expected_supports_zero_copy
    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderDropViewTestCase(
            description="quotes hyphenated project id when dropping view",
            target="test-project.dataset.__sqb_123456789abc__model__stg_events",
            expected_statements=(
                "DROP VIEW IF EXISTS `test-project.dataset.__sqb_123456789abc__model__stg_events`",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_adapter_when_rendering_drop_view_then_quotes_target(
    test_case: BigQueryRenderDropViewTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    result: tuple[str, ...] = adapter.render_drop_view(destination=test_case.target)

    assert result == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRenderDeleteInsertTestCase(
            description="renders cursor delete insert as merge replace window",
            expected_fragments=(
                "MERGE `example-project.dev.events` AS __target",
                "USING (SELECT id, event_time FROM delta_events) AS __source ON FALSE",
                "WHEN NOT MATCHED BY TARGET THEN INSERT (`id`, `event_time`)",
                "WHEN NOT MATCHED BY SOURCE AND __target.`event_time` >= "
                "TIMESTAMP '2026-01-01T00:00:00'",
                "AND __target.`event_time` < TIMESTAMP '2026-01-02T00:00:00' THEN DELETE",
            ),
            unexpected_fragments=(
                "DELETE FROM",
                "INSERT INTO",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_delete_insert_cursor_when_rendering_then_bigquery_uses_merge(
    test_case: BigQueryRenderDeleteInsertTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    statements: tuple[str, ...] = adapter.render_delete_insert_cursor(
        destination="example-project.dev.events",
        sql="SELECT id, event_time FROM delta_events",
        cursor_column="event_time",
        cursor_start="2026-01-01T00:00:00",
        cursor_end="2026-01-02T00:00:00",
        columns=("id", "event_time"),
    )
    rendered_sql: str = statements[0]

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in rendered_sql

    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in rendered_sql


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryKeyedCursorMergeTestCase(
            description="one merge replaces the delta keys and deletes the rest of the window",
            unique_key=("order_id",),
            columns=("order_id", "status", "updated_at"),
            expected_statements=(
                "MERGE INTO `example-project.dev.orders` AS __target USING "
                "(SELECT order_id, status, updated_at FROM delta_orders) AS __source "
                "ON __target.`order_id` = __source.`order_id` "
                "WHEN MATCHED THEN UPDATE SET `status` = __source.`status`, "
                "`updated_at` = __source.`updated_at` "
                "WHEN NOT MATCHED THEN INSERT (`order_id`, `status`, `updated_at`) "
                "VALUES (__source.`order_id`, __source.`status`, __source.`updated_at`) "
                "WHEN NOT MATCHED BY SOURCE AND __target.`updated_at` >= "
                "TIMESTAMP '2026-01-01T00:00:00' AND __target.`updated_at` < "
                "TIMESTAMP '2026-01-02T00:00:00' THEN DELETE",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_keyed_cursor_window_when_rendering_then_bigquery_replaces_it_in_one_merge(
    test_case: BigQueryKeyedCursorMergeTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    statements: tuple[str, ...] = adapter.render_delete_insert_cursor(
        destination="example-project.dev.orders",
        sql="SELECT order_id, status, updated_at FROM delta_orders",
        cursor_column="updated_at",
        cursor_start="2026-01-01T00:00:00",
        cursor_end="2026-01-02T00:00:00",
        columns=test_case.columns,
        unique_key=test_case.unique_key,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryMergeExclusionTestCase(
            description="bigquery selective merge",
            expected_update_assignment="`status` = __source.`status`",
            expected_insert_clause=(
                " (`tenant_id`, `order_id`, `status`, `updated_at`) VALUES "
                "(__source.`tenant_id`, __source.`order_id`, __source.`status`, "
                "__source.`updated_at`)"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_composite_key_and_case_different_exclusion_when_rendering_merge_then_updates_only_mutable_columns(  # noqa: E501
    test_case: BigQueryMergeExclusionTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    rendered_sql: str = adapter.render_merge(
        destination="`example-project.dev.orders`",
        sql="SELECT tenant_id, order_id, status, updated_at FROM delta_orders",
        unique_key=("tenant_id", "order_id"),
        source_columns=("tenant_id", "order_id", "status", "updated_at"),
        exclude_columns=("UPDATED_AT",),
    )[0]
    update_clause, insert_clause = rendered_sql.split("WHEN NOT MATCHED THEN INSERT", maxsplit=1)
    update_assignments: str = update_clause.split("WHEN MATCHED THEN UPDATE SET", maxsplit=1)[1]

    assert "`tenant_id` = __source.`tenant_id`" not in update_assignments
    assert "`order_id` = __source.`order_id`" not in update_assignments
    assert "`updated_at` = __source.`updated_at`" not in update_assignments
    assert test_case.expected_update_assignment in update_assignments
    assert insert_clause == test_case.expected_insert_clause


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySchemaDiffTestCase(
            description="detects added removed and changed column types",
            expected_result=SchemaDiffResult(
                added_columns=(ColumnInfo(name="new_col", type="DATE"),),
                removed_columns=(ColumnInfo(name="status", type="STRING"),),
                type_changed_columns=(
                    (ColumnInfo(name="id", type="INT64"), ColumnInfo(name="id", type="STRING")),
                ),
            ),
            relation_columns={
                "left_relation": (
                    ColumnInfo(name="id", type="INT64"),
                    ColumnInfo(name="status", type="STRING"),
                ),
                "right_relation": (
                    ColumnInfo(name="id", type="STRING"),
                    ColumnInfo(name="new_col", type="DATE"),
                ),
            },
        ),
        BigQuerySchemaDiffTestCase(
            description="ignores semantically equivalent type aliases",
            expected_result=SchemaDiffResult(),
            relation_columns={
                "left_relation": (ColumnInfo(name="id", type="INT64"),),
                "right_relation": (ColumnInfo(name="id", type="INTEGER"),),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_relations_when_diffing_schema_then_returns_expected_result(
    test_case: BigQuerySchemaDiffTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    monkeypatch.setattr(
        adapter,
        "describe_relation",
        lambda connection, relation: test_case.relation_columns[relation],
    )

    result: SchemaDiffResult = adapter.diff_schema(
        connection=object(),
        left="left_relation",
        right="right_relation",
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRowDiffTestCase(
            description="returns row diff counts and column mismatch counts",
            expected_result=RowDiffResult(
                left_count=3,
                right_count=3,
                joined_count=4,
                equal_count=1,
                unequal_count=1,
                left_only_count=1,
                right_only_count=1,
                column_results=(
                    RowDiffColumnResult(name="val", mismatched_count=1, tolerance=None),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_relations_when_diffing_rows_then_returns_expected_result(
    test_case: BigQueryRowDiffTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()
    executed_sql: list[str] = []

    monkeypatch.setattr(adapter, "describe_relation", fake_row_diff_describe_relation)
    monkeypatch.setattr(adapter, "execute", build_row_diff_execute(executed_sql))

    result: RowDiffResult = adapter.diff_rows(
        connection=object(),
        left="left_relation",
        right="right_relation",
        unique_key="id",
    )

    assert result == test_case.expected_result
    assert any("FULL OUTER JOIN" in sql for sql in executed_sql)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySampleRowsTestCase(
            description="returns unequal and side-only samples",
            expected_unequal_samples=(
                RowDiffSampleRow(
                    key_values=(("id", 1),),
                    changed_cells=(RowDiffSampleCell(name="val", left_value="a", right_value="x"),),
                ),
            ),
            expected_side_only_samples=((("id", 1),),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_relations_when_sampling_rows_then_returns_expected_examples(
    test_case: BigQuerySampleRowsTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    monkeypatch.setattr(adapter, "describe_relation", fake_row_diff_describe_relation)
    monkeypatch.setattr(adapter, "execute", build_sample_rows_execute())

    unequal_samples: tuple[RowDiffSampleRow, ...] = adapter.sample_unequal_rows(
        connection=object(),
        left="left_relation",
        right="right_relation",
        unique_key="id",
        limit=5,
    )
    side_only_samples: tuple[tuple[tuple[str, object], ...], ...] = adapter.sample_side_only_rows(
        connection=object(),
        left="left_relation",
        right="right_relation",
        unique_key="id",
        side="left",
        limit=5,
    )

    assert unequal_samples == test_case.expected_unequal_samples
    assert side_only_samples == test_case.expected_side_only_samples


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRelationAgeMetadataTestCase(
            description="dataset metadata supplies UTC creation and last-modified times",
            metadata_rows=(
                (
                    "old_orders",
                    datetime(2026, 8, 1, 3, 0, tzinfo=timezone(timedelta(hours=-7))),
                    datetime(2026, 8, 2, 10, 30, tzinfo=UTC),
                ),
                ("old_orders_view", datetime(2026, 8, 3, 4, 0), None),
            ),
            expected_timestamps=(
                ("2026-08-01 10:00:00+00:00", "2026-08-02 10:30:00+00:00"),
                ("2026-08-03 04:00:00+00:00", "None"),
            ),
        ),
        BigQueryRelationAgeMetadataTestCase(
            description="relations missing from dataset metadata stay unknown",
            metadata_rows=(),
            expected_timestamps=(("None", "None"), ("None", "None")),
        ),
        BigQueryRelationAgeMetadataTestCase(
            description="case-distinct tables keep their own timestamps",
            metadata_rows=(
                (
                    "Orders",
                    datetime(2025, 1, 1, tzinfo=UTC),
                    datetime(2025, 1, 2, tzinfo=UTC),
                ),
                (
                    "orders",
                    datetime(2026, 8, 1, tzinfo=UTC),
                    datetime(2026, 8, 2, tzinfo=UTC),
                ),
            ),
            expected_timestamps=(
                ("2026-08-01 00:00:00+00:00", "2026-08-02 00:00:00+00:00"),
                ("2025-01-01 00:00:00+00:00", "2025-01-02 00:00:00+00:00"),
                ("None", "None"),
            ),
            relation_names=("orders", "Orders", "ORDERS"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_listed_relations_when_adding_age_metadata_then_bigquery_reads_each_dataset_once(
    test_case: BigQueryRelationAgeMetadataTestCase,
) -> None:
    client: FakeBigQueryClient = FakeBigQueryClient(
        rows=FakeBigQueryRows(
            columns=("table_id", "creation_time", "last_modified_time"),
            rows=test_case.metadata_rows,
        )
    )
    relations: tuple[RelationInfo, ...] = tuple(
        RelationInfo(
            database="example-project",
            schema="dev_orders",
            name=name,
            relation_type="table",
        )
        for name in test_case.relation_names
    )

    enriched: tuple[RelationInfo, ...] = BigQueryAdapter().with_relation_age_metadata(
        connection=_BigQueryConnection(client=client, location=None),
        relations=relations,
    )

    assert (
        tuple((str(relation.created_at), str(relation.last_altered_at)) for relation in enriched)
        == test_case.expected_timestamps
    )
    assert tuple(sql for sql, _ in client.queries) == (
        "SELECT table_id, TIMESTAMP_MILLIS(creation_time), "
        "TIMESTAMP_MILLIS(last_modified_time) FROM `example-project.dev_orders.__TABLES__`",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryLoaderDecimalRowTestCase(
            description="a wide decimal in exponent form casts exactly to bignumeric",
            value=Decimal("1.23456789012E-2"),
            column_type="BIGNUMERIC(40, 13)",
            expected_sql="SELECT CAST('0.0123456789012' AS BIGNUMERIC) AS `amount`",
        ),
        BigQueryLoaderDecimalRowTestCase(
            description="a numeric decimal casts from a string, not a float literal",
            value=Decimal("12.50"),
            column_type="NUMERIC(4, 2)",
            expected_sql="SELECT CAST('12.50' AS NUMERIC) AS `amount`",
        ),
        BigQueryLoaderDecimalRowTestCase(
            description="a decimal in a declared integer column stays a numeric literal",
            value=Decimal("3.00"),
            column_type="INT64",
            inferred_types=(),
            expected_sql="SELECT CAST(3.00 AS INT64) AS `amount`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_decimal_loader_value_when_rendering_rows_then_bigquery_casts_it_exactly(
    test_case: BigQueryLoaderDecimalRowTestCase,
) -> None:
    adapter: BigQueryAdapter = BigQueryAdapter()

    rendered_sql: str = adapter.render_loader_rows_select(
        rows=({"amount": test_case.value},),
        column_names=("amount",),
        column_sql_types={"amount": test_case.column_type},
        inferred_types=dict(test_case.inferred_types),
    )

    assert rendered_sql == test_case.expected_sql
