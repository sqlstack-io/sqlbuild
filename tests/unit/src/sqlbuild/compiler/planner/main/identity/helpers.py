import json
from dataclasses import replace
from datetime import UTC, datetime
from pathlib import Path
from types import SimpleNamespace

from sqlbuild._native import (
    CompiledFunction,
    CompiledModel,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    DirectModelVersionIdentities,
    Fingerprint,
    FunctionArgument,
    FunctionReturnColumn,
    PlannerScope,
    SchemaColumn,
    SeedCsvSettings,
    WarehouseFingerprints,
    build_direct_model_version_identities,
    build_downstream_deps,
    build_model_version_identity_metadata_json,
    build_version_identity_metadata_json,
    compiled_model_record,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    FunctionLanguage,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import build_compiled_function


def build_version_identity_config(config_values: dict[str, object]) -> dict[str, object]:
    """Return the config entries recorded in a model's version identity metadata."""

    metadata: dict[str, object] = json.loads(
        build_version_identity_metadata_json(model_name="orders", config_values=config_values)
    )
    config: dict[str, object] = metadata["config"]
    return config


def build_table_function_graph_identities(*, base_query: str) -> DirectModelVersionIdentities:
    base_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.TABLE_FN,
        name="customer_orders",
    )
    consumer_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="customer_order_summary",
    )
    base_model: CompiledModelView = _build_model(
        key=base_key,
        query_sql=base_query,
        deps=(),
    )
    consumer_model: CompiledModelView = _build_model(
        key=consumer_key,
        query_sql='SELECT * FROM __table_fn("customer_orders")()',
        deps=(function_key,),
    )
    function: CompiledFunction = replace(
        build_compiled_function(body_sql='SELECT * FROM __ref("orders")'),
        key=function_key,
        deps=(base_key,),
        name=function_key.name,
        returns="TABLE",
        return_columns=(FunctionReturnColumn(name="order_id", type="INTEGER"),),
    )
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        base_key: (),
        function_key: (base_key,),
        consumer_key: (function_key,),
    }
    scope: PlannerScope = PlannerScope(
        upstream_deps=upstream,
        downstream_deps=build_downstream_deps(upstream),
        all_keys={key.name: key for key in upstream},
        models_by_name={
            base_model.name: base_model,
            consumer_model.name: consumer_model,
        },
        selected_keys=frozenset(upstream),
        execution_order=(base_key, function_key, consumer_key),
    )
    return build_direct_model_version_identities(functions=(function,), scope=scope, dialect=None)


def _build_model(
    *,
    key: CompiledObjectKey,
    query_sql: str,
    deps: tuple[CompiledObjectKey, ...],
) -> CompiledModelView:
    return CompiledModel(
        key=key,
        deps=deps,
        name=key.name,
        relative_path=Path(f"models/{key.name}.sql"),
        query_sql=query_sql,
        config=CompileModelConfig(),
        destination=CompiledRelationLocation(
            database=None,
            schema=None,
            name=key.name,
            qualified_name=key.name,
        ),
    )


def build_hooked_model_identity_json(*, hook_config: dict[str, object]) -> str:
    model: CompiledModelView = replace(
        compiled_model_record(
            _build_model(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
                query_sql="SELECT 1 AS order_id",
                deps=(),
            )
        ),
        config=CompileModelConfig(values=hook_config),
    )
    return build_model_version_identity_metadata_json(
        model=model,
        hook_version_hashes={"notify_orders": "notify-hash-v1"},
    )


def build_contract_model() -> SimpleNamespace:
    """Build a model view with one enforced declared column."""

    return SimpleNamespace(
        config=SimpleNamespace(values={"contract": "enforced"}),
        schema_entry=SimpleNamespace(
            columns=(SchemaColumn(name="id", type="int", nullable=False),),
            dynamic_columns=(),
            type_enforcement=True,
        ),
    )


def build_identity_function() -> SimpleNamespace:
    """Build a SQL scalar function view with one argument."""

    return SimpleNamespace(
        name="add_one",
        arguments=(FunctionArgument(name="x", type="int"),),
        returns="int",
        return_columns=(),
        body_sql="x + 1",
        language=FunctionLanguage.SQL,
        runtime_version=None,
        entry_point=None,
        packages=(),
    )


def build_identity_seed(directory: Path) -> SimpleNamespace:
    """Build a seed view over a two-row CSV file with Windows line endings."""

    file_path: Path = directory / "regions.csv"
    file_path.write_text("id,name\r\n1,café\r\n", encoding="utf-8")
    return SimpleNamespace(
        name="regions",
        external=False,
        seed_file=SimpleNamespace(file_path=file_path),
        schema_entry=SimpleNamespace(
            csv_settings=SeedCsvSettings(),
            columns=(SchemaColumn(name="id", type="int"),),
        ),
    )


def build_chain_graph_identities(*, depth: int) -> DirectModelVersionIdentities:
    """Build identities of a model chain resolved from its most downstream model."""

    keys: list[CompiledObjectKey] = [
        CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=f"m{index}")
        for index in range(depth)
    ]
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        key: tuple(keys[index - 1 : index]) for index, key in enumerate(keys)
    }
    scope: PlannerScope = PlannerScope(
        upstream_deps=upstream,
        downstream_deps={},
        all_keys={key.name: key for key in keys},
        models_by_name={
            key.name: _build_model(key=key, query_sql="SELECT 1 AS id", deps=upstream[key])
            for key in keys
        },
        selected_keys=frozenset(keys),
        execution_order=tuple(reversed(keys)),
    )
    return build_direct_model_version_identities(functions=(), scope=scope, dialect=None)


_RECORDED_TS: datetime = datetime(2026, 1, 15, 12, 0, 0, tzinfo=UTC)


def recorded_fingerprint(
    *, name: str, node_type: str, definition: str, definition_hash: str, metadata_json: str = "{}"
) -> Fingerprint:
    """A fingerprint row as an earlier SQLBuild version recorded it."""

    return Fingerprint(
        node_type=node_type,
        node_name=name,
        target_database=None,
        target_schema=None,
        target_name=name,
        run_id="run_001",
        definition_hash=definition_hash,
        schema_fingerprint="schema_a",
        definition=definition,
        ts=_RECORDED_TS,
        metadata_json=metadata_json,
        version_hash="recorded_version",
    )


def build_two_model_graph_identities(
    *, orders_sql: str, summary_sql: str, recorded: WarehouseFingerprints | None
) -> DirectModelVersionIdentities:
    """Identities of `orders` and its downstream `orders_summary`, optionally over recorded rows."""

    orders_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="orders"
    )
    summary_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="orders_summary"
    )
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        orders_key: (),
        summary_key: (orders_key,),
    }
    scope: PlannerScope = PlannerScope(
        upstream_deps=upstream,
        downstream_deps=build_downstream_deps(upstream),
        all_keys={key.name: key for key in upstream},
        models_by_name={
            "orders": _build_model(key=orders_key, query_sql=orders_sql, deps=()),
            "orders_summary": _build_model(
                key=summary_key, query_sql=summary_sql, deps=(orders_key,)
            ),
        },
        selected_keys=frozenset(upstream),
        execution_order=(orders_key, summary_key),
    )
    return build_direct_model_version_identities(
        functions=(), scope=scope, dialect=None, recorded_fingerprints=recorded
    )


def build_function_identities(
    *, body_sql: str, recorded: WarehouseFingerprints | None
) -> DirectModelVersionIdentities:
    """Identities of one SQL function and a model calling it, optionally over recorded rows."""

    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF, name="add_one"
    )
    model_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="orders"
    )
    function: CompiledFunction = replace(
        build_compiled_function(body_sql=body_sql), key=function_key, name="add_one"
    )
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        function_key: (),
        model_key: (function_key,),
    }
    scope: PlannerScope = PlannerScope(
        upstream_deps=upstream,
        downstream_deps=build_downstream_deps(upstream),
        all_keys={key.name: key for key in upstream},
        models_by_name={
            "orders": _build_model(
                key=model_key, query_sql="SELECT add_one(1) AS order_id", deps=(function_key,)
            )
        },
        selected_keys=frozenset(upstream),
        execution_order=(function_key, model_key),
    )
    return build_direct_model_version_identities(
        functions=(function,), scope=scope, dialect=None, recorded_fingerprints=recorded
    )


def old_scheme_hash(name: str) -> str:
    """The definition hash an older fingerprint scheme recorded for `name`."""

    return f"old_scheme_{name}"
