"""Integration coverage for built-in adapter snapshot SQL rendering."""

from __future__ import annotations

import pytest

from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.integration.src.sqlbuild.adapters.snapshot_sql_rendering._test_types import (
    SnapshotReappearanceRenderingTestCase,
    SnapshotSqlRenderingAdapterTestCase,
    SqlServerSnapshotRenderingTestCase,
)
from tests.integration.src.sqlbuild.adapters.snapshot_sql_rendering.helpers import (
    render_snapshot_sql_matrix,
)

_REAPPEARANCE_INITIAL_FRAGMENTS: tuple[str, ...] = (
    "__observed_group_sequence AS (SELECT __observed_at, "
    "LAG(__observed_at) OVER (ORDER BY __observed_at) AS __prev_group_observed_at, "
    "LEAD(__observed_at) OVER (ORDER BY __observed_at) AS __next_group_observed_at "
    "FROM (SELECT DISTINCT observed_at AS __observed_at FROM source_table)",
    "(__prev_observed_at IS NOT NULL AND __prev_observed_at <> __prev_group_observed_at)",
    "ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING) AS __next_absence_at",
    "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
)
_REAPPEARANCE_APPLY_FRAGMENTS: tuple[str, ...] = (
    "AS __latest_rn FROM target_table) AS __latest_ranked WHERE __latest_rn = 1",
    "__reappearances AS (SELECT __classified.customer_id AS customer_id, "
    "MIN(__classified.observed_at) AS __reappeared_at",
    "WHERE __latest.valid_to IS NOT NULL AND __closing_group.__observed_at IS NULL",
    "OR __classified.observed_at < __first_new_starts.__first_version_start",
    "__new_changes AS (SELECT *, LEAD(__version_start) OVER",
    "customer_id, __version_start AS __close_at FROM __new_changes",
    "__new_changes.__version_start, CASE WHEN __new_changes.__next_version_start IS NULL "
    "THEN __new_changes.__next_absence_at",
)
_PORTABLE_APPLY_EXCLUSIONS: tuple[str, ...] = ("QUALIFY", "UNION DISTINCT", "NOT EXISTS")


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotSqlRenderingAdapterTestCase(
            description="duckdb renders complete snapshot SQL matrix",
            adapter=DuckDbAdapter(),
            expected_create_initial_fragments=(
                "CREATE OR REPLACE TABLE target_table AS SELECT *",
                "updated_at AS valid_from",
                "CASE WHEN 1 = 0 THEN updated_at END AS valid_to",
            ),
            expected_timestamp_hard_delete_fragments=(
                "UPDATE target_table AS __target SET effective_to = __source.updated_at",
                "__target.customer_id = __source.customer_id",
                "__target.region = __source.region",
                "WHERE __target.effective_to IS NULL AND NOT EXISTS",
                "MAX(effective_to) AS __closed_at FROM target_table "
                "GROUP BY customer_id, region) AS __history",
                "AND __history.__closed_at IS NOT NULL THEN",
            ),
            expected_historical_check_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_apply_hard_delete_fragments=(
                "__hard_deletes AS (",
                "AS __close_at FROM target_table AS __target",
                "UNION ALL",
            ),
            expected_historical_check_apply_fragments=(
                "LAG(plan) OVER (PARTITION BY customer_id ORDER BY observed_at)",
                "__hard_deletes AS (",
                "INSERT INTO target_table (customer_id, plan, observed_at, valid_from, valid_to)",
                "SET valid_to = (SELECT MIN(__close_candidates.__close_at)",
                "UNION ALL",
            ),
        ),
        SnapshotSqlRenderingAdapterTestCase(
            description="bigquery renders complete snapshot SQL matrix",
            adapter=BigQueryAdapter(),
            expected_create_initial_fragments=(
                "CREATE OR REPLACE TABLE `target_table` AS SELECT *",
                "updated_at AS valid_from",
                "CASE WHEN 1 = 0 THEN updated_at END AS valid_to",
            ),
            expected_timestamp_hard_delete_fragments=(
                "UPDATE target_table AS __target SET effective_to = __source.updated_at",
                "__target.customer_id = __source.customer_id",
                "__target.region = __source.region",
                "WHERE __target.effective_to IS NULL AND NOT EXISTS",
                "MAX(effective_to) AS __closed_at FROM target_table "
                "GROUP BY customer_id, region) AS __history",
                "AND __history.__closed_at IS NOT NULL THEN",
            ),
            expected_historical_check_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_apply_hard_delete_fragments=(
                "__hard_deletes AS (",
                "AS __close_at FROM target_table AS __target",
                "UNION ALL",
            ),
            expected_historical_check_apply_fragments=(
                "LAG(plan) OVER (PARTITION BY customer_id ORDER BY observed_at)",
                "__hard_deletes AS (",
                "INSERT INTO target_table (customer_id, plan, observed_at, valid_from, valid_to)",
                "SET valid_to = __close_candidates.__close_at FROM (WITH",
                "UNION ALL",
            ),
        ),
        SnapshotSqlRenderingAdapterTestCase(
            description="snowflake renders complete snapshot SQL matrix",
            adapter=SnowflakeAdapter(),
            expected_create_initial_fragments=(
                "CREATE OR REPLACE TRANSIENT TABLE target_table AS SELECT *",
                "updated_at AS valid_from",
                "CASE WHEN 1 = 0 THEN updated_at END AS valid_to",
            ),
            expected_timestamp_hard_delete_fragments=(
                "UPDATE target_table AS __target SET effective_to = __source.updated_at",
                "__target.customer_id = __source.customer_id",
                "__target.region = __source.region",
                "WHERE __target.effective_to IS NULL AND NOT EXISTS",
                "MAX(effective_to) AS __closed_at FROM target_table "
                "GROUP BY customer_id, region) AS __history",
                "AND __history.__closed_at IS NOT NULL THEN",
            ),
            expected_historical_check_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_apply_hard_delete_fragments=(
                "__hard_deletes AS (",
                "AS __close_at FROM target_table AS __target",
                "UNION ALL",
            ),
            expected_historical_check_apply_fragments=(
                "LAG(plan) OVER (PARTITION BY customer_id ORDER BY observed_at)",
                "__hard_deletes AS (",
                "INSERT INTO target_table (customer_id, plan, observed_at, valid_from, valid_to)",
                "SET valid_to = __close_candidates.__close_at FROM (WITH",
                "UNION ALL",
            ),
        ),
        SnapshotSqlRenderingAdapterTestCase(
            description="databricks renders complete snapshot SQL matrix",
            adapter=DatabricksAdapter(),
            expected_create_initial_fragments=(
                "CREATE OR REPLACE TABLE target_table AS SELECT *",
                "updated_at AS valid_from",
                "CASE WHEN 1 = 0 THEN updated_at END AS valid_to",
            ),
            expected_timestamp_hard_delete_fragments=(
                "MERGE INTO target_table AS __target USING source_table AS __source",
                "__target.customer_id = __source.customer_id",
                "__target.region = __source.region",
                "WHEN MATCHED THEN UPDATE SET effective_to = __source.updated_at",
                "MAX(effective_to) AS __closed_at FROM target_table "
                "GROUP BY customer_id, region) AS __history",
                "AND __history.__closed_at IS NOT NULL THEN",
            ),
            expected_historical_check_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
            ),
            expected_historical_timestamp_apply_hard_delete_fragments=(
                "__hard_deletes AS (",
                "AS __close_at FROM target_table AS __target",
                "UNION ALL",
            ),
            expected_historical_check_apply_fragments=(
                "LAG(plan) OVER (PARTITION BY customer_id ORDER BY observed_at)",
                "__hard_deletes AS (",
                "INSERT INTO target_table (customer_id, plan, observed_at, valid_from, valid_to)",
                "MERGE INTO target_table AS __target USING (WITH",
                "UNION ALL",
            ),
        ),
        SnapshotSqlRenderingAdapterTestCase(
            description="sqlserver renders complete snapshot SQL matrix",
            adapter=SqlServerAdapter(),
            expected_create_initial_fragments=(
                "SELECT * INTO target_table FROM (SELECT *",
                "updated_at AS valid_from",
                "CASE WHEN 1 = 0 THEN updated_at END AS valid_to",
            ),
            expected_timestamp_hard_delete_fragments=(
                "UPDATE __target SET effective_to = __source.updated_at",
                "__target.customer_id = __source.customer_id",
                "__target.region = __source.region",
                "WHERE __target.effective_to IS NULL AND NOT EXISTS",
                "MAX(effective_to) AS __closed_at FROM target_table "
                "GROUP BY customer_id, region) AS __history",
                "AND __history.__closed_at IS NOT NULL THEN",
            ),
            expected_historical_check_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
                "AS valid_to INTO target_table FROM __versions",
            ),
            expected_historical_timestamp_initial_hard_delete_fragments=(
                "__next_absence_at",
                "WHEN __next_absence_at < __next_version_start THEN __next_absence_at",
                "AS valid_to INTO target_table FROM __versions",
            ),
            expected_historical_timestamp_apply_hard_delete_fragments=(
                "__hard_deletes AS (",
                "AS __close_at FROM target_table AS __target",
                "UNION ALL",
            ),
            expected_historical_check_apply_fragments=(
                "LAG(plan) OVER (PARTITION BY customer_id ORDER BY observed_at)",
                "__hard_deletes AS (",
                "INSERT INTO target_table (customer_id, plan, observed_at, valid_from, valid_to)",
                "UPDATE __target SET valid_to = (SELECT MIN(__close_candidates.__close_at)",
                "UNION ALL",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_snapshot_sql_then_covers_snapshot_matrix(
    test_case: SnapshotSqlRenderingAdapterTestCase,
) -> None:
    create_initial_sql: str = "\n".join(
        test_case.adapter.render_create_initial_snapshot_destination(
            table_type="transient",
            destination="target_table",
            origin="source_table",
            snapshot_strategy="timestamp",
            updated_at_column="updated_at",
            observed_at_column=None,
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            initial_valid_from=None,
        )
    )
    timestamp_hard_delete_sql: str = "\n".join(
        test_case.adapter.render_apply_timestamp_snapshot_changes(
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id", "region"),
            updated_at_column="updated_at",
            observed_at_column=None,
            valid_from_column="effective_from",
            valid_to_column="effective_to",
            initial_valid_from=None,
            output_columns=("customer_id", "region", "plan", "updated_at"),
            invalidate_hard_deletes=True,
        )
    )
    historical_check_initial_sql: str = "\n".join(
        test_case.adapter.render_create_initial_historical_check_snapshot_destination(
            table_type="transient",
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            check_columns=("plan",),
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )
    historical_timestamp_initial_sql: str = "\n".join(
        test_case.adapter.render_create_initial_historical_timestamp_snapshot_destination(
            table_type="transient",
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            updated_at_column="updated_at",
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "updated_at", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )
    historical_timestamp_apply_sql: str = "\n".join(
        test_case.adapter.render_apply_historical_timestamp_snapshot_changes(
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            updated_at_column="updated_at",
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "updated_at", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )
    historical_check_apply_sql: str = "\n".join(
        test_case.adapter.render_apply_historical_check_snapshot_changes(
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            check_columns=("plan",),
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )

    expected_fragment: str
    for expected_fragment in test_case.expected_create_initial_fragments:
        assert expected_fragment in create_initial_sql
    for expected_fragment in test_case.expected_timestamp_hard_delete_fragments:
        assert expected_fragment in timestamp_hard_delete_sql
    for expected_fragment in test_case.expected_historical_check_initial_hard_delete_fragments:
        assert expected_fragment in historical_check_initial_sql
    for expected_fragment in test_case.expected_historical_timestamp_initial_hard_delete_fragments:
        assert expected_fragment in historical_timestamp_initial_sql
    for expected_fragment in test_case.expected_historical_timestamp_apply_hard_delete_fragments:
        assert expected_fragment in historical_timestamp_apply_sql
    for expected_fragment in test_case.expected_historical_check_apply_fragments:
        assert expected_fragment in historical_check_apply_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotReappearanceRenderingTestCase(
            description="duckdb renders portable hard-delete history SQL",
            adapter=DuckDbAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="motherduck renders portable hard-delete history SQL",
            adapter=MotherDuckAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="postgres renders portable hard-delete history SQL",
            adapter=PostgresAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="bigquery renders portable hard-delete history SQL",
            adapter=BigQueryAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="snowflake renders portable hard-delete history SQL",
            adapter=SnowflakeAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="databricks renders portable hard-delete history SQL",
            adapter=DatabricksAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=_PORTABLE_APPLY_EXCLUSIONS,
        ),
        SnapshotReappearanceRenderingTestCase(
            description="sqlserver renders portable hard-delete history SQL",
            adapter=SqlServerAdapter(),
            expected_initial_fragments=_REAPPEARANCE_INITIAL_FRAGMENTS,
            expected_apply_fragments=_REAPPEARANCE_APPLY_FRAGMENTS,
            unexpected_apply_fragments=(*_PORTABLE_APPLY_EXCLUSIONS, "IS DISTINCT FROM"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hard_deletes_when_rendering_historical_check_snapshot_then_reopens_reappearing_keys(
    test_case: SnapshotReappearanceRenderingTestCase,
) -> None:
    initial_sql: str = "\n".join(
        test_case.adapter.render_create_initial_historical_check_snapshot_destination(
            table_type="table",
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            check_columns=("plan",),
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )
    apply_statements: tuple[str, ...] = (
        test_case.adapter.render_apply_historical_check_snapshot_changes(
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            check_columns=("plan",),
            observed_at_column="observed_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "observed_at"),
            invalidate_hard_deletes=True,
        )
    )
    without_hard_deletes_sql: str = "\n".join(
        (
            *test_case.adapter.render_create_initial_historical_check_snapshot_destination(
                table_type="table",
                destination="target_table",
                origin="source_table",
                unique_key=("customer_id",),
                check_columns=("plan",),
                observed_at_column="observed_at",
                valid_from_column="valid_from",
                valid_to_column="valid_to",
                output_columns=("customer_id", "plan", "observed_at"),
                invalidate_hard_deletes=False,
            ),
            *test_case.adapter.render_apply_historical_check_snapshot_changes(
                destination="target_table",
                origin="source_table",
                unique_key=("customer_id",),
                check_columns=("plan",),
                observed_at_column="observed_at",
                valid_from_column="valid_from",
                valid_to_column="valid_to",
                output_columns=("customer_id", "plan", "observed_at"),
                invalidate_hard_deletes=False,
            ),
        )
    )

    expected_fragment: str
    for expected_fragment in test_case.expected_initial_fragments:
        assert expected_fragment in initial_sql
    apply_sql: str = "\n".join(apply_statements)
    for expected_fragment in test_case.expected_apply_fragments:
        assert expected_fragment in apply_sql
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_apply_fragments:
        assert unexpected_fragment not in apply_sql
    assert "__prev_group_observed_at" not in without_hard_deletes_sql
    assert "__reappearances" not in without_hard_deletes_sql
    assert "__next_absence_at" not in without_hard_deletes_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSnapshotRenderingTestCase(
            description="sqlserver renders every snapshot statement without QUALIFY",
            unexpected_fragments=("QUALIFY",),
            expected_changes_apply_prefix=(
                ";WITH __latest_ordered AS (SELECT *, ROW_NUMBER() OVER ("
                "PARTITION BY customer_id ORDER BY updated_at DESC) AS __rn FROM target_table), "
                "__latest AS (SELECT * FROM __latest_ordered WHERE __rn = 1), __new_changes AS ("
            ),
            expected_changes_apply_fragments=(
                "UPDATE __target SET valid_to = (",
                "FROM target_table AS __target WHERE __target.valid_to IS NULL",
                "INSERT INTO target_table (",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_when_rendering_snapshot_matrix_then_sql_is_tsql_compatible(
    test_case: SqlServerSnapshotRenderingTestCase,
) -> None:
    statements_by_label: dict[str, str] = render_snapshot_sql_matrix(SqlServerAdapter())
    changes_apply_statements: tuple[str, ...] = (
        SqlServerAdapter().render_apply_historical_timestamp_changes(
            destination="target_table",
            origin="source_table",
            unique_key=("customer_id",),
            updated_at_column="updated_at",
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            output_columns=("customer_id", "plan", "updated_at"),
        )
    )

    fragment: str
    for fragment in test_case.unexpected_fragments:
        assert {
            label: fragment in statement for label, statement in statements_by_label.items()
        } == dict.fromkeys(statements_by_label, False)
    assert len(changes_apply_statements) == 2
    statement: str
    for statement in changes_apply_statements:
        assert statement.startswith(test_case.expected_changes_apply_prefix)
    changes_apply_sql: str = "\n".join(changes_apply_statements)
    expected_fragment: str
    for expected_fragment in test_case.expected_changes_apply_fragments:
        assert expected_fragment in changes_apply_sql
