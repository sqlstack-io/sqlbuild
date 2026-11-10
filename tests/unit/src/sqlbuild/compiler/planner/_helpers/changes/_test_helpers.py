"""Test helpers for change detection tests."""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import cast

from sqlbuild._native import (
    ChangeKind,
    CompiledFunction,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    Fingerprint,
    FunctionArgument,
    PlannerScope,
    SchemaColumn,
    SchemaModelEntry,
    WarehouseFingerprints,
    WarehouseSnapshot,
    build_version_identity_metadata_json,
    compute_query_hash,
)
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.compiler.compile.types import (
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
    FunctionLanguage,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.changes._test_types import (
    DetectModelChangesTestCase,
    DetectModelMetadataTestCase,
)

_STUB_TS: datetime = datetime(2026, 1, 15, 12, 0, 0)


def build_model_from_test_case(test_case: DetectModelChangesTestCase) -> CompiledModelView:
    """Build a CompiledModel from a test case."""

    schema_entry: SchemaModelEntry | None = _build_schema_entry(test_case)
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=test_case.model_name),
        deps=(),
        name=test_case.model_name,
        relative_path=Path(f"models/{test_case.model_name}.sql"),
        query_sql=test_case.query_sql,
        config=CompileModelConfig(values=test_case.config_values),
        destination=CompiledRelationLocation(
            database=None, schema="staging", name=test_case.model_name, qualified_name=None
        ),
        schema_entry=schema_entry,
        inferred_columns=test_case.inferred_columns,
        fast_lineage_has_star=test_case.fast_lineage_has_star,
    )


def build_model_from_metadata_test_case(
    test_case: DetectModelMetadataTestCase,
) -> CompiledModelView:
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.UDF, name=dep_name)
            for dep_name in test_case.deps
        ),
        name="orders",
        relative_path=Path("models/orders.sql"),
        query_sql="SELECT 1 AS order_id",
        config=CompileModelConfig(values=test_case.config_values),
        destination=CompiledRelationLocation(
            database=None, schema="staging", name="orders", qualified_name=None
        ),
        schema_entry=(
            None,
            SchemaModelEntry(
                name="orders",
                columns=tuple(
                    SchemaColumn(name=column[0], type=column[1], nullable=column[2])
                    for column in test_case.schema_columns
                ),
            ),
        )[bool(test_case.schema_columns)],
    )


def build_snapshot_for_metadata_test_case(
    test_case: DetectModelMetadataTestCase,
) -> WarehouseSnapshot:
    return WarehouseSnapshot(
        existing_relations={
            "orders": RelationInfo(
                database=None,
                schema="staging",
                name="orders",
                relation_type="BASE TABLE",
            )
        },
        existing_columns={},
        fingerprints=WarehouseFingerprints(
            models={
                "orders": Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash=compute_query_hash(
                        query_sql="SELECT 1 AS order_id", dialect=None
                    ),
                    schema_fingerprint="schema_a",
                    definition="SELECT 1 AS order_id",
                    metadata_json=test_case.previous_metadata_json,
                    ts=_STUB_TS,
                )
            }
        ),
    )


def build_metadata_json_with_audit_gate(metadata_json: str) -> str:
    metadata: dict[str, object] = json.loads(metadata_json)
    metadata["audit_gate"] = {
        "status": "passed",
        "binding_set_hash": "binding_hash",
        "blocking_set_hash": "blocking_hash",
        "mode": "executed",
        "run_id": "run_001",
        "results": [],
    }
    return json.dumps(metadata, sort_keys=True, separators=(",", ":"), default=str)


def build_project_for_function_metadata_detection() -> CompiledProjectView:
    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF,
        name="is_large_order",
    )
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(function_key,),
        name="orders",
        relative_path=Path("models/orders.sql"),
        query_sql="SELECT is_large_order(amount) AS large_order FROM orders",
        config=CompileModelConfig(values={}),
        destination=CompiledRelationLocation(
            database=None, schema="staging", name="orders", qualified_name=None
        ),
    )
    function_destination: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="staging",
        name="is_large_order",
        qualified_name=None,
    )
    function: CompiledFunctionView = CompiledFunction(
        key=function_key,
        deps=(),
        name="is_large_order",
        relative_path=Path("functions/is_large_order.sql"),
        arguments=(FunctionArgument(name="amount", type="INTEGER"),),
        returns="BOOLEAN",
        body_sql="amount > 100",
        destination=function_destination,
        fingerprint_destination=function_destination,
        language=FunctionLanguage.SQL,
    )
    return CompiledProject(
        run_id="run_001",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(model,),
        functions=(function,),
    )


def build_scope_for_function_metadata_detection() -> PlannerScope:
    model_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF,
        name="is_large_order",
    )
    project: CompiledProjectView = build_project_for_function_metadata_detection()
    return PlannerScope(
        upstream_deps={model_key: (function_key,), function_key: ()},
        downstream_deps={function_key: (model_key,), model_key: ()},
        all_keys={"orders": model_key, "is_large_order": function_key},
        models_by_name={"orders": project.models[0]},
        selected_keys=frozenset({model_key}),
        execution_order=(function_key, model_key),
    )


def build_snapshot_from_test_case(test_case: DetectModelChangesTestCase) -> WarehouseSnapshot:
    """Build a WarehouseSnapshot from a test case."""

    relations: dict[str, RelationInfo] = _build_relations(test_case)
    columns: dict[str, tuple[ColumnInfo, ...]] = _build_columns(test_case)
    fingerprints: dict[str, Fingerprint] = _build_fingerprints(test_case)
    return WarehouseSnapshot(
        existing_relations=relations,
        existing_columns=columns,
        fingerprints=WarehouseFingerprints(models=fingerprints),
        renamed_models=(frozenset(), frozenset({test_case.model_name}))[
            test_case.renamed_by_migration
        ],
    )


def _build_schema_entry(test_case: DetectModelChangesTestCase) -> SchemaModelEntry | None:
    schema_cols: tuple[tuple[str, str | None], ...] = test_case.schema_columns
    schema_entry: SchemaModelEntry = SchemaModelEntry(
        name=test_case.model_name,
        columns=tuple(SchemaColumn(name=c[0], type=c[1]) for c in schema_cols),
        type_enforcement=test_case.schema_type_enforcement,
    )
    return (None, schema_entry)[bool(schema_cols)]


def _build_relations(test_case: DetectModelChangesTestCase) -> dict[str, RelationInfo]:
    relations: dict[str, RelationInfo] = {
        test_case.model_name: RelationInfo(
            database=None,
            schema="staging",
            name=test_case.model_name,
            relation_type="BASE TABLE",
        )
    }
    return ({}, relations)[test_case.relation_exists]


def _build_columns(
    test_case: DetectModelChangesTestCase,
) -> dict[str, tuple[ColumnInfo, ...]]:
    columns: dict[str, tuple[ColumnInfo, ...]] = {
        test_case.model_name: tuple(
            ColumnInfo(name=c[0], type=c[1]) for c in test_case.warehouse_column_names
        )
    }
    return ({}, columns)[bool(test_case.warehouse_column_names)]


def _build_fingerprints(test_case: DetectModelChangesTestCase) -> dict[str, Fingerprint]:
    fingerprint_config_values: dict[str, object] = cast(
        dict[str, object],
        (test_case.config_values, test_case.fingerprint_config_values)[
            test_case.fingerprint_config_values is not None
        ],
    )
    fingerprints: dict[str, Fingerprint] = {
        test_case.model_name: Fingerprint(
            node_type="model",
            node_name=test_case.model_name,
            target_database=None,
            target_schema=None,
            target_name=test_case.model_name,
            run_id="run_001",
            definition_hash=test_case.fingerprint_query_hash or "",
            schema_fingerprint="schema_a",
            definition=test_case.fingerprint_definition,
            metadata_json=json.dumps(
                {
                    **json.loads(
                        build_version_identity_metadata_json(
                            model_name=test_case.model_name,
                            config_values=fingerprint_config_values,
                        )
                    ),
                    **test_case.fingerprint_extra_metadata,
                },
                sort_keys=True,
                separators=(",", ":"),
            ),
            ts=_STUB_TS,
            version_hash="recorded_version",
        )
    }
    return ({}, fingerprints)[test_case.fingerprint_query_hash is not None]


def build_function_caller_metadata_case(
    *,
    config_values: dict[str, object],
    function_local_hashes: dict[str, str],
    previous_function_hashes: dict[str, str],
    previous_config_values: dict[str, object],
) -> DetectModelMetadataTestCase:
    """Build metadata detection inputs for a model calling the given functions."""

    return DetectModelMetadataTestCase(
        description="caller metadata",
        config_values=config_values,
        schema_columns=(),
        deps=tuple(function_local_hashes),
        function_local_hashes=function_local_hashes,
        previous_metadata_json=build_version_identity_metadata_json(
            model_name="orders",
            config_values=previous_config_values,
            local_function_hashes=previous_function_hashes,
        ),
        expected_change_kind=ChangeKind.FUNCTION_CHANGED,
    )
