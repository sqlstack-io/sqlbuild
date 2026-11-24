"""Helpers for model configuration reuse and native validation tests."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, cast

import sqlbuild._native as _native
from sqlbuild._native import (
    CompileModelConfig,
    DefaultsConfig,
    MaterializationDefaultsConfig,
    NativeModelConfigInputs,
    NativeModelConfigSession,
    ProjectConfig,
    ResolvedTableType,
    ResolvedTimeTravelRetention,
    SchemaColumn,
    TableType,
    TableTypeSource,
    TargetConfig,
    TimeTravelRetentionSource,
    build_compile_inputs,
    discover_project_inputs,
    native_model_config_session,
    record_template_reads,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_NO_MATERIALIZATION_DEFAULTS: MaterializationDefaultsConfig = MaterializationDefaultsConfig()


def compile_with_native_config(*, project_dir: Path) -> CompileInputsView:
    """Compile a project's inputs through the native model config stage."""

    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )


def compile_input_schema_columns(inputs: CompileInputsView) -> tuple[SchemaColumn, ...]:
    """Flatten attached schema columns in model input order."""

    columns: list[SchemaColumn] = []
    for model_input in inputs.model_inputs:
        columns.extend(getattr(model_input.schema_entry, "columns", ()))
    return tuple(columns)


def validate_natively(  # noqa: PLR0913
    *,
    config: CompileModelConfig,
    model_name: str = "test_model",
    input_names: frozenset[str] = frozenset(),
    ref_count: int = 0,
    declared_columns: tuple[str, ...] | None = None,
    custom_materialization_names: frozenset[str] = frozenset(),
    query_sql: str = "SELECT 1",
    microbatch_concurrency: bool = False,
) -> None:
    """Run the native model validators, raising their first error.

    The model reads `ref_count` models, the first named from `input_names`; every one exists.
    """

    names: list[str] = sorted(input_names)[:ref_count]
    names.extend(f"upstream_{index}" for index in range(len(names), ref_count))
    validator: _native.NativeModelValidator = _native.NativeModelValidator(
        (set(names), set(), set(), set(), set()),
        set(custom_materialization_names),
        microbatch_concurrency,
    )
    outcome: int | _native.NativeConfigError | None = validator.validate(
        config.values,
        (model_name, query_sql, f"sql/models/{model_name}.sql"),
        (
            [("ref", name, False) for name in names],
            declared_columns,
            config.time_travel_retention.unmanaged,
            config.table_type.declared,
        ),
    )
    assert not isinstance(outcome, int)
    for error in filter(None, (outcome,)):
        raise error.exception(values=config.values)


def build_config_natively(  # noqa: PLR0913
    *,
    defaults: DefaultsConfig,
    path_defaults: dict[str, dict[str, object]],
    matched_path_default: str | None,
    model_header_values: dict[str, object],
    target_config: TargetConfig | None,
    materialization_defaults: MaterializationDefaultsConfig = _NO_MATERIALIZATION_DEFAULTS,
    effective_target_name: str | None = None,
) -> CompileModelConfig:
    """Build the effective config of model `orders` with the native config builder."""

    session: NativeModelConfigSession = native_model_config_session(
        inputs=NativeModelConfigInputs(
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                defaults=defaults,
                path_defaults=path_defaults,
                materialization_defaults=materialization_defaults,
            ),
            target_config=target_config,
            effective_vars={},
            effective_target_name=effective_target_name,
            run_id="run_123",
            microbatch_concurrency=False,
        ),
        validator=_native.NativeModelValidator(
            ({"orders"}, set(), set(), set(), set()), set(), False
        ),
    )
    built: object = session.builder.model_config(
        model_header_values,
        (matched_path_default, "orders"),
        native_config_classes(session=session),
    )
    for error in filter(_is_config_error, (built,)):
        raise cast(_native.NativeConfigError, error).exception()
    return cast(CompileModelConfig, built)


def native_config_classes(*, session: NativeModelConfigSession) -> dict[str, object]:
    """The builder, storage defaults and classes a model's `CompileModelConfig` is made of."""

    return {
        "builder": session.builder,
        "inherited_storage": session.inherited_storage,
        "table_backed": ("table", "incremental", "snapshot"),
        "compile_model_config": CompileModelConfig,
        "resolved_retention": ResolvedTimeTravelRetention,
        "retention_source": TimeTravelRetentionSource.MODEL,
        "resolved_table_type": ResolvedTableType,
        "table_type": TableType,
        "table_type_source": TableTypeSource.MODEL,
        "record_template_reads": record_template_reads,
    }


def _is_config_error(value: object) -> bool:
    return isinstance(value, _native.NativeConfigError)
