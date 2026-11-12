from __future__ import annotations

from pathlib import Path
from typing import cast

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    DynamicColumnContractProof,
    InferredColumn,
    InferredNullability,
    SchemaAuditInstance,
    SchemaColumn,
    SchemaDynamicColumnFamily,
    SchemaModelEntry,
    SettingsConfig,
    SourceLocation,
)
from sqlbuild.compiler.compile.types import CompiledProjectView


def make_contract_project(
    *,
    declared_columns: tuple[tuple[str, str | None], ...],
    inferred_columns: tuple[tuple[str, str | None], ...] | None,
    type_enforcement: bool | None,
    contract: str | None = None,
    check_declared_columns: bool = True,
    model_name: str = "orders",
    column_locations: dict[str, SourceLocation] | None = None,
    not_null_audit_columns: tuple[str, ...] = (),
    declared_nullable_by_column: dict[str, bool | None] | None = None,
    inferred_nullability_by_column: dict[str, InferredNullability] | None = None,
    fast_lineage_has_star: bool = False,
    authored_sql: str = "",
    dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = (),
    dynamic_column_contract: DynamicColumnContractProof | None = None,
) -> CompiledProjectView:
    """Build a compiled project for contract validation tests."""

    key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name=model_name,
    )
    return CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        settings=SettingsConfig(check_declared_columns=check_declared_columns),
        models=(
            CompiledModel(
                key=key,
                deps=(),
                name=model_name,
                relative_path=Path(f"models/{model_name}.sql"),
                query_sql="SELECT 1 AS id",
                config=CompileModelConfig(
                    values=cast(
                        dict[str, object],
                        ({}, {"contract": contract})[contract is not None],
                    )
                ),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name=model_name,
                    qualified_name=f"analytics.{model_name}",
                ),
                schema_entry=SchemaModelEntry(
                    name=model_name,
                    type_enforcement=type_enforcement,
                    columns=tuple(
                        SchemaColumn(
                            name=name,
                            type=column_type,
                            audits=_column_audits(
                                name=name,
                                not_null_audit_columns=not_null_audit_columns,
                            ),
                            nullable=(declared_nullable_by_column or {}).get(name),
                            location=(column_locations or {}).get(name),
                        )
                        for name, column_type in declared_columns
                    ),
                    dynamic_columns=dynamic_columns,
                ),
                inferred_columns=(
                    None,
                    tuple(
                        InferredColumn(
                            name=name,
                            type=column_type,
                            nullability=(inferred_nullability_by_column or {}).get(
                                name, InferredNullability.UNKNOWN
                            ),
                        )
                        for name, column_type in (inferred_columns or ())
                    ),
                )[inferred_columns is not None],
                fast_lineage_has_star=fast_lineage_has_star,
                authored_sql=authored_sql,
                dynamic_column_contract=dynamic_column_contract,
            ),
        ),
    )


def _column_audits(
    *, name: str, not_null_audit_columns: tuple[str, ...]
) -> tuple[SchemaAuditInstance, ...]:
    return ((), (SchemaAuditInstance(definition_name="not_null"),))[name in not_null_audit_columns]


def make_promotion_project(
    *,
    model_configs: tuple[tuple[str, dict[str, object]], ...],
    table_promotion_mode: str | None,
) -> CompiledProjectView:
    """Build a compiled project of models with the given configs and promotion setting."""

    return CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        settings=SettingsConfig(table_promotion_mode=table_promotion_mode),
        models=tuple(
            CompiledModel(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
                deps=(),
                name=name,
                relative_path=Path(f"models/{name}.sql"),
                query_sql="SELECT 1 AS id",
                config=CompileModelConfig(values=values),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name=name,
                    qualified_name=f"analytics.{name}",
                ),
            )
            for name, values in model_configs
        ),
    )
