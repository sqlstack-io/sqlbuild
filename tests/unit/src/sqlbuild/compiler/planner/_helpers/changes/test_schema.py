from __future__ import annotations

import pytest

from sqlbuild._native import (
    InferredColumn,
    SchemaChangeKind,
    SchemaColumnSource,
    SchemaDynamicColumnFamily,
    SchemaFinding,
    detect_schema_changes,
)
from sqlbuild.adapter.contract.models import ColumnInfo
from tests.unit.src.sqlbuild.compiler.planner._helpers.changes._test_types import (
    DetectSchemaChangesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DetectSchemaChangesTestCase(
            description="detects no changes when yml columns match warehouse with enforcement",
            yml_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="name", type="VARCHAR"),
            ),
            inferred_columns=None,
            warehouse_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="name", type="VARCHAR"),
            ),
            type_enforcement=True,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="detects added column from yml with enforcement",
            yml_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="status", type="VARCHAR"),
            ),
            inferred_columns=None,
            warehouse_columns=(ColumnInfo(name="id", type="INTEGER"),),
            type_enforcement=True,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_ADDED,
                    column_name="status",
                    source=SchemaColumnSource.YML,
                    expected_type="VARCHAR",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="detects removed column against yml with enforcement",
            yml_columns=(ColumnInfo(name="id", type="INTEGER"),),
            inferred_columns=None,
            warehouse_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="old_col", type="BOOLEAN"),
            ),
            type_enforcement=True,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_REMOVED,
                    column_name="old_col",
                    source=SchemaColumnSource.YML,
                    actual_type="BOOLEAN",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="detects type changed column from yml with enforcement",
            yml_columns=(ColumnInfo(name="id", type="BIGINT"),),
            inferred_columns=None,
            warehouse_columns=(ColumnInfo(name="id", type="INTEGER"),),
            type_enforcement=True,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="id",
                    source=SchemaColumnSource.YML,
                    expected_type="BIGINT",
                    actual_type="INTEGER",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="detects added column from sql_analysis when yml has no columns",
            yml_columns=(),
            inferred_columns=(
                InferredColumn(name="id", type=None),
                InferredColumn(name="new_col", type=None),
            ),
            warehouse_columns=(ColumnInfo(name="id", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_ADDED,
                    column_name="new_col",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="detects type change from sql_analysis explicit cast",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="amount", type="DECIMAL(10, 2)"),),
            warehouse_columns=(ColumnInfo(name="amount", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="amount",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="DECIMAL(10, 2)",
                    actual_type="INTEGER",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="ignores inferred type casing differences from warehouse metadata",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="order_date", type="DATE"),),
            warehouse_columns=(ColumnInfo(name="order_date", type="date"),),
            type_enforcement=False,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="ignores enforced yml type casing differences from warehouse metadata",
            yml_columns=(ColumnInfo(name="order_date", type="DATE"),),
            inferred_columns=None,
            warehouse_columns=(ColumnInfo(name="order_date", type="date"),),
            type_enforcement=True,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="ignores non-enforced yml type casing differences from warehouse metadata",
            yml_columns=(ColumnInfo(name="order_date", type="DATE"),),
            inferred_columns=(),
            warehouse_columns=(ColumnInfo(name="order_date", type="date"),),
            type_enforcement=False,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="preserves meaningful inferred decimal scale differences",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="amount", type="DECIMAL(10, 2)"),),
            warehouse_columns=(ColumnInfo(name="amount", type="decimal(10,3)"),),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="amount",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="DECIMAL(10, 2)",
                    actual_type="decimal(10,3)",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="skips type comparison for inferred column with no type",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="amount", type=None),),
            warehouse_columns=(ColumnInfo(name="amount", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="enforced yml type wins over inferred type for same column",
            yml_columns=(ColumnInfo(name="amount", type="DECIMAL"),),
            inferred_columns=(InferredColumn(name="amount", type="FLOAT"),),
            warehouse_columns=(ColumnInfo(name="amount", type="INTEGER"),),
            type_enforcement=True,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="amount",
                    source=SchemaColumnSource.YML,
                    expected_type="DECIMAL",
                    actual_type="INTEGER",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="non-enforced inferred type wins over yml type for same column",
            yml_columns=(ColumnInfo(name="amount", type="DECIMAL"),),
            inferred_columns=(InferredColumn(name="amount", type="FLOAT"),),
            warehouse_columns=(ColumnInfo(name="amount", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="amount",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="FLOAT",
                    actual_type="INTEGER",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="non-enforced yml type used when inferred has no type for same column",
            yml_columns=(ColumnInfo(name="amount", type="DECIMAL"),),
            inferred_columns=(InferredColumn(name="amount", type=None),),
            warehouse_columns=(ColumnInfo(name="amount", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="non-enforced yml detects added column not covered by inferred",
            yml_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="extra", type="VARCHAR"),
            ),
            inferred_columns=(InferredColumn(name="id", type=None),),
            warehouse_columns=(ColumnInfo(name="id", type="INTEGER"),),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_ADDED,
                    column_name="extra",
                    source=SchemaColumnSource.YML,
                    expected_type="VARCHAR",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="removed column uses sql_analysis source when no yml columns exist",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="id", type=None),),
            warehouse_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="old_col", type="BOOLEAN"),
            ),
            type_enforcement=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_REMOVED,
                    column_name="old_col",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    actual_type="BOOLEAN",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="unresolved star suppresses removed columns but preserves positive findings",
            yml_columns=(),
            inferred_columns=(
                InferredColumn(name="amount", type="FLOAT"),
                InferredColumn(name="loaded_at", type="TIMESTAMP"),
            ),
            warehouse_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="amount", type="INTEGER"),
            ),
            type_enforcement=False,
            inferred_schema_complete=False,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="amount",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="FLOAT",
                    actual_type="INTEGER",
                ),
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_ADDED,
                    column_name="loaded_at",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="TIMESTAMP",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="typed dynamic members match their family",
            yml_columns=(ColumnInfo(name="customer_id", type="INTEGER"),),
            inferred_columns=(InferredColumn(name="customer_id", type="INTEGER"),),
            warehouse_columns=(
                ColumnInfo(name="customer_id", type="INTEGER"),
                ColumnInfo(name="books", type="DECIMAL(12,2)"),
                ColumnInfo(name="games", type="DECIMAL(12,2)"),
            ),
            type_enforcement=True,
            inferred_schema_complete=False,
            dynamic_columns=(
                SchemaDynamicColumnFamily(
                    name="category_amounts",
                    pivot_column="category",
                    value_column="amount",
                    aggregate="MAX",
                    type="DECIMAL(12,2)",
                ),
            ),
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="snowflake type aliases match warehouse canonical types",
            yml_columns=(
                ColumnInfo(name="order_id", type="INTEGER"),
                ColumnInfo(name="customer_name", type="VARCHAR"),
                ColumnInfo(name="notes", type="TEXT"),
                ColumnInfo(name="ordered_at", type="TIMESTAMP"),
                ColumnInfo(name="amount", type="NUMERIC(10, 2)"),
            ),
            inferred_columns=None,
            warehouse_columns=(
                ColumnInfo(name="order_id", type="NUMBER(38,0)"),
                ColumnInfo(name="customer_name", type="VARCHAR(16777216)"),
                ColumnInfo(name="notes", type="VARCHAR(16777216)"),
                ColumnInfo(name="ordered_at", type="TIMESTAMP_NTZ"),
                ColumnInfo(name="amount", type="NUMBER(10,2)"),
            ),
            type_enforcement=True,
            dialect="snowflake",
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="snowflake still reports a real varchar length change",
            yml_columns=(ColumnInfo(name="customer_name", type="VARCHAR(20)"),),
            inferred_columns=None,
            warehouse_columns=(ColumnInfo(name="customer_name", type="VARCHAR(16777216)"),),
            type_enforcement=True,
            dialect="snowflake",
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="customer_name",
                    source=SchemaColumnSource.YML,
                    expected_type="VARCHAR(20)",
                    actual_type="VARCHAR(16777216)",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="snowflake matches uppercase declared and inferred names case-insensitively",
            yml_columns=(ColumnInfo(name="ORDER_DATE", type="DATE"),),
            inferred_columns=(
                InferredColumn(name="ORDER_DATE", type="DATE"),
                InferredColumn(name="STATUS", type="VARCHAR"),
            ),
            warehouse_columns=(
                ColumnInfo(name="order_date", type="DATE"),
                ColumnInfo(name="status", type="VARCHAR(16777216)"),
            ),
            type_enforcement=True,
            dialect="snowflake",
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="clickhouse nullable columns are the types a model infers or declares",
            yml_columns=(ColumnInfo(name="order_id", type="BIGINT"),),
            inferred_columns=(
                InferredColumn(name="waffles_ordered", type="Int64"),
                InferredColumn(name="activity_hour", type="DateTime"),
            ),
            warehouse_columns=(
                ColumnInfo(name="order_id", type="Nullable(Int64)"),
                ColumnInfo(name="waffles_ordered", type="Nullable(Int64)"),
                ColumnInfo(name="activity_hour", type="Nullable(DateTime)"),
            ),
            type_enforcement=False,
            dialect="clickhouse",
            expected_findings=(),
        ),
        DetectSchemaChangesTestCase(
            description="clickhouse still reports a real type change with or without nullable",
            yml_columns=(),
            inferred_columns=(
                InferredColumn(name="waffles_ordered", type="Int64"),
                InferredColumn(name="revenue_cents", type="Nullable(Int64)"),
            ),
            warehouse_columns=(
                ColumnInfo(name="waffles_ordered", type="String"),
                ColumnInfo(name="revenue_cents", type="Nullable(String)"),
            ),
            type_enforcement=False,
            dialect="clickhouse",
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="waffles_ordered",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="Int64",
                    actual_type="String",
                ),
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="revenue_cents",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="Nullable(Int64)",
                    actual_type="Nullable(String)",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="databricks still reports a string written into a bounded varchar",
            yml_columns=(),
            inferred_columns=(InferredColumn(name="status", type="STRING"),),
            warehouse_columns=(ColumnInfo(name="status", type="varchar(10)"),),
            type_enforcement=False,
            dialect="databricks",
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="status",
                    source=SchemaColumnSource.SQL_ANALYSIS,
                    expected_type="STRING",
                    actual_type="varchar(10)",
                ),
            ),
        ),
        DetectSchemaChangesTestCase(
            description="without a dialect aliases and name case still differ",
            yml_columns=(ColumnInfo(name="ORDER_ID", type="INTEGER"),),
            inferred_columns=None,
            warehouse_columns=(ColumnInfo(name="ORDER_ID", type="NUMBER(38,0)"),),
            type_enforcement=True,
            expected_findings=(
                SchemaFinding(
                    kind=SchemaChangeKind.COLUMN_TYPE_CHANGED,
                    column_name="ORDER_ID",
                    source=SchemaColumnSource.YML,
                    expected_type="INTEGER",
                    actual_type="NUMBER(38,0)",
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_columns_when_detecting_schema_changes_then_returns_expected_findings(
    test_case: DetectSchemaChangesTestCase,
) -> None:
    result: tuple[SchemaFinding, ...] = detect_schema_changes(
        yml_columns=test_case.yml_columns,
        inferred_columns=test_case.inferred_columns,
        warehouse_columns=test_case.warehouse_columns,
        type_enforcement=test_case.type_enforcement,
        inferred_schema_complete=test_case.inferred_schema_complete,
        dynamic_columns=test_case.dynamic_columns,
        dialect=test_case.dialect,
    )

    assert result == test_case.expected_findings
