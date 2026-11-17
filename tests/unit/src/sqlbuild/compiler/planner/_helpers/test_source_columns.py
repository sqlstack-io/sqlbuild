"""Tests for planner source column gathering."""

from __future__ import annotations

from typing import Any, ClassVar

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledProject,
    CompiledResourceType,
    CursorInputRelation,
    Dialect,
    PlannerInputError,
    PlannerRelationsContext,
    PlannerScope,
    SourceEntry,
    SqlReferenceKind,
    build_cursor_input_relations,
    build_planner_relations_context,
    gather_source_columns,
    live_reads,
    runtime_cursor_producer_names,
    validate_source_cursor_input_columns,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    CursorRuntimeOwnershipTestCase,
    KnownSourceColumnsReuseTestCase,
    MultiDatabaseSourceColumnsTestCase,
    RuntimeCursorProducerNamesTestCase,
    SourceColumnsTestCase,
    SourceCursorInputColumnsTestCase,
    WatermarkLineageTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    build_cursor_input_contract_models,
    build_cursor_input_contract_sources,
    build_function_transitive_watermark_models,
    build_source_cursor_input_model,
    build_test_project_with_source_entry,
    build_transitive_watermark_models,
)


class _RecordingAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "recording-test"

    def __init__(self, column_names: tuple[str, ...]) -> None:
        self.column_names: tuple[str, ...] = column_names
        self.queried_sql: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def close(self, connection: Any) -> None:
        del connection

    def _execute(self, connection: Any, sql: str) -> Any:
        del connection, sql
        raise AssertionError("execute should not be called")

    def query_column_names(self, connection: Any, sql: str) -> tuple[str, ...]:
        del connection
        self.queried_sql.append(sql)
        return self.column_names

    def get_all_columns(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        del connection, database, schemas, names
        return {}


class _QualifiedSourceAdapter(_RecordingAdapter):
    def __init__(self) -> None:
        super().__init__(())
        self.database_queries: list[str | None] = []

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        del connection, schemas, names
        self.database_queries.append(database)
        relations_by_database: dict[str | None, tuple[RelationInfo, ...]] = {
            "DB_A": (
                RelationInfo(database="DB_A", schema="RAW", name="SHARED", relation_type="table"),
            ),
            "DB_B": (
                RelationInfo(database="DB_B", schema="RAW", name="SHARED", relation_type="table"),
            ),
        }
        return relations_by_database.get(database, ())

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        del connection
        return {
            relation.identity: (
                ColumnInfo(name="id", type=f"{relation.database}.{relation.schema}"),
            )
            for relation in relations
        }


@pytest.mark.parametrize(
    "test_case",
    [
        SourceColumnsTestCase(
            description="probes enforced expression source query output",
            source_entry=SourceEntry(
                name="raw_payments",
                expression="SELECT 1 AS id, 1700 AS amount_cents, 'success' AS status",
                type_enforcement=True,
            ),
            adapter_column_names=("id", "amount_cents", "status"),
            expected_queried_sql=("SELECT 1 AS id, 1700 AS amount_cents, 'success' AS status",),
            expected_source_column_names=("id", "amount_cents", "status"),
        ),
        SourceColumnsTestCase(
            description="skips non-enforced expression source probe",
            source_entry=SourceEntry(
                name="raw_payments",
                expression="SELECT 1 AS id, 1700 AS amount_cents",
                type_enforcement=None,
            ),
            adapter_column_names=("id", "amount_cents"),
            expected_queried_sql=(),
            expected_source_column_names=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sources_when_gathering_columns_then_returns_expected_source_columns(
    test_case: SourceColumnsTestCase,
) -> None:
    project: CompiledProjectView = build_test_project_with_source_entry(test_case.source_entry)
    adapter: _RecordingAdapter = _RecordingAdapter(test_case.adapter_column_names)

    result: dict[str, tuple[ColumnInfo, ...]] = gather_source_columns(
        project=project,
        reads=live_reads(adapter=adapter, connection=None),
    )

    assert tuple(adapter.queried_sql) == test_case.expected_queried_sql
    assert tuple(column.name for column in result.get("raw_payments", ())) == (
        test_case.expected_source_column_names
    )


@pytest.mark.parametrize(
    "test_case",
    [
        MultiDatabaseSourceColumnsTestCase(
            description="same physical name in two databases remains isolated and missing omitted",
            expected_source_types={
                "source_a": "DB_A.RAW",
                "source_b": "DB_B.RAW",
            },
            expected_database_queries=("DB_A", "DB_B"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multi_database_sources_when_gathering_columns_then_matches_current_database_only(
    test_case: MultiDatabaseSourceColumnsTestCase,
) -> None:
    project: CompiledProjectView = CompiledProject(
        run_id="test",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
    )
    adapter: _QualifiedSourceAdapter = _QualifiedSourceAdapter()
    source_entries: tuple[SourceEntry, ...] = (
        SourceEntry(name="source_a", database="DB_A", schema="RAW", table="SHARED"),
        SourceEntry(name="source_b", database="DB_B", schema="RAW", table="SHARED"),
        SourceEntry(name="missing", database="DB_A", schema="RAW", table="MISSING"),
    )

    result: dict[str, tuple[ColumnInfo, ...]] = gather_source_columns(
        project=project,
        reads=live_reads(adapter=adapter, connection=None),
        source_entries=source_entries,
    )

    actual_types: dict[str, str] = {name: columns[0].type for name, columns in result.items()}
    assert actual_types == test_case.expected_source_types
    assert tuple(adapter.database_queries) == test_case.expected_database_queries


@pytest.mark.parametrize(
    "test_case",
    [
        KnownSourceColumnsReuseTestCase(
            description="reuses known source columns without warehouse queries",
            known_source_columns={
                "raw_payments": ("id", "amount_cents"),
                "unrelated_source": ("other_id",),
            },
            adapter_column_names=("id", "amount_cents", "status"),
            expected_queried_sql_count=0,
            expected_source_column_names={"raw_payments": ("id", "amount_cents")},
        ),
        KnownSourceColumnsReuseTestCase(
            description="gathers from the warehouse when no known source columns are provided",
            known_source_columns=None,
            adapter_column_names=("id", "amount_cents", "status"),
            expected_queried_sql_count=1,
            expected_source_column_names={"raw_payments": ("id", "amount_cents", "status")},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_source_columns_when_building_relations_context_then_reuses_prior_gather(
    test_case: KnownSourceColumnsReuseTestCase,
) -> None:
    project: CompiledProjectView = build_test_project_with_source_entry(
        SourceEntry(
            name="raw_payments",
            expression="SELECT 1 AS id, 1700 AS amount_cents, 'success' AS status",
            type_enforcement=True,
        )
    )
    adapter: _RecordingAdapter = _RecordingAdapter(test_case.adapter_column_names)
    scope: PlannerScope = PlannerScope(
        upstream_deps={},
        downstream_deps={},
        all_keys={},
        models_by_name={},
        selected_keys=frozenset(),
        execution_order=(),
    )
    known_source_columns_by_name: dict[str, tuple[ColumnInfo, ...]] = {}
    source_name: str
    column_names: tuple[str, ...]
    for source_name, column_names in (test_case.known_source_columns or {}).items():
        columns: list[ColumnInfo] = []
        name: str
        for name in column_names:
            columns.append(ColumnInfo(name=name, type=""))
        known_source_columns_by_name[source_name] = tuple(columns)
    known_source_columns: dict[str, tuple[ColumnInfo, ...]] | None = (
        known_source_columns_by_name or None
    )

    context: PlannerRelationsContext = build_planner_relations_context(
        project=project,
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=None),
        scope=scope,
        known_source_columns=known_source_columns,
    )

    assert len(adapter.queried_sql) == test_case.expected_queried_sql_count
    actual_source_column_names: dict[str, tuple[str, ...]] = {}
    for source_name, context_columns in context.source_warehouse_columns.items():
        column_names: list[str] = []
        for column in context_columns:
            column_names.append(column.name)
        actual_source_column_names[source_name] = tuple(column_names)
    assert actual_source_column_names == test_case.expected_source_column_names


@pytest.mark.parametrize(
    "test_case",
    [
        SourceCursorInputColumnsTestCase(
            description="passes when source cursor input column is known",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "loaded_at"},
            source_columns={"raw_orders": ("order_id", "loaded_at")},
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="passes when default source cursor column is known",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs=None,
            source_columns={"raw_orders": ("order_id", "event_time")},
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="skips when source columns are unknown",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "missing_loaded_at"},
            source_columns={},
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="skips dbt ref cursor input validation",
            reference_kind=SqlReferenceKind.DBT_REF,
            reference_name="dbt_orders",
            cursor_column="event_time",
            cursor_inputs={"dbt_orders": "missing_loaded_at"},
            source_columns={"dbt_orders": ("order_id", "event_time")},
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="skips model ref cursor input validation",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "missing_loaded_at"},
            source_columns={"stg_orders": ("order_id", "event_time")},
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="passes when enforced model contract declares cursor input column",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "loaded_at"},
            source_columns={},
            upstream_contract="enforced",
            upstream_declared_columns=("order_id", "loaded_at"),
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="skips non-enforced model contract cursor input validation",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "missing_loaded_at"},
            source_columns={},
            upstream_contract="none",
            upstream_declared_columns=("order_id",),
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="passes when inferred model output exposes cursor input column",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "loaded_at"},
            source_columns={},
            upstream_inferred_columns=("order_id", "loaded_at"),
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="defers missing inferred model output column to warehouse inspection",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "missing_loaded_at"},
            source_columns={},
            upstream_inferred_columns=("order_id", "event_time"),
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="passes when enforced source contract declares cursor input column",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "loaded_at"},
            source_columns={},
            upstream_contract="enforced",
            upstream_declared_columns=("order_id", "loaded_at"),
            expected_valid=True,
        ),
        SourceCursorInputColumnsTestCase(
            description="skips non-enforced source contract cursor input validation",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "missing_loaded_at"},
            source_columns={},
            upstream_contract="none",
            upstream_declared_columns=("order_id",),
            expected_valid=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_valid_cursor_input_source_columns_when_validating_then_passes(
    test_case: SourceCursorInputColumnsTestCase,
) -> None:
    model: CompiledModelView = build_source_cursor_input_model(test_case)
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]] = {}
    for source_name, column_names in test_case.source_columns.items():
        columns: list[ColumnInfo] = []
        for name in column_names:
            columns.append(ColumnInfo(name=name, type=""))
        source_warehouse_columns[source_name] = tuple(columns)

    validate_source_cursor_input_columns(
        model=model,
        cursor_column=test_case.cursor_column,
        models_by_name=build_cursor_input_contract_models(test_case),
        source_map=build_cursor_input_contract_sources(test_case),
        source_warehouse_columns=source_warehouse_columns,
    )

    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        WatermarkLineageTestCase(
            description="direct view filter resolves transitive physical source watermark",
            watermark_name="raw_orders",
            watermark_column="loaded_at",
            expected_valid=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_view_filter_and_transitive_watermark_when_planning_then_relation_is_physical(
    test_case: WatermarkLineageTestCase,
) -> None:
    model: CompiledModelView
    models_by_name: dict[str, CompiledModelView]
    model, models_by_name = build_transitive_watermark_models(
        watermark_name=test_case.watermark_name,
        watermark_column=test_case.watermark_column,
    )
    view_model: CompiledModelView = models_by_name["orders_view"]

    relations: tuple[CursorInputRelation, ...] = build_cursor_input_relations(
        model=model,
        dialect=Dialect(DuckDbAdapter()),
        model_locations={view_model.name: view_model.destination},
        models_by_name=models_by_name,
        seed_locations={},
        source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw", table="raw_orders")},
        cursor_column="event_time",
        runtime_cursor_producer_names=frozenset({"raw_orders"}),
    )

    assert len(relations) == 1
    assert relations[0].relation == "raw.raw_orders"
    assert relations[0].cursor_column == "loaded_at"
    assert relations[0].is_runtime_produced is True
    assert "orders_view" not in relations[0].relation
    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        SourceCursorInputColumnsTestCase(
            description="raises when source cursor input column is missing",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "missing_loaded_at"},
            source_columns={"raw_orders": ("order_id", "event_time")},
            expected_valid=False,
            expected_error_fragment=(
                "model 'test_model': cursor_inputs references source 'raw_orders' column "
                "'missing_loaded_at'"
            ),
        ),
        SourceCursorInputColumnsTestCase(
            description="raises when enforced model contract omits cursor input column",
            reference_kind=SqlReferenceKind.REF,
            reference_name="stg_orders",
            cursor_column="event_time",
            cursor_inputs={"stg_orders": "missing_loaded_at"},
            source_columns={},
            upstream_contract="enforced",
            upstream_declared_columns=("order_id", "event_time"),
            expected_valid=False,
            expected_error_fragment="model contract does not expose the column",
        ),
        SourceCursorInputColumnsTestCase(
            description="raises when enforced source contract omits cursor input column",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "missing_loaded_at"},
            source_columns={},
            upstream_contract="enforced",
            upstream_declared_columns=("order_id", "event_time"),
            expected_valid=False,
            expected_error_fragment="source contract does not expose the column",
        ),
        SourceCursorInputColumnsTestCase(
            description="raises when enforced source contract omits physical extra column",
            reference_kind=SqlReferenceKind.SOURCE,
            reference_name="raw_orders",
            cursor_column="event_time",
            cursor_inputs={"raw_orders": "extra_loaded_at"},
            source_columns={"raw_orders": ("order_id", "extra_loaded_at")},
            upstream_contract="enforced",
            upstream_declared_columns=("order_id",),
            expected_valid=False,
            expected_error_fragment="source contract does not expose the column",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_cursor_input_source_column_when_validating_then_raises_clear_error(
    test_case: SourceCursorInputColumnsTestCase,
) -> None:
    model: CompiledModelView = build_source_cursor_input_model(test_case)
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]] = {}
    for source_name, column_names in test_case.source_columns.items():
        columns: list[ColumnInfo] = []
        for name in column_names:
            columns.append(ColumnInfo(name=name, type=""))
        source_warehouse_columns[source_name] = tuple(columns)
    assert test_case.expected_error_fragment is not None

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        validate_source_cursor_input_columns(
            model=model,
            cursor_column=test_case.cursor_column,
            models_by_name=build_cursor_input_contract_models(test_case),
            source_map=build_cursor_input_contract_sources(test_case),
            source_warehouse_columns=source_warehouse_columns,
        )

    assert test_case.expected_valid is False


@pytest.mark.parametrize(
    "test_case",
    [
        WatermarkLineageTestCase(
            description="transitive source watermark belongs to upstream lineage",
            watermark_name="raw_orders",
            watermark_column="loaded_at",
            expected_valid=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_transitive_watermark_when_validating_lineage_then_accepts_upstream_source(
    test_case: WatermarkLineageTestCase,
) -> None:
    model: CompiledModelView
    models_by_name: dict[str, CompiledModelView]
    model, models_by_name = build_transitive_watermark_models(
        watermark_name=test_case.watermark_name,
        watermark_column=test_case.watermark_column,
    )

    validate_source_cursor_input_columns(
        model=model,
        cursor_column="event_time",
        models_by_name=models_by_name,
        source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw")},
        source_warehouse_columns={"raw_orders": (ColumnInfo(name="loaded_at", type="TIMESTAMP"),)},
    )
    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        WatermarkLineageTestCase(
            description="table function resolves transitive physical source watermark",
            watermark_name="raw_orders",
            watermark_column="loaded_at",
            expected_valid=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_and_transitive_watermark_when_planning_then_source_is_accepted(
    test_case: WatermarkLineageTestCase,
) -> None:
    model: CompiledModelView
    models_by_name: dict[str, CompiledModelView]
    functions_by_name: dict[str, CompiledFunctionView]
    model, models_by_name, functions_by_name = build_function_transitive_watermark_models(
        watermark_name=test_case.watermark_name,
        watermark_column=test_case.watermark_column,
    )

    validate_source_cursor_input_columns(
        model=model,
        cursor_column="event_time",
        models_by_name=models_by_name,
        functions_by_name=functions_by_name,
        source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw")},
        source_warehouse_columns={"raw_orders": (ColumnInfo(name="loaded_at", type="TIMESTAMP"),)},
    )
    relations: tuple[CursorInputRelation, ...] = build_cursor_input_relations(
        model=model,
        dialect=Dialect(DuckDbAdapter()),
        model_locations={name: upstream.destination for name, upstream in models_by_name.items()},
        models_by_name=models_by_name,
        functions_by_name=functions_by_name,
        seed_locations={},
        source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw", table="raw_orders")},
        cursor_column="event_time",
        runtime_cursor_producer_names=frozenset(),
    )

    assert len(relations) == 1
    assert relations[0].relation == "raw.raw_orders"
    assert relations[0].cursor_column == "loaded_at"
    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        WatermarkLineageTestCase(
            description="unrelated watermark relation is rejected",
            watermark_name="unrelated_orders",
            watermark_column="loaded_at",
            expected_valid=False,
            expected_error_fragment="not in the model's upstream lineage",
        ),
        WatermarkLineageTestCase(
            description="transitive watermark missing its cursor column is rejected",
            watermark_name="raw_orders",
            watermark_column="missing_loaded_at",
            expected_valid=False,
            expected_error_fragment="does not expose the column",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_watermark_when_validating_lineage_then_raises_precise_error(
    test_case: WatermarkLineageTestCase,
) -> None:
    model: CompiledModelView
    models_by_name: dict[str, CompiledModelView]
    model, models_by_name = build_transitive_watermark_models(
        watermark_name=test_case.watermark_name,
        watermark_column=test_case.watermark_column,
    )
    assert test_case.expected_error_fragment is not None

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        validate_source_cursor_input_columns(
            model=model,
            cursor_column="event_time",
            models_by_name=models_by_name,
            source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw")},
            source_warehouse_columns={
                "raw_orders": (ColumnInfo(name="loaded_at", type="TIMESTAMP"),)
            },
        )

    assert test_case.expected_valid is False


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorProducerNamesTestCase(
            description="only selected scheduled relation producers own runtime watermarks",
            selected_keys=frozenset(
                {
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.MODEL, name="selected_model"
                    ),
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.SEED, name="selected_seed"
                    ),
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.SOURCE, name="loaded_source"
                    ),
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.SOURCE, name="static_source"
                    ),
                }
            ),
            source_map={
                "loaded_source": SourceEntry(name="loaded_source", loader="load_orders"),
                "static_source": SourceEntry(name="static_source"),
                "unselected_loaded_source": SourceEntry(
                    name="unselected_loaded_source", loader="load_orders"
                ),
            },
            expected_names=frozenset({"selected_model", "selected_seed", "loaded_source"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_selected_relations_when_classifying_runtime_watermarks_then_only_producers_own(
    test_case: RuntimeCursorProducerNamesTestCase,
) -> None:
    scope: PlannerScope = PlannerScope(
        upstream_deps={},
        downstream_deps={},
        all_keys={},
        models_by_name={},
        selected_keys=test_case.selected_keys,
        execution_order=(),
    )

    names: frozenset[str] = runtime_cursor_producer_names(
        scope=scope,
        source_map=test_case.source_map,
    )

    assert names == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    [
        CursorRuntimeOwnershipTestCase(
            description="selected model",
            is_model_backed=True,
            is_runtime_produced=True,
            expected_runtime_owned=True,
        ),
        CursorRuntimeOwnershipTestCase(
            description="unselected model",
            is_model_backed=True,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
        CursorRuntimeOwnershipTestCase(
            description="deferred model",
            is_model_backed=True,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
        CursorRuntimeOwnershipTestCase(
            description="selected seed",
            is_model_backed=False,
            is_runtime_produced=True,
            expected_runtime_owned=True,
        ),
        CursorRuntimeOwnershipTestCase(
            description="unselected seed",
            is_model_backed=False,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
        CursorRuntimeOwnershipTestCase(
            description="deferred seed",
            is_model_backed=False,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
        CursorRuntimeOwnershipTestCase(
            description="selected managed loader",
            is_model_backed=False,
            is_runtime_produced=True,
            expected_runtime_owned=True,
        ),
        CursorRuntimeOwnershipTestCase(
            description="unselected managed loader",
            is_model_backed=False,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
        CursorRuntimeOwnershipTestCase(
            description="static source",
            is_model_backed=False,
            is_runtime_produced=False,
            expected_runtime_owned=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_relation_metadata_when_checking_runtime_ownership_then_only_produced_owns(
    test_case: CursorRuntimeOwnershipTestCase,
) -> None:
    relation: CursorInputRelation = CursorInputRelation(
        relation="warehouse.input",
        cursor_column="event_time",
        is_model_backed=test_case.is_model_backed,
        is_runtime_produced=test_case.is_runtime_produced,
    )

    assert relation.is_runtime_owned is test_case.expected_runtime_owned


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
