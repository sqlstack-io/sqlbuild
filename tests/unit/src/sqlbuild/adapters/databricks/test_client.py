from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone
from typing import Any, cast

import pytest

from sqlbuild._native import FunctionArgument, FunctionReturnColumn, InferredNullability
from sqlbuild.adapter.contract.models import (
    ExpressionInferenceProfile,
    RelationInfo,
    TableFreshnessMetadata,
    TableFreshnessRequest,
    TableFreshnessStatus,
)
from sqlbuild.adapter.contract.types import FunctionNullabilityRule, LoaderLogicalType
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.compiler.compile.types import FunctionLanguage
from tests.unit.src.sqlbuild.adapters.databricks._test_types import (
    DatabricksDeltaDetailAgeTestCase,
    DatabricksExpressionInferenceProfileTestCase,
    DatabricksMergeExclusionTestCase,
    DatabricksPruneSqlTestCase,
    DatabricksPythonFunctionSupportTestCase,
    DatabricksRelationAgeMetadataTestCase,
    DatabricksRenderCloneTestCase,
    DatabricksRenderDeleteInsertCursorTestCase,
    DatabricksRenderDurableCloneTestCase,
    DatabricksRenderPythonFunctionTestCase,
    DatabricksRenderTableFunctionTestCase,
    DatabricksSchemaExistsTestCase,
    DatabricksStringTypeCastRenderingTestCase,
    DatabricksTableFreshnessBatchTestCase,
    DatabricksTableFreshnessMixedBatchTestCase,
)
from tests.unit.src.sqlbuild.adapters.databricks.helpers import (
    FailingDatabricksMetadataCursor,
    FakeDatabricksMetadataConnection,
    FakeDatabricksMetadataCursor,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksExpressionInferenceProfileTestCase(
            description="returns Databricks inference rules",
            expected_sql_analysis_dialect="databricks",
            expected_identifier_limit=255,
            expected_rule_results={
                "IF": InferredNullability.NON_NULL,
                "LOWER": InferredNullability.NON_NULL,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_databricks_adapter_when_getting_inference_profile_then_returns_expected_rules(
    test_case: DatabricksExpressionInferenceProfileTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

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
        DatabricksStringTypeCastRenderingTestCase(
            description="normalizes varchar casts to string",
            declared_type="VARCHAR",
            expected_loader_fragment="CAST(`status` AS STRING) AS `status`",
            expected_source_cast="CAST(status AS STRING) AS status",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_databricks_string_declared_type_when_rendering_casts_then_uses_string(
    test_case: DatabricksStringTypeCastRenderingTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    loader_sql: str = adapter.render_loader_rows_select(
        rows=({"status": "placed"},),
        column_names=("status",),
        column_sql_types={"status": test_case.declared_type},
        inferred_types={"status": LoaderLogicalType.STRING},
    )
    source_cast_sql: str = adapter.render_source_expression_cast(
        expression="status",
        target_type=test_case.declared_type,
        alias="status",
    )

    assert test_case.expected_loader_fragment in loader_sql
    assert test_case.declared_type not in loader_sql
    assert source_cast_sql == test_case.expected_source_cast


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksPruneSqlTestCase(
            description="renders fingerprint pruning with correlated exists",
            database="workspace",
            schema="analytics",
            retain_versions=5,
            expected_fragments=(
                "DELETE FROM `workspace`.`analytics`.`_sqlbuild_fingerprints` "
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
def test_given_fingerprint_table_when_rendering_prune_then_databricks_uses_history_rank(
    test_case: DatabricksPruneSqlTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

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
        DatabricksPruneSqlTestCase(
            description="renders source freshness pruning with null-safe full identity",
            database="workspace",
            schema="analytics",
            retain_versions=3,
            expected_fragments=(
                "DELETE FROM `workspace`.`analytics`.`_sqlbuild_source_freshness` "
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
def test_given_source_freshness_table_when_rendering_prune_then_databricks_uses_history_rank(
    test_case: DatabricksPruneSqlTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

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
        DatabricksTableFreshnessBatchTestCase(
            description="uses delta history timestamps for multiple tables",
            expected_data_versions=(
                datetime(2026, 1, 2, 3, 4, 5),
                datetime(2026, 1, 3, 4, 5, 6),
            ),
            expected_query_fragments=(
                "DESCRIBE HISTORY `main`.`raw`.`orders`",
                "UNION ALL",
                "max(timestamp) AS last_modified",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_physical_tables_when_getting_freshness_metadata_then_databricks_uses_delta_history(
    test_case: DatabricksTableFreshnessBatchTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()
    requests: tuple[TableFreshnessRequest, ...] = (
        TableFreshnessRequest(database="main", schema="raw", name="orders"),
        TableFreshnessRequest(database="main", schema="raw", name="customers"),
    )
    cursor: FakeDatabricksMetadataCursor = FakeDatabricksMetadataCursor(
        rows=[
            ("main", "raw", "orders", test_case.expected_data_versions[0]),
            ("main", "raw", "customers", test_case.expected_data_versions[1]),
        ]
    )
    connection: FakeDatabricksMetadataConnection = FakeDatabricksMetadataConnection((cursor,))

    metadata_by_request: dict[TableFreshnessRequest, TableFreshnessMetadata] = (
        adapter.get_tables_freshness_metadata(connection=connection, requests=requests)
    )

    assert (
        tuple(metadata_by_request[request].data_version for request in requests)
        == test_case.expected_data_versions
    )
    assert all(metadata.value_kind == "timestamp" for metadata in metadata_by_request.values())
    assert cursor.executed_sql is not None
    for fragment in test_case.expected_query_fragments:
        assert fragment in cursor.executed_sql
    assert cursor.closed is True


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksTableFreshnessMixedBatchTestCase(
            description="a view in the batch leaves only the view unknown",
            delta_version=datetime(2026, 1, 4, 3, 4, 5),
            view_error="DESCRIBE HISTORY is only supported for Delta tables\nstack line",
            expected_view_message=(
                "Databricks Delta history unavailable for orders_v: "
                "DESCRIBE HISTORY is only supported for Delta tables"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_view_in_freshness_batch_when_getting_metadata_then_only_the_view_is_unknown(
    test_case: DatabricksTableFreshnessMixedBatchTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()
    orders: TableFreshnessRequest = TableFreshnessRequest(
        database="main", schema="raw", name="orders"
    )
    orders_v: TableFreshnessRequest = TableFreshnessRequest(
        database="main", schema="raw", name="orders_v"
    )
    batch_cursor: FakeDatabricksMetadataCursor = FailingDatabricksMetadataCursor(
        execute_error=RuntimeError(test_case.view_error)
    )
    orders_cursor: FakeDatabricksMetadataCursor = FakeDatabricksMetadataCursor(
        rows=[("main", "raw", "orders", test_case.delta_version)]
    )
    view_cursor: FakeDatabricksMetadataCursor = FailingDatabricksMetadataCursor(
        execute_error=RuntimeError(test_case.view_error)
    )
    connection: FakeDatabricksMetadataConnection = FakeDatabricksMetadataConnection(
        (batch_cursor, orders_cursor, view_cursor)
    )

    metadata_by_request: dict[TableFreshnessRequest, TableFreshnessMetadata] = (
        adapter.get_tables_freshness_metadata(connection=connection, requests=(orders, orders_v))
    )

    assert metadata_by_request[orders].status == TableFreshnessStatus.OBSERVED
    assert metadata_by_request[orders].data_version == test_case.delta_version
    assert metadata_by_request[orders_v].status == TableFreshnessStatus.UNAVAILABLE
    assert metadata_by_request[orders_v].message == test_case.expected_view_message
    assert orders_cursor.executed_sql is not None
    assert "DESCRIBE HISTORY `main`.`raw`.`orders`" in orders_cursor.executed_sql
    assert "UNION ALL" not in orders_cursor.executed_sql
    assert all(cursor.closed for cursor in connection.returned_cursors)
    assert "information_schema" not in "".join(
        cursor.executed_sql or "" for cursor in connection.returned_cursors
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksRenderDeleteInsertCursorTestCase(
            description="renders replace where for timestamp cursor bounds",
            target="`workspace`.`test`.`orders`",
            sql="SELECT * FROM `workspace`.`test`.`orders__delta`",
            cursor_column="ordered_at",
            cursor_start="2026-01-01 00:00:00",
            cursor_end="2026-01-02 00:00:00",
            columns=None,
            expected_statements=(
                "INSERT INTO `workspace`.`test`.`orders` REPLACE WHERE "
                "`ordered_at` >= TIMESTAMP '2026-01-01 00:00:00' AND "
                "`ordered_at` < TIMESTAMP '2026-01-02 00:00:00' "
                "SELECT * FROM `workspace`.`test`.`orders__delta`",
            ),
        ),
        DatabricksRenderDeleteInsertCursorTestCase(
            description="renders replace where with explicit columns",
            target="`workspace`.`test`.`orders`",
            sql="SELECT id, status FROM `workspace`.`test`.`orders__delta`",
            cursor_column="id",
            cursor_start="1",
            cursor_end="10",
            columns=("id", "status"),
            expected_statements=(
                "INSERT INTO `workspace`.`test`.`orders` (`id`, `status`) REPLACE WHERE "
                "`id` >= 1 AND `id` < 10 "
                "SELECT id, status FROM `workspace`.`test`.`orders__delta`",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_delete_insert_when_rendering_then_databricks_uses_replace_where(
    test_case: DatabricksRenderDeleteInsertCursorTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    statements: tuple[str, ...] = adapter.render_delete_insert_cursor(
        destination=test_case.target,
        sql=test_case.sql,
        cursor_column=test_case.cursor_column,
        cursor_start=test_case.cursor_start,
        cursor_end=test_case.cursor_end,
        columns=test_case.columns,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksMergeExclusionTestCase(
            description="databricks selective merge",
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
    test_case: DatabricksMergeExclusionTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    rendered_sql: str = adapter.render_merge(
        destination="`workspace`.`test`.`orders`",
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
        DatabricksRenderCloneTestCase(
            description="renders shallow table clone by default",
            source="`workspace`.`prod`.`fact_orders`",
            target="`workspace`.`dev`.`fact_orders`",
            hard_copy=False,
            expected_statements=(
                "CREATE TABLE `workspace`.`dev`.`fact_orders` "
                "SHALLOW CLONE `workspace`.`prod`.`fact_orders`",
            ),
            expected_supports_zero_copy=True,
        ),
        DatabricksRenderCloneTestCase(
            description="renders CTAS when hard copy is requested",
            source="`workspace`.`prod`.`fact_orders`",
            target="`workspace`.`dev`.`fact_orders`",
            hard_copy=True,
            expected_statements=(
                "CREATE OR REPLACE TABLE `workspace`.`dev`.`fact_orders` AS "
                "SELECT * FROM `workspace`.`prod`.`fact_orders`",
            ),
            expected_supports_zero_copy=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_clone_request_when_rendering_then_databricks_uses_expected_clone_sql(
    test_case: DatabricksRenderCloneTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

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
        DatabricksRenderDurableCloneTestCase(
            description="renders deep clone for durable physical versions",
            source="`workspace`.`prod`.`fact_orders`",
            target="`workspace`.`dev`.`fact_orders`",
            expected_statements=(
                "CREATE TABLE `workspace`.`dev`.`fact_orders` "
                "DEEP CLONE `workspace`.`prod`.`fact_orders`",
            ),
            expected_supports_durable_clone=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_durable_clone_request_when_rendering_then_databricks_uses_deep_clone_sql(
    test_case: DatabricksRenderDurableCloneTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    statements: tuple[str, ...] = adapter.render_durable_clone(
        origin=test_case.source,
        destination=test_case.target,
    )

    assert adapter.supports_durable_clone() is test_case.expected_supports_durable_clone
    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksRenderPythonFunctionTestCase(
            description="renders Python UDF DDL with unwrapped function body",
            body_sql=(
                "def main(order_status: str | None) -> bool:\n    return order_status == 'completed'"
            ),
            packages=(),
            expected_statements=(
                "CREATE OR REPLACE FUNCTION `workspace`.`test`.`is_completed_order_py`"
                "(order_status STRING)\n"
                "RETURNS BOOLEAN\n"
                "LANGUAGE PYTHON\n"
                "AS $$\n"
                "return order_status == 'completed'\n"
                "$$",
            ),
        ),
        DatabricksRenderPythonFunctionTestCase(
            description="renders Python UDF DDL with imports helpers and dependencies",
            body_sql=(
                "import json\n\n"
                "def normalize(value):\n"
                "    return value.strip()\n\n"
                "def main(order_status: str | None) -> bool:\n"
                "    if order_status is None:\n"
                "        return False\n"
                "    return normalize(order_status) == 'completed'"
            ),
            packages=("simplejson==3.19.3",),
            expected_statements=(
                "CREATE OR REPLACE FUNCTION `workspace`.`test`.`is_completed_order_py`"
                "(order_status STRING)\n"
                "RETURNS BOOLEAN\n"
                "LANGUAGE PYTHON\n"
                "ENVIRONMENT (\n"
                "  dependencies = '[\"simplejson==3.19.3\"]',\n"
                "  environment_version = 'None'\n"
                ")\n"
                "AS $$\n"
                "import json\n\n"
                "def normalize(value):\n"
                "    return value.strip()\n"
                "if order_status is None:\n"
                "    return False\n"
                "return normalize(order_status) == 'completed'\n"
                "$$",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_function_when_rendering_then_databricks_returns_expected_ddl(
    test_case: DatabricksRenderPythonFunctionTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    statements: tuple[str, ...] = adapter.render_create_function(
        destination="`workspace`.`test`.`is_completed_order_py`",
        arguments=(FunctionArgument(name="order_status", type="STRING"),),
        returns="BOOLEAN",
        body_sql=test_case.body_sql,
        language=FunctionLanguage.PYTHON,
        runtime_version="3.11",
        entry_point="main",
        packages=test_case.packages,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksPythonFunctionSupportTestCase(
            description="supports Python function execution",
            expected_supports_python_functions=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_databricks_adapter_when_checking_capabilities_then_python_functions_supported(
    test_case: DatabricksPythonFunctionSupportTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    assert adapter.supports_python_functions() is test_case.expected_supports_python_functions


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksRenderTableFunctionTestCase(
            description="renders SQL table function DDL",
            expected_statements=(
                "CREATE OR REPLACE FUNCTION `workspace`.`test`.`customer_orders`"
                "(p_customer_id INT)\n"
                "RETURNS TABLE\n"
                "RETURN SELECT order_id FROM `workspace`.`test`.`fact_orders`\n"
                "WHERE customer_id = p_customer_id",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_when_rendering_then_databricks_returns_expected_ddl(
    test_case: DatabricksRenderTableFunctionTestCase,
) -> None:
    adapter: DatabricksAdapter = DatabricksAdapter()

    statements: tuple[str, ...] = adapter.render_create_function(
        destination="`workspace`.`test`.`customer_orders`",
        arguments=(FunctionArgument(name="p_customer_id", type="INT"),),
        returns="TABLE",
        body_sql=(
            "SELECT order_id FROM `workspace`.`test`.`fact_orders`\n"
            "WHERE customer_id = p_customer_id"
        ),
        return_columns=(FunctionReturnColumn(name="order_id", type="INT"),),
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksRelationAgeMetadataTestCase(
            description="managed table ignores DDL-only information_schema timestamps",
            table_type="MANAGED",
            created=datetime(2026, 8, 1, 3, 0, tzinfo=timezone(timedelta(hours=-7))),
            last_altered=datetime(2026, 8, 2, 12, 30, tzinfo=timezone(timedelta(hours=2))),
            expected_relation_type="table",
            expected_timestamps=("None", "None"),
        ),
        DatabricksRelationAgeMetadataTestCase(
            description="view offset timestamps are normalized to UTC",
            table_type="VIEW",
            created=datetime(2026, 8, 1, 3, 0, tzinfo=timezone(timedelta(hours=-7))),
            last_altered=datetime(2026, 8, 2, 12, 30, tzinfo=timezone(timedelta(hours=2))),
            expected_relation_type="view",
            expected_timestamps=("2026-08-01 10:00:00+00:00", "2026-08-02 10:30:00+00:00"),
        ),
        DatabricksRelationAgeMetadataTestCase(
            description="view naive timestamps are treated as UTC",
            table_type="VIEW",
            created=datetime(2026, 8, 1, 3, 0),
            last_altered=datetime(2026, 8, 3, 4, 0),
            expected_relation_type="view",
            expected_timestamps=("2026-08-01 03:00:00+00:00", "2026-08-03 04:00:00+00:00"),
        ),
        DatabricksRelationAgeMetadataTestCase(
            description="missing view timestamps stay unknown",
            table_type="VIEW",
            created=None,
            last_altered=None,
            expected_relation_type="view",
            expected_timestamps=("None", "None"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_databricks_relation_timestamps_when_listing_then_exposes_view_ages_only(
    test_case: DatabricksRelationAgeMetadataTestCase,
) -> None:
    cursor: FakeDatabricksMetadataCursor = FakeDatabricksMetadataCursor(
        rows=[
            (
                "OLD_ORDERS",
                "DEV_ORDERS",
                test_case.table_type,
                test_case.created,
                test_case.last_altered,
            )
        ]
    )
    adapter: DatabricksAdapter = DatabricksAdapter()

    relations: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=cast(Any, FakeDatabricksMetadataConnection((cursor,))),
        database="analytics",
        schemas=("dev_orders",),
    )

    assert "table_type, created, last_altered" in str(cursor.executed_sql)
    assert relations[0].name == "old_orders"
    assert relations[0].relation_type == test_case.expected_relation_type
    assert (
        str(relations[0].created_at),
        str(relations[0].last_altered_at),
    ) == test_case.expected_timestamps


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksDeltaDetailAgeTestCase(
            description="table age comes from Delta detail lastModified in UTC",
            relation=RelationInfo(
                database="analytics", schema="dev_orders", name="old_orders", relation_type="table"
            ),
            cursors=(
                FakeDatabricksMetadataCursor(
                    rows=[
                        (
                            "delta",
                            datetime(2025, 1, 1, 3, 0, tzinfo=timezone(timedelta(hours=-7))),
                            datetime(2026, 8, 2, 12, 30, tzinfo=timezone(timedelta(hours=2))),
                        )
                    ],
                    description=(("format",), ("createdAt",), ("lastModified",)),
                ),
            ),
            expected_sql=("DESCRIBE DETAIL `analytics`.`dev_orders`.`old_orders`",),
            expected_timestamps=("2025-01-01 10:00:00+00:00", "2026-08-02 10:30:00+00:00"),
        ),
        DatabricksDeltaDetailAgeTestCase(
            description="backticks in table identifiers are escaped",
            relation=RelationInfo(
                database="analytics", schema="dev_orders", name="odd`orders", relation_type="table"
            ),
            cursors=(
                FakeDatabricksMetadataCursor(
                    rows=[("delta", None, datetime(2026, 8, 2, 10, 30))],
                    description=(("format",), ("createdAt",), ("lastModified",)),
                ),
            ),
            expected_sql=("DESCRIBE DETAIL `analytics`.`dev_orders`.`odd``orders`",),
            expected_timestamps=("None", "2026-08-02 10:30:00+00:00"),
        ),
        DatabricksDeltaDetailAgeTestCase(
            description="views keep information_schema ages without Delta detail",
            relation=RelationInfo(
                database="analytics",
                schema="dev_orders",
                name="old_orders_view",
                relation_type="view",
                created_at=datetime(2026, 8, 1, tzinfo=UTC),
                last_altered_at=datetime(2026, 8, 3, tzinfo=UTC),
            ),
            cursors=(),
            expected_sql=(),
            expected_timestamps=("2026-08-01 00:00:00+00:00", "2026-08-03 00:00:00+00:00"),
        ),
        DatabricksDeltaDetailAgeTestCase(
            description="failed Delta detail leaves table age unknown",
            relation=RelationInfo(
                database="analytics",
                schema="dev_orders",
                name="old_orders",
                relation_type="table",
                created_at=datetime(2025, 1, 1, tzinfo=UTC),
            ),
            cursors=(
                FailingDatabricksMetadataCursor(execute_error=RuntimeError("not a Delta table")),
            ),
            expected_sql=("DESCRIBE DETAIL `analytics`.`dev_orders`.`old_orders`",),
            expected_timestamps=("None", "None"),
        ),
        DatabricksDeltaDetailAgeTestCase(
            description="Delta detail without lastModified leaves table age unknown",
            relation=RelationInfo(
                database="analytics", schema="dev_orders", name="old_orders", relation_type="table"
            ),
            cursors=(
                FakeDatabricksMetadataCursor(
                    rows=[("delta", datetime(2025, 1, 1, tzinfo=UTC))],
                    description=(("format",), ("createdAt",)),
                ),
            ),
            expected_sql=("DESCRIBE DETAIL `analytics`.`dev_orders`.`old_orders`",),
            expected_timestamps=("None", "None"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_candidate_relations_when_adding_age_metadata_then_databricks_reads_delta_detail(
    test_case: DatabricksDeltaDetailAgeTestCase,
) -> None:
    connection: FakeDatabricksMetadataConnection = FakeDatabricksMetadataConnection(
        test_case.cursors
    )

    enriched: tuple[RelationInfo, ...] = DatabricksAdapter().with_relation_age_metadata(
        connection=cast(Any, connection),
        relations=(test_case.relation,),
    )

    assert (
        tuple(cursor.executed_sql for cursor in connection.returned_cursors)
        == test_case.expected_sql
    )
    assert tuple(cursor.closed for cursor in connection.returned_cursors) == tuple(
        True for _ in test_case.expected_sql
    )
    assert (
        str(enriched[0].created_at),
        str(enriched[0].last_altered_at),
    ) == test_case.expected_timestamps


@pytest.mark.parametrize(
    "test_case",
    [
        DatabricksSchemaExistsTestCase(
            description="a schema in the session's current catalog is found without a catalog",
            database=None,
            rows=[(1,)],
            expected_exists=True,
            expected_query_fragment="SELECT 1 FROM information_schema.schemata",
        ),
        DatabricksSchemaExistsTestCase(
            description="a schema missing from the current catalog is reported missing",
            database=None,
            rows=[],
            expected_exists=False,
            expected_query_fragment="SELECT 1 FROM information_schema.schemata",
        ),
        DatabricksSchemaExistsTestCase(
            description="an explicit catalog is queried through its information schema",
            database="main",
            rows=[(1,)],
            expected_exists=True,
            expected_query_fragment="SELECT 1 FROM `main`.information_schema.schemata",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_databricks_schema_when_checking_existence_then_current_catalog_is_used_without_one(
    test_case: DatabricksSchemaExistsTestCase,
) -> None:
    cursor: FakeDatabricksMetadataCursor = FakeDatabricksMetadataCursor(rows=test_case.rows)

    exists: bool = DatabricksAdapter().schema_exists(
        connection=cast(Any, FakeDatabricksMetadataConnection((cursor,))),
        database=test_case.database,
        schema="analytics",
    )

    assert exists is test_case.expected_exists, test_case.description
    assert cursor.executed_sql is not None
    assert test_case.expected_query_fragment in cursor.executed_sql, test_case.description
