"""Returned-row normalization helpers for source loaders."""

from __future__ import annotations

from collections.abc import Iterable, Iterator, Mapping
from datetime import date, datetime
from decimal import Decimal
from types import MappingProxyType

from sqlbuild._native import SourceColumnEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import LoaderLogicalType
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.models import DecimalShape, LoaderRowsSchema

_MAX_DECIMAL_PRECISION: int = 38


def iter_loader_row_batches(
    *, value: object, batch_size: int
) -> Iterator[tuple[dict[str, object], ...]]:
    """Yield normalized loader rows in fixed-size batches."""

    batch: list[dict[str, object]] = []
    row: dict[str, object]
    for row in iter_normalized_loader_rows(value):
        batch.append(row)
        if len(batch) == batch_size:
            yield tuple(batch)
            batch = []
    if batch:
        yield tuple(batch)


def iter_normalized_loader_rows(value: object) -> Iterator[dict[str, object]]:
    """Validate and stream-normalize a loader return value to dict rows."""

    if value is None:
        raise ExecutorInputError("Source loaders that return nothing are not supported yet")
    if isinstance(value, (str, bytes)) or not isinstance(value, Iterable):
        raise ExecutorInputError("Source loaders must return a list or iterable of dict rows")
    item: object
    for item in value:
        if not isinstance(item, Mapping):
            raise ExecutorInputError("Source loaders must return only dict rows")
        yield {str(key): row_value for key, row_value in item.items()}


def update_loader_rows_schema(
    *,
    adapter: BaseAdapter,
    rows: tuple[dict[str, object], ...],
    columns: tuple[SourceColumnEntry, ...],
    column_names: tuple[str, ...],
    inferred_types: dict[str, LoaderLogicalType],
    decimal_shapes: Mapping[str, DecimalShape] = MappingProxyType({}),
    contract_enforced: bool = False,
) -> LoaderRowsSchema:
    """Merge one loader row batch into the tracked staging schema, keeping each decimal's width."""

    declared_types: dict[str, str] = _declared_types(columns=columns)
    names: list[str] = list(column_names)
    next_inferred_types: dict[str, LoaderLogicalType] = dict(inferred_types)
    batch_shapes: dict[str, DecimalShape] = _decimal_shapes(
        rows=rows, declared_types=declared_types
    )
    added_column_names: list[str] = []
    row: dict[str, object]
    for row in rows:
        if contract_enforced:
            _validate_contract_row(
                row=row,
                declared_names=tuple(column.name for column in columns),
            )
        column_name: str
        value: object
        for column_name, value in row.items():
            if column_name not in names:
                names.append(column_name)
                added_column_names.append(column_name)
            if column_name in declared_types:
                continue
            logical_type: LoaderLogicalType | None = _infer_value_type(
                column_name=column_name, value=value
            )
            if logical_type is None and column_name in added_column_names:
                logical_type = LoaderLogicalType.STRING
            if logical_type is None:
                continue
            previous_type: LoaderLogicalType | None = next_inferred_types.get(column_name)
            if previous_type is not None and previous_type != logical_type:
                raise ExecutorInputError(
                    f"Source loader returned conflicting types for column '{column_name}'"
                )
            next_inferred_types[column_name] = logical_type
    next_shapes: dict[str, DecimalShape] = dict(decimal_shapes)
    shape: DecimalShape
    for column_name, shape in batch_shapes.items():
        previous_shape: DecimalShape | None = decimal_shapes.get(column_name)
        if previous_shape is not None and previous_shape.widened(shape) != previous_shape:
            raise ExecutorInputError(
                f"Source loader column '{column_name}' needs {shape.widened(previous_shape)}, "
                f"wider than the {previous_shape} its earlier rows gave it; declare the "
                "column's type in the source's columns"
            )
        next_shapes.setdefault(column_name, shape)
    added_columns: tuple[ColumnInfo, ...] = tuple(
        ColumnInfo(
            name=column_name,
            type=(
                _column_sql_type(
                    adapter=adapter,
                    column_name=column_name,
                    declared_types=declared_types,
                    inferred_types=next_inferred_types,
                    decimal_shapes=next_shapes,
                )
                or "VARCHAR"
            ),
        )
        for column_name in added_column_names
    )
    return LoaderRowsSchema(
        column_names=tuple(names),
        inferred_types=next_inferred_types,
        added_columns=added_columns,
        decimal_shapes=next_shapes,
    )


def _validate_contract_row(*, row: dict[str, object], declared_names: tuple[str, ...]) -> None:
    declared_name_set: frozenset[str] = frozenset(declared_names)
    extra_names: tuple[str, ...] = tuple(name for name in row if name not in declared_name_set)
    if extra_names:
        raise ExecutorInputError(
            f"Source loader contract has extra columns: {', '.join(extra_names)}"
        )
    missing_names: tuple[str, ...] = tuple(name for name in declared_names if name not in row)
    if missing_names:
        raise ExecutorInputError(
            f"Source loader contract missing columns: {', '.join(missing_names)}"
        )


def build_rows_sql(
    *,
    adapter: BaseAdapter,
    rows: tuple[dict[str, object], ...],
    columns: tuple[SourceColumnEntry, ...],
    column_names: tuple[str, ...] | None = None,
    inferred_types: dict[str, LoaderLogicalType] | None = None,
    decimal_shapes: Mapping[str, DecimalShape] | None = None,
) -> str:
    """Build an adapter-rendered SELECT for framework-managed loader rows."""

    resolved_column_names: tuple[str, ...] = column_names or _resolve_column_names(
        rows=rows, columns=columns
    )
    if not resolved_column_names:
        raise ExecutorInputError("Source loader returned no rows and source declares no columns")
    declared_types: dict[str, str] = _declared_types(columns=columns)
    resolved_inferred_types: dict[str, LoaderLogicalType] = inferred_types or _infer_column_types(
        rows=rows, declared_types=declared_types
    )
    resolved_shapes: Mapping[str, DecimalShape] = (
        _decimal_shapes(rows=rows, declared_types=declared_types)
        if decimal_shapes is None
        else decimal_shapes
    )
    return adapter.render_loader_rows_select(
        rows=rows,
        column_names=resolved_column_names,
        column_sql_types=loader_column_sql_types(
            adapter=adapter,
            columns=columns,
            column_names=resolved_column_names,
            inferred_types=resolved_inferred_types,
            decimal_shapes=resolved_shapes,
        ),
        inferred_types=resolved_inferred_types,
    )


def loader_column_sql_types(
    *,
    adapter: BaseAdapter,
    columns: tuple[SourceColumnEntry, ...],
    column_names: tuple[str, ...],
    inferred_types: dict[str, LoaderLogicalType],
    decimal_shapes: Mapping[str, DecimalShape],
) -> dict[str, str]:
    """Return each loader column's declared or inferred warehouse type, when known."""

    declared_types: dict[str, str] = _declared_types(columns=columns)
    return {
        column_name: sql_type
        for column_name in column_names
        if (
            sql_type := _column_sql_type(
                adapter=adapter,
                column_name=column_name,
                declared_types=declared_types,
                inferred_types=inferred_types,
                decimal_shapes=decimal_shapes,
            )
        )
        is not None
    }


def _declared_types(*, columns: tuple[SourceColumnEntry, ...]) -> dict[str, str]:
    return {column.name: column.type for column in columns if column.type is not None}


def _resolve_column_names(
    *, rows: tuple[dict[str, object], ...], columns: tuple[SourceColumnEntry, ...]
) -> tuple[str, ...]:
    names: list[str] = [column.name for column in columns]
    row: dict[str, object]
    for row in rows:
        key: str
        for key in row:
            if key not in names:
                names.append(key)
    return tuple(names)


def _column_sql_type(
    *,
    adapter: BaseAdapter,
    column_name: str,
    declared_types: dict[str, str],
    inferred_types: dict[str, LoaderLogicalType],
    decimal_shapes: Mapping[str, DecimalShape],
) -> str | None:
    declared_type: str | None = declared_types.get(column_name)
    if declared_type is not None:
        return declared_type
    inferred_type: LoaderLogicalType | None = inferred_types.get(column_name)
    if inferred_type is None:
        return None
    shape: DecimalShape | None = decimal_shapes.get(column_name)
    if shape is None:
        return adapter.render_loader_logical_type(type_name=inferred_type)
    return adapter.render_loader_logical_type(
        type_name=inferred_type, precision=shape.precision, scale=shape.scale
    )


def _decimal_shapes(
    *, rows: tuple[dict[str, object], ...], declared_types: Mapping[str, str]
) -> dict[str, DecimalShape]:
    """The narrowest decimal type holding every Decimal of each undeclared column."""

    shapes: dict[str, DecimalShape] = {}
    row: dict[str, object]
    for row in rows:
        column_name: str
        value: object
        for column_name, value in row.items():
            if column_name in declared_types or not isinstance(value, Decimal):
                continue
            shape: DecimalShape = _decimal_shape(column_name=column_name, value=value)
            previous: DecimalShape | None = shapes.get(column_name)
            shapes[column_name] = shape if previous is None else previous.widened(shape)
    return shapes


def _decimal_shape(*, column_name: str, value: Decimal) -> DecimalShape:
    if not value.is_finite():
        raise ExecutorInputError(
            f"Source loader returned the Decimal {value} for column '{column_name}', which no "
            "DECIMAL column can hold"
        )
    digit_count: int = len(value.as_tuple().digits)
    exponent: int = int(value.as_tuple().exponent)
    shape: DecimalShape = DecimalShape(
        integer_digits=max(digit_count + exponent, 0), scale=max(-exponent, 0)
    )
    if shape.precision > _MAX_DECIMAL_PRECISION:
        raise ExecutorInputError(
            f"Source loader returned the Decimal {value} for column '{column_name}', which needs "
            f"more than {_MAX_DECIMAL_PRECISION} digits; declare the column's type in the "
            "source's columns"
        )
    return shape


def _infer_column_types(
    *, rows: tuple[dict[str, object], ...], declared_types: Mapping[str, str]
) -> dict[str, LoaderLogicalType]:
    inferred: dict[str, LoaderLogicalType] = {}
    row: dict[str, object]
    for row in rows:
        column_name: str
        value: object
        for column_name, value in row.items():
            if column_name in declared_types:
                continue
            logical_type: LoaderLogicalType | None = _infer_value_type(
                column_name=column_name, value=value
            )
            if logical_type is None:
                continue
            previous_type: LoaderLogicalType | None = inferred.get(column_name)
            if previous_type is not None and previous_type != logical_type:
                raise ExecutorInputError(
                    f"Source loader returned conflicting types for column '{column_name}'"
                )
            inferred[column_name] = logical_type
    return inferred


def _infer_value_type(*, column_name: str, value: object) -> LoaderLogicalType | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return LoaderLogicalType.BOOLEAN
    if isinstance(value, int):
        return LoaderLogicalType.INTEGER
    if isinstance(value, float):
        return LoaderLogicalType.FLOAT
    if isinstance(value, Decimal):
        return LoaderLogicalType.DECIMAL
    if isinstance(value, str):
        return LoaderLogicalType.STRING
    if isinstance(value, datetime):
        return LoaderLogicalType.TIMESTAMP
    if isinstance(value, date):
        return LoaderLogicalType.DATE
    if isinstance(value, dict | list):
        return LoaderLogicalType.JSON
    raise ExecutorInputError(
        f"Source loader returned a {type(value).__name__} value for column '{column_name}', "
        "which has no declared type; declare the column's type in the source's columns, or "
        "return a str, int, float, Decimal, bool, date, datetime, dict or list"
    )
