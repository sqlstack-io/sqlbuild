"""Generated contract inputs, and the native contract results they produce."""

from __future__ import annotations

import random
from collections import Counter
from collections.abc import Callable, Sequence
from dataclasses import replace
from pathlib import Path
from typing import Any, NamedTuple, TypeGuard

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    CompilerDiagnostic,
    ContractPolicy,
    ContractValidationResult,
    DynamicColumnContractProof,
    DynamicColumnFamilyProof,
    IncrementalMode,
    InferredColumn,
    InferredNullability,
    MaterializationType,
    SchemaColumn,
    SchemaDynamicColumnFamily,
    SchemaModelEntry,
    SourceLocation,
    build_project_graph,
    compiled_model_record,
    compiled_project_record,
    discover_project_inputs,
)
from sqlbuild.adapter.contract.types import TypeDialect
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView

_PROJECT_TOML: str = (
    'name = "orders_contracts"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SOURCES: str = """sources:
  - name: raw_orders
    description: Orders feed.
    expression: >-
      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status)
    columns:
      - name: order_id
        type: INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR
"""
_MODELS: dict[str, str] = {
    "models/staging/stg_orders.sql": (
        'MODEL (\n  description "Staged orders",\n  contract enforced,\n  columns (\n'
        "    order_id (type INTEGER, nullable false),\n    amount (type DOUBLE),\n  ),\n);\n\n"
        'SELECT order_id, customer_id, amount, status FROM __source("raw_orders")\n'
    ),
    "models/marts/customer_totals.sql": (
        'MODEL (\n  description "Totals per customer",\n  columns (\n'
        "    customer_id (type BIGINT),\n    total_amount (type DECIMAL(18, 2)),\n  ),\n);\n\n"
        "SELECT customer_id, SUM(amount) AS total_amount, COUNT(*) AS order_count,\n"
        "  MAX(status) AS last_status\n"
        'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
    ),
    "models/marts/order_flags.sql": (
        'MODEL (\n  description "Order flags",\n);\n\n'
        "SELECT order_id, amount > 10 AS is_large, CAST(NULL AS VARCHAR) AS note,\n"
        "  status AS Status_Label\n"
        'FROM __ref("stg_orders")\n'
    ),
    "models/marts/all_orders.sql": (
        'MODEL (\n  description "Every order column",\n);\n\nSELECT * FROM __ref("stg_orders")\n'
    ),
}
TYPE_POOL: tuple[str | None, ...] = (
    None,
    "INTEGER",
    "INT",
    "int4",
    "BIGINT",
    "INT64",
    "SMALLINT",
    "VARCHAR",
    "VARCHAR(10)",
    "varchar(255)",
    "STRING",
    "TEXT",
    "CHAR(3)",
    "DOUBLE",
    "FLOAT",
    "FLOAT64",
    "REAL",
    "NUMERIC",
    "NUMERIC(10, 2)",
    "DECIMAL(18,2)",
    "NUMBER(38, 0)",
    "BOOLEAN",
    "BOOL",
    "DATE",
    "TIMESTAMP",
    "TIMESTAMP_NTZ",
    "TIMESTAMPTZ",
    "TIMESTAMP WITH TIME ZONE",
    "DATETIME",
    "JSON",
    "VARIANT",
    "STRUCT<a INT>",
    "ARRAY<INT>",
    "INTEGER[]",
    "MAP(VARCHAR, INTEGER)",
    "not a type (",
    "",
)
_OUTPUT_NAMES: list[str] = [
    "order_id",
    "customer_id",
    "amount",
    "status",
    "total_amount",
    "order_count",
    "is_large",
    "note",
    "Status_Label",
    "missing_id",
    "Order_ID",
]
_ENFORCED_MODEL: str = "stg_orders"
_NAMED_SCHEMA_PATH: Path = Path("schemas/orders_schema.sql")
_LIFECYCLE_KEYS: tuple[str, ...] = ("contract", "materialized", "incremental_mode")
_FAMILY_NAMES: tuple[str, ...] = ("amount_*", "Amount_*", "count_*", "label_*")
_REASONS: tuple[str | None, ...] = (None, "", "no supported pivot")
_CONTRACTS: tuple[object, ...] = (
    "enforced",
    ContractPolicy.ENFORCED,
    "none",
    ContractPolicy.NONE,
    "ENFORCED",
    None,
    1,
)
_MATERIALIZATIONS: tuple[object, ...] = (
    MaterializationType.TABLE,
    "table",
    MaterializationType.VIEW,
    MaterializationType.INCREMENTAL,
    "incremental",
    MaterializationType.SNAPSHOT,
    None,
)
_INCREMENTAL_MODES: tuple[object, ...] = (
    IncrementalMode.MICROBATCH,
    "microbatch",
    IncrementalMode.FULL,
    None,
)
_PROMOTION_MODES: tuple[str | None, ...] = (None, "immediate", "staged", "IMMEDIATE", "")

type ContractView = tuple[CompilerDiagnostic, ...]
type NativeContractRequest = tuple[str, bool, list[Any]]
_EMPTY_SCHEMA_ROW: tuple[list[Any], list[Any], bool, bool] = ([], [], False, False)


def compiled_contract_project(*, project_dir: Path) -> CompiledProjectView:
    """Write and compile the base project whose models the generators perturb."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sources/raw.yml": _SOURCES,
        **_MODELS,
    }
    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return build_project_graph(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
    ).project


def perturbed_project(*, project: CompiledProjectView, rng: random.Random) -> CompiledProjectView:
    """The project with every model's contract inputs replaced by generated ones."""

    return replace(
        compiled_project_record(project),
        native_project=None,
        settings=replace(
            project.settings,
            check_declared_columns=rng.choice((True, False)),
            table_promotion_mode=rng.choice(_PROMOTION_MODES),
        ),
        models=tuple(perturbed_model(model=model, rng=rng) for model in project.models),
    )


def perturbed_model(*, model: CompiledModelView, rng: random.Random) -> CompiledModelView:
    """One model with generated contract config, declared shape, inference and proof."""

    values: dict[str, object] = dict(model.config.values)
    for key in _LIFECYCLE_KEYS:
        _ = values.pop(key, None)
    for key, options in zip(
        _LIFECYCLE_KEYS, (_CONTRACTS, _MATERIALIZATIONS, _INCREMENTAL_MODES), strict=True
    ):
        values.update(rng.choice(({}, *({key: option} for option in options))))
    return replace(
        compiled_model_record(model),
        config=replace(model.config, values=values),
        schema_entry=rng.choice((None, *(_schema_entry(model=model, rng=rng),) * 9)),
        inferred_columns=rng.choice((None, *(_inferred_columns(rng=rng),) * 9)),
        fast_lineage_has_star=rng.random() < 0.2,
        dynamic_column_contract=rng.choice((None, None, *(_dynamic_proof(rng=rng),) * 5)),
        unchecked_output_columns=frozenset(rng.sample(_OUTPUT_NAMES, k=rng.randint(0, 2))),
    )


def _schema_entry(*, model: CompiledModelView, rng: random.Random) -> SchemaModelEntry:
    columns: tuple[SchemaColumn, ...] = tuple(
        SchemaColumn(
            name=name,
            type=rng.choice(TYPE_POOL),
            nullable=rng.choice((None, False, True)),
            location=rng.choice(
                (
                    None,
                    SourceLocation(path=model.relative_path, line=3, column=5),
                    SourceLocation(path=_NAMED_SCHEMA_PATH, line=2, column=3),
                )
            ),
        )
        for name in rng.sample(_OUTPUT_NAMES, k=rng.randint(0, 4))
    )
    families: tuple[SchemaDynamicColumnFamily, ...] = tuple(
        SchemaDynamicColumnFamily(
            name=name,
            pivot_column="status",
            value_column="amount",
            aggregate="sum",
            type=str(rng.choice(TYPE_POOL[1:])),
        )
        for name in rng.sample(_FAMILY_NAMES, k=rng.choice((0, 0, 1, 2)))
    )
    return SchemaModelEntry(
        name=model.name,
        model_schema=rng.choice((None, None, "orders_schema")),
        type_enforcement=rng.choice((None, False, True)),
        columns=columns,
        dynamic_columns=families,
    )


def _inferred_columns(*, rng: random.Random) -> tuple[InferredColumn, ...]:
    return tuple(
        InferredColumn(
            name=name,
            type=rng.choice(TYPE_POOL),
            nullability=rng.choice(tuple(InferredNullability)),
        )
        for name in rng.choices(_OUTPUT_NAMES, k=rng.randint(0, 6))
    )


def _dynamic_proof(*, rng: random.Random) -> DynamicColumnContractProof:
    return DynamicColumnContractProof(
        output_proven=rng.random() < 0.6,
        failure_reason=rng.choice(_REASONS),
        families=tuple(
            DynamicColumnFamilyProof(
                name=rng.choice((name, name.upper(), name.title())),
                inferred_type=rng.choice(TYPE_POOL),
            )
            for name in rng.sample(_FAMILY_NAMES, k=rng.randint(0, 3))
        ),
    )


def contract_diagnostics(*, project: CompiledProjectView, dialect: str | None) -> ContractView:
    """The project's contract diagnostics, through the public contract entry point."""

    return native_module.evaluate_model_contracts(project=project, dialect=dialect).diagnostics


def diagnostic_view(*, diagnostics: ContractView, project_dir: Path) -> str:
    """Every field of the diagnostics, in order, with the project directory masked."""

    return repr(
        [
            (
                diagnostic.code,
                diagnostic.severity,
                diagnostic.resource_name,
                diagnostic.column_name,
                diagnostic.message,
                diagnostic.location,
                diagnostic.related_locations,
                diagnostic.help,
            )
            for diagnostic in diagnostics
        ]
    ).replace(str(project_dir), "<project>")


class NativeContractRecord(NamedTuple):
    """Native contract outcomes: `native` (evaluated natively), `typed_comparisons` and
    `native_diagnostics` in `statuses`; codes of natively built diagnostics in `codes`."""

    statuses: Counter[str]
    codes: Counter[str]


def record_native_outcomes(*, monkeypatch: pytest.MonkeyPatch) -> NativeContractRecord:
    """Record native contract outcomes for the rest of the test."""

    record: NativeContractRecord = NativeContractRecord(statuses=Counter(), codes=Counter())
    evaluate: Callable[..., ContractValidationResult] = native_module.evaluate_model_contracts

    def counted(
        *, project: CompiledProjectView, dialect: TypeDialect | str | None
    ) -> ContractValidationResult:
        result: ContractValidationResult = evaluate(project=project, dialect=dialect)
        record.statuses.update(contract_statuses(project=project, result=result))
        record.codes.update(diagnostic.code for diagnostic in result.diagnostics)
        return result

    monkeypatch.setattr(native_module, "evaluate_model_contracts", counted)
    return record


def contract_statuses(
    *, project: CompiledProjectView, result: ContractValidationResult
) -> Counter[str]:
    """Counts of evaluated models, typed comparisons and diagnostics for one contract run."""

    request: NativeContractRequest = (
        "",
        project.settings.check_declared_columns,
        [_status_row(model) for model in project.models],
    )
    return native_contract_statuses(request=request, diagnostics=len(result.diagnostics))


def _status_row(model: CompiledModelView) -> tuple[object, ...]:
    contract: object = model.config.values.get("contract")
    return (
        model.name,
        (None, contract)[isinstance(contract, str)],
        next(map(_status_schema, filter(_present, (model.schema_entry,))), None),
        next(map(_status_columns, filter(_present, (model.inferred_columns,))), None),
    )


def _present[T](value: T | None) -> TypeGuard[T]:
    return value is not None


def _status_columns(columns: tuple[InferredColumn, ...]) -> list[tuple[str, str | None, bool]]:
    return [
        (column.name, column.type, column.nullability == InferredNullability.NULLABLE)
        for column in columns
    ]


def _status_schema(schema: SchemaModelEntry) -> tuple[object, ...]:
    return (
        [(column.name, column.type, column.nullable is False, False) for column in schema.columns],
        [(family.name, family.type) for family in schema.dynamic_columns],
        bool(schema.type_enforcement),
    )


def native_contract_statuses(*, request: NativeContractRequest, diagnostics: int) -> Counter[str]:
    """Per-request counts: `native` for evaluated models, `typed_comparisons` for those that
    compared at least one typed column, and `native_diagnostics`."""

    _, implicit, models = request
    evaluated: list[Sequence[Any]] = list(
        filter(lambda model: _requires_evaluation(model=model, implicit=implicit), models)
    )
    statuses: Counter[str] = Counter()
    statuses["native"] += len(evaluated)
    statuses["typed_comparisons"] += sum(
        _compares_typed_column(model=model, implicit=implicit) for model in evaluated
    )
    statuses["native_diagnostics"] += diagnostics
    return statuses


def _schema(model: Sequence[Any]) -> Sequence[Any]:
    return (_EMPTY_SCHEMA_ROW, model[2])[model[2] is not None]


def _shape_validation_active(*, model: Sequence[Any], implicit: bool) -> bool:
    contract: str | None = model[1]
    return contract == "enforced" or (contract != "none" and implicit and bool(_schema(model)[0]))


def _requires_evaluation(*, model: Sequence[Any], implicit: bool) -> bool:
    return _shape_validation_active(model=model, implicit=implicit) or bool(_schema(model)[2])


def _compares_typed_column(*, model: Sequence[Any], implicit: bool) -> bool:
    inferred_types: dict[str, str | None] = {
        name: inferred_type for name, inferred_type, _ in (model[3] or ())
    }
    return (
        any(
            declared_type is not None and inferred_types.get(name) is not None
            for name, declared_type, _, _ in _schema(model)[0]
        )
        and model[3] is not None
    )


def with_declared_type(*, project: CompiledProjectView, declared_type: str) -> CompiledProjectView:
    """The project with its enforced staging model declaring `declared_type` for every column."""

    by_name: dict[str, CompiledModelView] = {model.name: model for model in project.models}
    staging: CompiledModelView = by_name[_ENFORCED_MODEL]
    assert staging.schema_entry is not None
    columns: tuple[SchemaColumn, ...] = tuple(
        replace(column, type=declared_type) for column in staging.schema_entry.columns
    )
    by_name[_ENFORCED_MODEL] = replace(
        compiled_model_record(staging), schema_entry=replace(staging.schema_entry, columns=columns)
    )
    return replace(
        compiled_project_record(project),
        native_project=None,
        models=tuple(by_name[model.name] for model in project.models),
    )
