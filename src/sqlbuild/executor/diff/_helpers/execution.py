"""Per-model diff execution."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

from sqlbuild._native import (
    merge_row_diff_tolerances,
    parse_row_diff_tolerances,
    resolve_row_diff_sampling,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import DIFF_LEFT_SIDE, DIFF_RIGHT_SIDE
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    RowDiffCoverage,
    RowDiffResult,
    RowDiffSampling,
    RowDiffTolerances,
    SchemaDiffResult,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.diff._helpers.bounds import resolve_bounded_cursors
from sqlbuild.executor.diff._helpers.selection import (
    get_row_diff_exclude_columns,
    get_unique_key,
    qualified_name,
)
from sqlbuild.executor.diff.constants import DIFF_INPUT_KIND_QUERY
from sqlbuild.executor.diff.models import DiffExecutionOptions, ModelDiffResult


def execute_model_diff(
    *,
    adapter: BaseAdapter,
    connection: Any,
    name: str,
    left_model: Any,
    right_model: Any,
    options: DiffExecutionOptions,
) -> ModelDiffResult:
    """Execute schema and optional row diff for one model pair."""

    left_relation: str = qualified_name(adapter=adapter, model=left_model)
    right_relation: str = qualified_name(adapter=adapter, model=right_model)
    unique_key: tuple[str, ...] = ()
    if options.unkeyed:
        unique_key = ()
    elif options.unique_key_override:
        unique_key = options.unique_key_override
    elif _model_has_unique_key(right_model):
        unique_key = get_unique_key(right_model)
    try:
        schema_result: SchemaDiffResult = adapter.diff_schema(
            connection=connection,
            left=left_relation,
            right=right_relation,
        )
    except Exception as error:
        _raise_missing_relation(
            adapter=adapter,
            connection=connection,
            name=name,
            sides=(("FROM", left_model, left_relation), ("TO", right_model, right_relation)),
            error=error,
        )
        raise
    if options.max_columns is not None:
        observed_columns: int = max(
            schema_result.left_column_count,
            schema_result.right_column_count,
        )
        if observed_columns > options.max_columns:
            raise ExecutorInputError(
                f"model '{name}' has {observed_columns} columns, exceeding --max-columns "
                f"{options.max_columns}",
                code="X304",
            )
    row_result: RowDiffResult | None = None
    unequal_row_samples: tuple[Any, ...] = ()
    left_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    right_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    bounded_fallback: bool = False
    excluded_columns: tuple[str, ...] = ()
    if not options.schema_only:
        (
            row_result,
            unequal_row_samples,
            left_only_key_samples,
            right_only_key_samples,
            bounded_fallback,
            excluded_columns,
        ) = _execute_model_row_diff(
            adapter=adapter,
            connection=connection,
            name=name,
            left_relation=left_relation,
            right_relation=right_relation,
            right_model=right_model,
            unique_key=unique_key,
            options=options,
        )
        if not unique_key and not options.unkeyed:
            unique_key = get_unique_key(right_model)
    return ModelDiffResult(
        name=name,
        left_relation=left_relation,
        right_relation=right_relation,
        unique_key=unique_key,
        schema_result=schema_result,
        row_result=row_result,
        unequal_row_samples=tuple(unequal_row_samples),
        left_only_key_samples=left_only_key_samples,
        right_only_key_samples=right_only_key_samples,
        bounded_fallback=bounded_fallback,
        excluded_columns=excluded_columns,
        unkeyed=options.unkeyed,
    )


def execute_query_diff(
    *,
    adapter: BaseAdapter,
    connection: Any,
    left_relation: str,
    right_relation: str,
    options: DiffExecutionOptions,
) -> ModelDiffResult:
    """Execute one prepared raw-query relation comparison."""

    unique_key: tuple[str, ...] = options.unique_key_override
    schema_result: SchemaDiffResult = adapter.diff_schema(
        connection=connection,
        left=left_relation,
        right=right_relation,
    )
    observed_columns: int = max(
        schema_result.left_column_count,
        schema_result.right_column_count,
    )
    if options.max_columns is not None and observed_columns > options.max_columns:
        raise ExecutorInputError(
            f"query results have {observed_columns} columns, exceeding --max-columns "
            f"{options.max_columns}",
            code="X305",
        )
    result: ModelDiffResult = ModelDiffResult(
        name=options.comparison_name or "query comparison",
        left_relation=left_relation,
        right_relation=right_relation,
        unique_key=unique_key,
        schema_result=schema_result,
        excluded_columns=options.excluded_columns_override,
        input_kind=DIFF_INPUT_KIND_QUERY,
        unkeyed=options.unkeyed,
    )
    if options.schema_only or _schema_differs(schema_result=schema_result):
        return result
    left_columns: tuple[ColumnInfo, ...] = adapter.describe_relation(
        connection=connection,
        relation=left_relation,
    )
    right_columns: tuple[ColumnInfo, ...] = adapter.describe_relation(
        connection=connection,
        relation=right_relation,
    )
    _validate_query_diff_columns(
        left_columns=left_columns,
        right_columns=right_columns,
        unique_key=unique_key,
        excluded_columns=options.excluded_columns_override,
        tolerance_columns=tuple(
            options.tolerance_overrides.by_column if options.tolerance_overrides is not None else ()
        ),
        unkeyed=options.unkeyed,
    )
    canonical_names: dict[str, str] = {
        column_name_key(column.name): column.name for column in left_columns
    }
    unique_key = tuple(canonical_names[column_name_key(key)] for key in unique_key)
    excluded_columns: tuple[str, ...] = tuple(
        canonical_names[column_name_key(column)] for column in options.excluded_columns_override
    )
    tolerance_overrides: RowDiffTolerances = options.tolerance_overrides or RowDiffTolerances()
    tolerances: RowDiffTolerances = replace(
        tolerance_overrides,
        by_column={
            canonical_names[column_name_key(column)]: tolerance
            for column, tolerance in tolerance_overrides.by_column.items()
        },
    )
    result = replace(
        result,
        unique_key=unique_key,
        excluded_columns=excluded_columns,
    )
    left_coverage: RowDiffCoverage = adapter.inspect_row_diff_coverage(
        connection=connection,
        relation=left_relation,
    )
    right_coverage: RowDiffCoverage = adapter.inspect_row_diff_coverage(
        connection=connection,
        relation=right_relation,
    )
    if options.unkeyed:
        row_result: RowDiffResult = adapter.diff_unkeyed_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            excluded_columns=excluded_columns,
        )
        left_only_rows, right_only_rows = _unkeyed_side_only_samples(
            adapter=adapter,
            connection=connection,
            relations=(left_relation, right_relation),
            row_result=row_result,
            options=options,
            excluded_columns=excluded_columns,
        )
        return replace(
            result,
            row_result=replace(
                row_result,
                left_coverage=left_coverage,
                right_coverage=right_coverage,
            ),
            left_only_key_samples=left_only_rows,
            right_only_key_samples=right_only_rows,
        )
    sampling: RowDiffSampling | None = resolve_row_diff_sampling(
        raw_row_limit=None,
        raw_seed=None,
        override=options.sampling_override,
        label="raw-query diff",
    )
    tolerances = replace(tolerances, sampling=sampling)
    row_result = adapter.diff_rows(
        connection=connection,
        left=left_relation,
        right=right_relation,
        unique_key=unique_key,
        excluded_columns=excluded_columns,
        tolerances=tolerances,
    )
    row_result = replace(
        row_result,
        left_coverage=left_coverage,
        right_coverage=right_coverage,
    )
    unequal_row_samples: tuple[Any, ...] = ()
    left_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    right_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    if options.collect_samples and row_result.unequal_count > 0:
        unequal_row_samples = adapter.sample_unequal_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=unique_key,
            excluded_columns=excluded_columns,
            tolerances=tolerances,
            limit=options.max_column_examples * 5,
        )
    if options.collect_samples and row_result.left_only_count > 0:
        left_only_key_samples = adapter.sample_side_only_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=unique_key,
            side="left",
            limit=options.max_row_only_examples,
            sampling=sampling,
        )
    if options.collect_samples and row_result.right_only_count > 0:
        right_only_key_samples = adapter.sample_side_only_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=unique_key,
            side="right",
            limit=options.max_row_only_examples,
            sampling=sampling,
        )
    return replace(
        result,
        row_result=row_result,
        unequal_row_samples=tuple(unequal_row_samples),
        left_only_key_samples=left_only_key_samples,
        right_only_key_samples=right_only_key_samples,
    )


def _schema_differs(*, schema_result: SchemaDiffResult) -> bool:
    return bool(
        schema_result.added_columns
        or schema_result.removed_columns
        or schema_result.type_changed_columns
    )


def _validate_query_diff_columns(
    *,
    left_columns: tuple[ColumnInfo, ...],
    right_columns: tuple[ColumnInfo, ...],
    unique_key: tuple[str, ...],
    excluded_columns: tuple[str, ...],
    tolerance_columns: tuple[str, ...],
    unkeyed: bool,
) -> None:
    left_names: dict[str, str] = {
        column_name_key(column.name): column.name for column in left_columns
    }
    right_names: dict[str, str] = {
        column_name_key(column.name): column.name for column in right_columns
    }
    for key in unique_key:
        if column_name_key(key) not in left_names or column_name_key(key) not in right_names:
            raise ExecutorInputError(
                f"raw-query diff key column '{key}' is missing from one or both results",
                code="X307",
            )
    missing_exclusions: tuple[str, ...] = tuple(
        column
        for column in excluded_columns
        if column_name_key(column) not in left_names or column_name_key(column) not in right_names
    )
    if missing_exclusions:
        raise ExecutorInputError(
            "raw-query diff excluded columns are missing from one or both results: "
            + ", ".join(missing_exclusions),
            code="X308",
        )
    missing_tolerances: tuple[str, ...] = tuple(
        column
        for column in tolerance_columns
        if column_name_key(column) not in left_names or column_name_key(column) not in right_names
    )
    if missing_tolerances:
        raise ExecutorInputError(
            "raw-query diff tolerance columns are missing from one or both results: "
            + ", ".join(missing_tolerances),
            code="X311",
        )
    excluded_column_names: frozenset[str] = frozenset(
        column_name_key(item) for item in excluded_columns
    )
    intersection: tuple[str, ...] = tuple(
        key for key in unique_key if column_name_key(key) in excluded_column_names
    )
    if intersection:
        raise ExecutorInputError(
            "raw-query diff excluded columns intersect --key: " + ", ".join(intersection),
            code="X309",
        )
    tolerance_column_names: frozenset[str] = frozenset(
        column_name_key(item) for item in tolerance_columns
    )
    tolerance_key_intersection: tuple[str, ...] = tuple(
        key for key in unique_key if column_name_key(key) in tolerance_column_names
    )
    if tolerance_key_intersection:
        raise ExecutorInputError(
            "raw-query diff tolerance columns intersect --key: "
            + ", ".join(tolerance_key_intersection),
            code="X312",
        )
    excluded_names: frozenset[str] = frozenset(
        column_name_key(column) for column in excluded_columns
    )
    if unkeyed and all(column_name_key(column.name) in excluded_names for column in left_columns):
        raise ExecutorInputError(
            "unkeyed raw-query diff requires at least one compared column",
            code="X310",
        )


def _execute_model_row_diff(
    *,
    adapter: BaseAdapter,
    connection: Any,
    name: str,
    left_relation: str,
    right_relation: str,
    right_model: Any,
    unique_key: tuple[str, ...],
    options: DiffExecutionOptions,
) -> tuple[
    RowDiffResult,
    tuple[Any, ...],
    tuple[tuple[tuple[str, object], ...], ...],
    tuple[tuple[tuple[str, object], ...], ...],
    bool,
    tuple[str, ...],
]:
    resolved_unique_key: tuple[str, ...] = unique_key
    if not resolved_unique_key and not options.unkeyed:
        resolved_unique_key = get_unique_key(right_model)
    excluded_columns: tuple[str, ...] = tuple(
        dict.fromkeys(
            (*get_row_diff_exclude_columns(right_model), *options.excluded_columns_override)
        )
    )
    _require_key_columns(
        adapter=adapter,
        connection=connection,
        relations=(left_relation, right_relation),
        keys=resolved_unique_key,
    )
    intersection: tuple[str, ...] = tuple(
        key for key in resolved_unique_key if key in excluded_columns
    )
    if intersection:
        columns: str = ", ".join(intersection)
        raise ExecutorInputError(
            f"model '{name}' row_diff_exclude_columns intersects unique_key: {columns}",
            code="X302",
        )
    cursor_column: str | None
    start_cursor: Any | None
    end_cursor: Any | None
    bounded_fallback: bool
    cursor_column, start_cursor, end_cursor, bounded_fallback = resolve_bounded_cursors(
        model=right_model,
        bounded=options.bounded,
    )
    left_coverage: RowDiffCoverage = adapter.inspect_row_diff_coverage(
        connection=connection,
        relation=left_relation,
        cursor_column=cursor_column,
        start_cursor=start_cursor,
        end_cursor=end_cursor,
    )
    right_coverage: RowDiffCoverage = adapter.inspect_row_diff_coverage(
        connection=connection,
        relation=right_relation,
        cursor_column=cursor_column,
        start_cursor=start_cursor,
        end_cursor=end_cursor,
    )
    if options.unkeyed:
        row_result: RowDiffResult = adapter.diff_unkeyed_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            excluded_columns=excluded_columns,
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        unkeyed_left_rows, unkeyed_right_rows = _unkeyed_side_only_samples(
            adapter=adapter,
            connection=connection,
            relations=(left_relation, right_relation),
            row_result=row_result,
            options=options,
            excluded_columns=excluded_columns,
            cursor=(cursor_column, start_cursor, end_cursor),
        )
        return (
            replace(
                row_result,
                left_coverage=left_coverage,
                right_coverage=right_coverage,
                cursor_column=cursor_column,
                cursor_start=start_cursor.value if start_cursor is not None else None,
                cursor_end=end_cursor.value if end_cursor is not None else None,
            ),
            (),
            unkeyed_left_rows,
            unkeyed_right_rows,
            bounded_fallback,
            excluded_columns,
        )
    tolerances: RowDiffTolerances = parse_row_diff_tolerances(
        raw=right_model.config.values.get("row_diff_tolerances"),
        label=f"model '{name}' row_diff_tolerances",
    )
    tolerances = merge_row_diff_tolerances(
        base=tolerances,
        override=options.tolerance_overrides,
    )
    sampling: RowDiffSampling | None = resolve_row_diff_sampling(
        raw_row_limit=right_model.config.values.get("row_diff_sample_rows"),
        raw_seed=right_model.config.values.get("row_diff_sample_seed"),
        override=options.sampling_override,
        label=f"model '{name}'",
    )
    tolerances = replace(tolerances, sampling=sampling)
    row_result: RowDiffResult = adapter.diff_rows(
        connection=connection,
        left=left_relation,
        right=right_relation,
        unique_key=resolved_unique_key,
        excluded_columns=excluded_columns,
        tolerances=tolerances,
        cursor_column=cursor_column,
        start_cursor=start_cursor,
        end_cursor=end_cursor,
    )
    row_result = replace(
        row_result,
        left_coverage=left_coverage,
        right_coverage=right_coverage,
        cursor_column=cursor_column,
        cursor_start=start_cursor.value if start_cursor is not None else None,
        cursor_end=end_cursor.value if end_cursor is not None else None,
    )
    unequal_row_samples: tuple[Any, ...] = ()
    left_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    right_only_key_samples: tuple[tuple[tuple[str, object], ...], ...] = ()
    if options.collect_samples and row_result.unequal_count > 0:
        unequal_row_samples = adapter.sample_unequal_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=resolved_unique_key,
            excluded_columns=excluded_columns,
            tolerances=tolerances,
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
            limit=options.max_column_examples * 5,
        )
    if options.collect_samples and row_result.left_only_count > 0:
        left_only_key_samples = adapter.sample_side_only_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=resolved_unique_key,
            side="left",
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
            limit=options.max_row_only_examples,
            sampling=sampling,
        )
    if options.collect_samples and row_result.right_only_count > 0:
        right_only_key_samples = adapter.sample_side_only_rows(
            connection=connection,
            left=left_relation,
            right=right_relation,
            unique_key=resolved_unique_key,
            side="right",
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
            limit=options.max_row_only_examples,
            sampling=sampling,
        )
    return (
        row_result,
        unequal_row_samples,
        left_only_key_samples,
        right_only_key_samples,
        bounded_fallback,
        excluded_columns,
    )


def _unkeyed_side_only_samples(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relations: tuple[str, str],
    row_result: RowDiffResult,
    options: DiffExecutionOptions,
    excluded_columns: tuple[str, ...],
    cursor: tuple[str | None, Any | None, Any | None] = (None, None, None),
) -> tuple[tuple[tuple[tuple[str, object], ...], ...], tuple[tuple[tuple[str, object], ...], ...]]:
    """Return bounded samples of the complete rows each side holds more often than the other."""

    samples: list[tuple[tuple[tuple[str, object], ...], ...]] = []
    side: str
    count: int
    for side, count in (
        (DIFF_LEFT_SIDE, row_result.left_only_count),
        (DIFF_RIGHT_SIDE, row_result.right_only_count),
    ):
        if not options.collect_samples or count <= 0 or not isinstance(adapter, UnkeyedDiffMixin):
            samples.append(())
            continue
        samples.append(
            adapter.sample_unkeyed_side_only_rows(
                connection=connection,
                left=relations[0],
                right=relations[1],
                side=side,
                excluded_columns=excluded_columns,
                cursor_column=cursor[0],
                start_cursor=cursor[1],
                end_cursor=cursor[2],
                limit=options.max_row_only_examples,
            )
        )
    return samples[0], samples[1]


def _model_has_unique_key(model: Any) -> bool:
    raw: object | None = model.config.values.get("unique_key")
    if isinstance(raw, str):
        return bool(raw)
    if isinstance(raw, list | tuple):
        return any(isinstance(item, str) and item for item in raw)
    return False


def _raise_missing_relation(
    *,
    adapter: BaseAdapter,
    connection: Any,
    name: str,
    sides: tuple[tuple[str, Any, str], tuple[str, Any, str]],
    error: Exception,
) -> None:
    """Raise a coded error when a schema diff failed because one side's relation is missing."""

    left, right = sides
    left_missing: bool = _relation_missing(
        adapter=adapter, connection=connection, model=left[1], error=error
    )
    right_missing: bool = not left_missing and _relation_missing(
        adapter=adapter, connection=connection, model=right[1], error=error
    )
    missing: tuple[tuple[str, Any, str], ...] = tuple(
        side for side, absent in ((left, left_missing), (right, right_missing)) if absent
    )
    if not missing:
        return
    label, _, relation = missing[0]
    raise ExecutorInputError(
        f"model '{name}' has no relation {relation} in the {label} target",
        code="X310",
        help="Build the model in that target first, or leave it out of --select.",
    ) from error


def _relation_missing(
    *, adapter: BaseAdapter, connection: Any, model: Any, error: Exception
) -> bool:
    """Whether the model's relation is absent; a failed check re-raises the diff's own error."""

    try:
        exists: bool = adapter.relation_exists(
            connection=connection,
            database=model.destination.database,
            schema=model.destination.schema,
            name=model.destination.name,
        )
    except Exception:
        raise error  # noqa: B904
    return not exists


def _require_key_columns(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relations: tuple[str, str],
    keys: tuple[str, ...],
) -> None:
    """Reject row diff key columns that one of the relations does not have."""

    if not keys:
        return
    left, right = relations
    _require_relation_keys(adapter=adapter, connection=connection, relation=left, keys=keys)
    _require_relation_keys(adapter=adapter, connection=connection, relation=right, keys=keys)


def _require_relation_keys(
    *, adapter: BaseAdapter, connection: Any, relation: str, keys: tuple[str, ...]
) -> None:
    columns: set[str] = {
        column_name_key(column.name)
        for column in adapter.describe_relation(connection=connection, relation=relation)
    }
    missing: tuple[str, ...] = tuple(key for key in keys if column_name_key(key) not in columns)
    if not missing:
        return
    noun: str = "column" if len(missing) == 1 else "columns"
    verb: str = "does" if len(missing) == 1 else "do"
    raise ExecutorInputError(
        f"row diff key {noun} {', '.join(repr(key) for key in missing)} "
        f"{verb} not exist in {relation}",
        code="X311",
        help="Pass --key with columns both relations have, or fix the model's unique_key.",
    )
