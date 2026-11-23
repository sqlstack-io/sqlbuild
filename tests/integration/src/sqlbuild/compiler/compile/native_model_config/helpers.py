"""Validate one model's effective config with a native model config session."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import sqlbuild._native as _native
from sqlbuild._native import (
    CompileModelConfig,
    CompileSqlReference,
    DiscoveredSqlModelFile,
    ModelValidationRequest,
    NativeModelConfigInputs,
    NativeModelConfigSession,
    ProjectConfig,
    SqlReferenceKind,
    native_model_config_session,
)

_ACCEPTED: str = "accepted"


def validation_outcome(*, values: dict[str, object]) -> object:
    """Return `accepted`, or the raised error's shape, for model `orders_daily` reading orders."""

    session: NativeModelConfigSession = native_model_config_session(
        inputs=NativeModelConfigInputs(
            project_config=ProjectConfig(name="orders", adapter="duckdb"),
            target_config=None,
            effective_vars={},
            effective_target_name=None,
            run_id="20261008T000000Z_orders",
            microbatch_concurrency=False,
        ),
        validator=_native.NativeModelValidator(
            (
                {"orders", "customers"},
                {"regions"},
                {"raw.orders"},
                {"cents", "order_lines"},
                {"order_lines"},
            ),
            {"ledger"},
            False,
        ),
    )
    request: ModelValidationRequest = ModelValidationRequest(
        model_file=DiscoveredSqlModelFile(
            file_path=Path("/project/models/marts/orders_daily.sql"),
            relative_path=Path("sql/models/marts/orders_daily.sql"),
            contents="",
            header_values={},
            header_column_locations={},
            output_column_locations={},
            query_sql="SELECT 1",
        ),
        config=CompileModelConfig(values=values),
        references=(CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name="orders"),),
        declared_columns=None,
        query_sql="SELECT 1",
    )
    outcome: _native.NativeConfigError | None = cast(
        _native.NativeConfigError | None,
        session.validator.validate(
            request.config.values,
            (
                request.model_file.file_path.stem,
                request.query_sql,
                str(request.model_file.relative_path),
            ),
            (
                [
                    (str(reference.ref_kind), reference.ref_name, False)
                    for reference in request.references
                ],
                None,
                request.config.time_travel_retention.unmanaged,
                request.config.table_type.declared,
            ),
        ),
    )
    return [
        _ACCEPTED,
        *(raised_error_shape(error.exception(values=values)) for error in filter(None, (outcome,))),
    ][-1]


def header_help(*, purpose: str, entry: str) -> str:
    """Return the help that shows the exact MODEL header entry to add."""

    indent: str = " " * 12
    return (
        f"{purpose}, add this to the MODEL header:\n{indent}MODEL (\n"
        f"{indent}  {entry},\n{indent}  ...\n{indent});"
    )


def raised_error_shape(error: Exception) -> tuple[object, ...]:
    """Return the type, message, code and help of an error."""

    return (
        type(error).__name__,
        str(error),
        getattr(error, "code", None),
        getattr(error, "help", None),
    )
