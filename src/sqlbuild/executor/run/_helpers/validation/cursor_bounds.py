"""Runtime cursor bound resolution and sentinel substitution."""

from __future__ import annotations

import time
from collections.abc import Callable
from datetime import UTC, datetime
from functools import partial
from typing import Any

from sqlbuild._native import (
    GRAIN_ORDER,
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    ZERO_DAY_CURSOR_DURATION,
    AlignedInterval,
    BackfillAction,
    CursorBounds,
    CursorGrain,
    CursorInputEvidence,
    CursorInputRelation,
    CursorType,
    DateValue,
    Duration,
    EmptyCursorInputDecision,
    IntegerValue,
    MaximumStartPolicyInputs,
    MicrobatchStrategy,
    ModelCursorSnapshot,
    ModelPlanEntry,
    StartCursorsConfig,
    TimestampValue,
    apply_maximum_start_policy,
    apply_typed_cursor_replay_policy,
    cursor_window_override_flags,
    decide_empty_cursor_inputs,
    empty_input_rebuild_refusal,
    replace_cursor_markers,
    resolve_cursor_intrinsics,
)
from sqlbuild._native import cursor_clamp as clamp
from sqlbuild._native import cursor_compare as compare
from sqlbuild._native import (
    cursor_exclusive_end_from_observed_max as exclusive_end_from_observed_max,
)
from sqlbuild._native import cursor_exclusive_to_inclusive as exclusive_to_inclusive
from sqlbuild._native import cursor_floor_to_grain as floor_to_grain
from sqlbuild._native import cursor_inclusive_to_exclusive as inclusive_to_exclusive
from sqlbuild._native import cursor_max_bound as max_bound
from sqlbuild._native import cursor_min_bound as min_bound
from sqlbuild._native import cursor_parse as parse
from sqlbuild._native import cursor_render as render
from sqlbuild._native import cursor_sentinel_from_token as sentinel_from_token
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from sqlbuild._native import resolve_future_cursor_safety as apply_future_cursor_safety
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.relations.main.cached_relation_exists import cached_relation_exists
from sqlbuild.cursor_algebra.types import BoundSentinel, CursorScalar
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run.exceptions import EmptyCursorInputsError
from sqlbuild.executor.run.models import RuntimeCursorInputRelation, RuntimeCursorSpec
from sqlbuild.executor.run.types import RuntimeCursorWatermarkMode, WatermarkResolver


def has_runtime_owned_cursor_watermarks(
    cursor_input_relations: tuple[CursorInputRelation | RuntimeCursorInputRelation, ...],
) -> bool:
    """Return whether any cursor input is produced by this invocation."""

    return any(relation.is_runtime_owned for relation in cursor_input_relations)


def has_authoritative_cursor_override(*, entry: ModelPlanEntry) -> bool:
    """Return whether complete operator bounds supersede runtime discovery."""

    return entry.start_cursor_override is not None and entry.end_cursor_override is not None


def build_runtime_cursor_spec(
    *, entry: ModelPlanEntry, read_destination_cursor: bool = True
) -> RuntimeCursorSpec:
    """Build runtime cursor policy from the corresponding immutable model plan entry."""

    if entry.cursor_column is None:
        raise ExecutorInputError("runtime-owned cursor resolution requires cursor_column")
    return RuntimeCursorSpec(
        cursor_column=entry.cursor_column,
        cursor_type=entry.cursor_type,
        cursor_grain=entry.cursor_grain,
        cursor_start=entry.cursor_start,
        cursor_end=entry.cursor_end,
        cursor_input_relations=tuple(
            RuntimeCursorInputRelation(
                relation=relation.relation,
                cursor_column=relation.cursor_column,
                input_name=relation.input_name,
                cursor_grain=relation.cursor_grain,
                is_model_backed=relation.is_model_backed,
                is_runtime_produced=relation.is_runtime_produced,
                terminal_cursor_start=relation.terminal_cursor_start,
                terminal_cursor_end=relation.terminal_cursor_end,
            )
            for relation in entry.cursor_input_relations
        ),
        cursor_watermark_mode=(
            entry.cursor_watermark_mode
            or (
                RuntimeCursorWatermarkMode.ALL
                if entry.microbatch_strategy == MicrobatchStrategy.WATERMARK
                else RuntimeCursorWatermarkMode.LEGACY
            )
        ),
        microbatch_strategy=entry.microbatch_strategy,
        incremental_strategy=entry.incremental_strategy,
        incremental_mode=entry.incremental_mode,
        start_cursor_override=entry.start_cursor_override,
        end_cursor_override=entry.end_cursor_override,
        lookback=entry.lookback,
        backfill_duration=(
            entry.backfill.duration if entry.backfill.action == BackfillAction.BOUNDED else None
        ),
        read_destination_cursor=read_destination_cursor,
        future_cursor_config=entry.future_cursor_config,
        start_cursor_config=entry.start_cursor_config,
        invocation_time=entry.invocation_time,
    )


def resolve_runtime_cursor_bounds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    target_relation: str,
    target_database: str | None,
    target_schema: str | None,
    target_name: str,
    spec: RuntimeCursorSpec,
    on_progress: Callable[[str], None] | None = None,
    watermark_resolver: WatermarkResolver | None = None,
) -> CursorBounds | None:
    """Resolve concrete runtime cursor bounds from target and upstream relations."""

    cursor_type: CursorType | None = spec.cursor_type
    bounded_override: CursorBounds | None = _resolve_typed_bounded_override(spec=spec)
    if bounded_override is not None:
        return bounded_override
    physical_inputs: tuple[tuple[tuple[str, str], RuntimeCursorInputRelation], ...] = tuple(
        sorted(
            {
                (cursor_input.relation, cursor_input.cursor_column): cursor_input
                for cursor_input in spec.cursor_input_relations
            }.items()
        )
    )
    if not physical_inputs:
        return None
    effective_grain: CursorGrain | None = resolve_effective_timestamp_grain(
        cursor_type=cursor_type,
        downstream_grain=spec.cursor_grain,
        cursor_input_relations=spec.cursor_input_relations,
        microbatch_strategy=spec.microbatch_strategy,
    )
    discovered_grain: CursorGrain | None = (
        effective_grain
        if spec.cursor_grain is not None
        or any(item.cursor_grain is not None for item in spec.cursor_input_relations)
        else None
    )

    target_max: CursorScalar | None = None
    if spec.read_destination_cursor:
        target_max = _query_target_max(
            adapter=adapter,
            connection=connection,
            target_relation=target_relation,
            target_database=target_database,
            target_schema=target_schema,
            target_name=target_name,
            cursor_column=spec.cursor_column,
            cursor_type=cursor_type,
        )
    target_eligible_max: CursorScalar | None = _query_eligible_target_max(
        adapter=adapter,
        connection=connection,
        target_relation=target_relation,
        cursor_column=spec.cursor_column,
        cursor_type=cursor_type,
        cursor_grain=effective_grain,
        target_max=target_max,
        spec=spec,
    )

    read_minimum: bool = target_max is None
    upstream_mins: list[CursorScalar] = []
    upstream_maxes: list[CursorScalar] = []
    upstream_availability_ends: list[CursorScalar] = []
    input_evidence: list[CursorInputEvidence] = []
    input_index: int
    physical_input: tuple[tuple[str, str], RuntimeCursorInputRelation]
    for input_index, physical_input in enumerate(physical_inputs, start=1):
        relation_column, cursor_input = physical_input
        terminal_start: CursorScalar | None = cursor_input.terminal_cursor_start
        terminal_end: CursorScalar | None = cursor_input.terminal_cursor_end
        input_grain: CursorGrain | None = cursor_input.cursor_grain or discovered_grain
        minimum: CursorScalar | None
        maximum: CursorScalar | None
        minimum, maximum = _query_watermark_values(
            adapter=adapter,
            connection=connection,
            relation=relation_column[0],
            cursor_column=relation_column[1],
            read_minimum=read_minimum,
            query_index=input_index,
            total=len(physical_inputs),
            on_progress=on_progress,
            watermark_resolver=watermark_resolver,
            cursor_type=cursor_type,
        )
        terminal_maximum: CursorScalar | None = _terminal_inclusive_maximum(
            value=terminal_end, cursor_type=cursor_type, cursor_grain=input_grain
        )
        if terminal_maximum is not None:
            maximum = terminal_maximum
        if read_minimum and minimum is None and terminal_start is not None:
            minimum = terminal_start
        if maximum is None or (read_minimum and minimum is None):
            continue
        if minimum is not None:
            upstream_mins.append(minimum)
        upstream_maxes.append(maximum)
        if (
            cursor_type == CursorType.TIMESTAMP
            and spec.microbatch_strategy == MicrobatchStrategy.WATERMARK
        ):
            availability_end: CursorScalar = (
                terminal_end
                if terminal_end is not None
                else _exclusive_end_from_observed(
                    value=maximum,
                    cursor_type=cursor_type,
                    cursor_grain=input_grain,
                )
            )
            upstream_availability_ends.append(availability_end)
        input_evidence.append(
            CursorInputEvidence(
                relation=relation_column[0],
                cursor_column=relation_column[1],
                minimum=minimum,
                maximum=maximum,
            )
        )
    _raise_for_empty_inputs(
        physical_inputs=physical_inputs,
        input_evidence=input_evidence,
        waits_for_every_input=_waits_for_every_input(spec=spec),
    )
    if not upstream_maxes:
        return None
    upstream_min: CursorScalar | None = (
        _minimum_bound(
            values=upstream_mins,
            cursor_type=cursor_type,
            effective_grain=effective_grain,
        )
        if upstream_mins
        else None
    )
    aggregate: Callable[..., CursorScalar] = (
        _maximum_bound
        if spec.cursor_watermark_mode == RuntimeCursorWatermarkMode.ANY
        else _minimum_bound
    )
    watermark_timestamp: bool = (
        cursor_type == CursorType.TIMESTAMP
        and spec.microbatch_strategy == MicrobatchStrategy.WATERMARK
    )
    upstream_max: CursorScalar = aggregate(
        values=(upstream_availability_ends if watermark_timestamp else upstream_maxes),
        cursor_type=cursor_type,
        effective_grain=(CursorGrain.SECOND if watermark_timestamp else effective_grain),
    )
    if cursor_type == CursorType.TIMESTAMP and effective_grain is not None:
        start_value: CursorScalar | None = target_max if target_max is not None else upstream_min
        if start_value is None:
            return None
        start: CursorScalar = floor_to_grain(value=start_value, grain=effective_grain)
        end: CursorScalar = (
            floor_to_grain(value=upstream_max, grain=effective_grain)
            if watermark_timestamp and discovered_grain is not None
            else upstream_max
            if watermark_timestamp
            else _exclusive_end_from_observed(
                value=upstream_max,
                cursor_type=cursor_type,
                cursor_grain=discovered_grain,
            )
        )
    else:
        start_optional: CursorScalar | None = target_max if target_max is not None else upstream_min
        if start_optional is None:
            return None
        start = start_optional
        end = _exclusive_end_from_observed(
            value=upstream_max,
            cursor_type=cursor_type,
            cursor_grain=discovered_grain,
        )
    if spec.end_cursor_override is not None:
        end = _apply_cursor_end_ceiling(
            current_end=end,
            end_cursor_override=spec.end_cursor_override,
            cursor_type=cursor_type,
            effective_grain=effective_grain,
        )
    if spec.start_cursor_override is not None:
        start = spec.start_cursor_override
    start = apply_typed_cursor_replay_policy(
        start=start,
        end=end,
        cursor_start=spec.cursor_start,
        cursor_type=cursor_type,
        lookback=spec.lookback,
        backfill_duration=spec.backfill_duration,
        has_start_override=spec.start_cursor_override is not None,
    )
    bounds: CursorBounds = apply_maximum_start_policy(
        bounds=CursorBounds(start=start, end=end),
        snapshot=ModelCursorSnapshot(
            target_max=target_max,
            upstream_mins=tuple(
                _normalize_temporal(value=value, cursor_type=cursor_type, grain=effective_grain)
                for value in upstream_mins
            ),
            upstream_maxes=tuple(
                _normalize_temporal(value=value, cursor_type=cursor_type, grain=effective_grain)
                for value in upstream_maxes
            ),
            physical_target_max=target_max,
            target_eligible_max=(
                _normalize_temporal(
                    value=target_eligible_max,
                    cursor_type=cursor_type,
                    grain=effective_grain,
                )
                if target_eligible_max is not None
                else None
            ),
            target_relation=target_relation,
            destination_cursor_column=spec.cursor_column,
        ),
        cursor_type=cursor_type,
        cursor_grain=effective_grain,
        cursor_start=spec.cursor_start,
        lookback=spec.lookback,
        backfill_duration=spec.backfill_duration,
        policy=MaximumStartPolicyInputs(
            config=spec.start_cursor_config,
            invocation_time=spec.invocation_time,
            incremental_strategy=spec.incremental_strategy,
            incremental_mode=spec.incremental_mode,
        ),
        has_start_override=spec.start_cursor_override is not None,
    )
    return _clamp_cursor_end(
        bounds=apply_future_cursor_safety(
            bounds=bounds,
            cursor_type=cursor_type,
            cursor_grain=effective_grain,
            config=spec.future_cursor_config,
            invocation_time=spec.invocation_time,
            has_complete_override=(
                spec.start_cursor_override is not None and spec.end_cursor_override is not None
            ),
            input_evidence=tuple(input_evidence),
        ),
        cursor_end=spec.cursor_end,
        cursor_type=cursor_type,
    )


def cursor_window_unavailable_error(*, entry: ModelPlanEntry) -> ExecutorInputError:
    """Report a missing cursor window in terms of the model and its cursor inputs."""

    input_names: str = ", ".join(
        _input_display_name(relation=relation) for relation in entry.cursor_input_relations
    )
    return ExecutorInputError(
        f"model '{entry.name}' cannot work out its cursor window: no cursor bounds were found "
        f"for its inputs ({input_names or 'none'})",
        help=(
            "check that the inputs exist and have their cursor columns, or set the window "
            f"explicitly with {cursor_window_override_flags(cursor_type=entry.cursor_type)}"
        ),
    )


def empty_input_rebuild_error(
    *, entry: ModelPlanEntry, input_names: tuple[str, ...]
) -> ExecutorInputError:
    """Refuse to replace an existing destination with the result of empty cursor inputs."""

    message, help_text = empty_input_rebuild_refusal(
        model_name=entry.name, input_names=input_names, cursor_type=entry.cursor_type
    )
    return ExecutorInputError(message, code="S302", help=help_text)


def _raise_for_empty_inputs(
    *,
    physical_inputs: tuple[tuple[tuple[str, str], RuntimeCursorInputRelation], ...],
    input_evidence: list[CursorInputEvidence],
    waits_for_every_input: bool,
) -> None:
    """Signal empty inputs with the same decision the planner applies to its own windows."""

    with_rows: set[tuple[str, str]] = {
        (evidence.relation, evidence.cursor_column) for evidence in input_evidence
    }
    decision: EmptyCursorInputDecision = decide_empty_cursor_inputs(
        empty_input_names=tuple(
            _input_display_name(relation=cursor_input)
            for relation_column, cursor_input in physical_inputs
            if relation_column not in with_rows
        ),
        waits_for_every_input=waits_for_every_input,
        inputs_with_rows=bool(with_rows),
        has_window_start=bool(with_rows),
    )
    if decision.input_names:
        raise EmptyCursorInputsError(
            input_names=decision.input_names,
            waiting_on_empty_inputs=decision.waiting_on_empty_inputs,
        )


def _waits_for_every_input(*, spec: RuntimeCursorSpec) -> bool:
    """Return whether every cursor input must have rows, matching planner-owned windows."""

    if spec.cursor_watermark_mode == RuntimeCursorWatermarkMode.ANY:
        return False
    return spec.microbatch_strategy != MicrobatchStrategy.ROLLING_WINDOW


def _input_display_name(*, relation: CursorInputRelation | RuntimeCursorInputRelation) -> str:
    """Label one cursor input by its project name, falling back to the physical relation."""

    relation_name: str = relation.input_name or relation.relation.rsplit(".", 1)[-1].strip('"`[]')
    return f"{relation_name}.{relation.cursor_column}"


def _clamp_cursor_end(
    *, bounds: CursorBounds, cursor_end: CursorScalar | None, cursor_type: CursorType | None
) -> CursorBounds:
    if cursor_end is None:
        return bounds
    effective_type: CursorType = cursor_type or CursorType.TIMESTAMP
    end_value: CursorScalar = cursor_end
    if isinstance(bounds.start, BoundSentinel) or isinstance(bounds.end, BoundSentinel):
        return bounds
    if compare(left=bounds.start, right=end_value) >= 0:
        return CursorBounds(start=end_value, end=end_value)
    if compare(left=bounds.end, right=end_value) > 0:
        if effective_type == CursorType.INTEGER:
            start_value: CursorScalar = bounds.start
            current_end_value: CursorScalar = bounds.end
            if (
                isinstance(start_value, IntegerValue)
                and isinstance(current_end_value, IntegerValue)
                and isinstance(end_value, IntegerValue)
            ):
                original: AlignedInterval = AlignedInterval(
                    start=start_value, end=current_end_value, grain=None
                )
                ceiling: AlignedInterval = AlignedInterval(
                    start=start_value, end=end_value, grain=None
                )
                clamped: AlignedInterval | None = clamp(interval=original, bounds=ceiling)
                if clamped is not None:
                    return CursorBounds(start=bounds.start, end=end_value)
        return CursorBounds(start=bounds.start, end=end_value)
    return bounds


def _normalize_temporal(
    *, value: CursorScalar, cursor_type: CursorType | None, grain: CursorGrain | None
) -> CursorScalar:
    if cursor_type == CursorType.TIMESTAMP and grain is not None:
        return floor_to_grain(value=value, grain=grain)
    return value


def _query_watermark_raw(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relation: str,
    cursor_column: str,
    read_minimum: bool,
    query_index: int,
    total: int,
    on_progress: Callable[[str], None] | None,
    watermark_resolver: WatermarkResolver | None,
) -> tuple[object | None, object | None]:
    """Resolve one physical watermark through the optional run-scoped cache."""

    query: Callable[[], tuple[object | None, object | None]] = partial(
        _execute_watermark_query,
        adapter=adapter,
        connection=connection,
        relation=relation,
        cursor_column=cursor_column,
        read_minimum=read_minimum,
        query_index=query_index,
        total=total,
        on_progress=on_progress,
    )
    if watermark_resolver is None:
        return query()
    return watermark_resolver.resolve(
        relation=relation,
        cursor_column=cursor_column,
        read_minimum=read_minimum,
        query=query,
    )


def _query_watermark_values(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relation: str,
    cursor_column: str,
    cursor_type: CursorType | None,
    read_minimum: bool,
    query_index: int,
    total: int,
    on_progress: Callable[[str], None] | None,
    watermark_resolver: WatermarkResolver | None,
) -> tuple[CursorScalar | None, CursorScalar | None]:
    minimum_raw, maximum_raw = _query_watermark_raw(
        adapter=adapter,
        connection=connection,
        relation=relation,
        cursor_column=adapter.render_cursor_operand(
            expression=cursor_column, cursor_type=cursor_type
        ),
        read_minimum=read_minimum,
        query_index=query_index,
        total=total,
        on_progress=on_progress,
        watermark_resolver=watermark_resolver,
    )
    effective_type: CursorType = cursor_type or CursorType.TIMESTAMP
    return (
        parse(raw=minimum_raw, cursor_type=effective_type) if minimum_raw is not None else None,
        parse(raw=maximum_raw, cursor_type=effective_type) if maximum_raw is not None else None,
    )


def _execute_watermark_query(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relation: str,
    cursor_column: str,
    read_minimum: bool,
    query_index: int,
    total: int,
    on_progress: Callable[[str], None] | None,
) -> tuple[object | None, object | None]:
    """Execute one optimizer-friendly physical watermark statement."""

    bounds: str = "min,max" if read_minimum else "max"
    identity: str = f"({query_index}/{total}): {relation}.{cursor_column} [{bounds}]"
    select_sql: str = (
        f"MIN({cursor_column}), MAX({cursor_column})" if read_minimum else f"MAX({cursor_column})"
    )
    sql: str = f"SELECT {select_sql} FROM {relation}"
    if on_progress is not None:
        on_progress(f"Inspecting runtime cursor watermark {identity}...")
    query_start: float = time.monotonic()
    try:
        cursor: Any = adapter.execute(connection=connection, sql=sql)
        row: Any = cursor.fetchone()
    except Exception as error:
        elapsed: float = time.monotonic() - query_start
        if on_progress is not None:
            on_progress(f"Failed runtime cursor watermark {identity} ({elapsed:.2f}s): {error}")
        raise
    elapsed = time.monotonic() - query_start
    if on_progress is not None:
        on_progress(f"Inspected runtime cursor watermark {identity} ({elapsed:.2f}s)")
    if row is None:
        return None, None
    if read_minimum:
        return row[0], row[1]
    return None, row[0]


def _minimum_bound(
    *,
    values: list[CursorScalar],
    cursor_type: CursorType | None,
    effective_grain: CursorGrain | None,
) -> CursorScalar:
    """Choose a conservative minimum across typed watermark values."""

    normalized: list[CursorScalar] = [
        _normalize_temporal(value=value, cursor_type=cursor_type, grain=effective_grain)
        for value in values
    ]
    selected: CursorScalar = min_bound(
        values=normalized, cursor_type=cursor_type or CursorType.TIMESTAMP
    )
    return values[normalized.index(selected)]


def _maximum_bound(
    *,
    values: list[CursorScalar],
    cursor_type: CursorType | None,
    effective_grain: CursorGrain | None,
) -> CursorScalar:
    """Choose the furthest usable alternative typed watermark value."""

    normalized: list[CursorScalar] = [
        _normalize_temporal(value=value, cursor_type=cursor_type, grain=effective_grain)
        for value in values
    ]
    selected: CursorScalar = max_bound(
        values=normalized, cursor_type=cursor_type or CursorType.TIMESTAMP
    )
    return values[normalized.index(selected)]


def _terminal_inclusive_maximum(
    *,
    value: CursorScalar | None,
    cursor_type: CursorType | None,
    cursor_grain: CursorGrain | None,
) -> CursorScalar | None:
    if value is None:
        return None
    if cursor_type == CursorType.INTEGER:
        return exclusive_to_inclusive(value=value, grain=None)
    parsed: CursorScalar = value
    if isinstance(parsed, DateValue):
        parsed = TimestampValue(value=datetime.combine(parsed.value, datetime.min.time()))
    inclusive: CursorScalar = exclusive_to_inclusive(
        value=parsed, grain=CursorGrain(cursor_grain or CursorGrain.SECOND)
    )
    return inclusive


def resolve_effective_timestamp_grain(
    *,
    cursor_type: CursorType | str | None,
    downstream_grain: CursorGrain | str | None,
    cursor_input_relations: tuple[CursorInputRelation | RuntimeCursorInputRelation, ...],
    microbatch_strategy: str | None = None,
) -> CursorGrain | None:
    """Return the coarsest timestamp grain participating in runtime-owned replay."""

    if cursor_type != CursorType.TIMESTAMP:
        return None
    if microbatch_strategy == MicrobatchStrategy.WATERMARK:
        return CursorGrain(downstream_grain or CursorGrain.SECOND)
    effective: CursorGrain = CursorGrain(downstream_grain or CursorGrain.SECOND)
    cursor_input: CursorInputRelation | RuntimeCursorInputRelation
    for cursor_input in cursor_input_relations:
        input_grain: CursorGrain = CursorGrain(cursor_input.cursor_grain or CursorGrain.SECOND)
        if GRAIN_ORDER[input_grain] > GRAIN_ORDER[effective]:
            effective = input_grain
    return effective


def substitute_cursor_sentinels(*, sql: str, bounds: CursorBounds) -> str:
    """Substitute runtime cursor sentinels with concrete bounds."""

    if sentinel_from_token(token=MICROBATCH_START_SENTINEL) is None:
        raise ExecutorInputError("invalid start cursor marker")
    result: str = replace_cursor_markers(
        sql=sql,
        start_sql=sentinel_to_token(sentinel=bounds.start),
        end_sql=sentinel_to_token(sentinel=bounds.end),
    )
    _, has_intrinsics = resolve_cursor_intrinsics(sql=result)
    if has_intrinsics or MICROBATCH_START_SENTINEL in result or MICROBATCH_END_SENTINEL in result:
        raise ExecutorInputError("executable model SQL contains unresolved cursor markers")
    return result


def _query_target_max(
    *,
    adapter: BaseAdapter,
    connection: Any,
    target_relation: str,
    target_database: str | None,
    target_schema: str | None,
    target_name: str,
    cursor_column: str,
    cursor_type: CursorType | None,
) -> CursorScalar | None:
    """Read the target cursor high-water mark or None when the target does not exist."""

    if not cached_relation_exists(
        adapter=adapter,
        connection=connection,
        database=target_database,
        schema=target_schema,
        name=target_name,
    ):
        return None
    operand: str = adapter.render_cursor_operand(expression=cursor_column, cursor_type=cursor_type)
    sql: str = f"SELECT MAX({operand}) FROM {target_relation}"
    cursor: Any = adapter.execute(connection=connection, sql=sql)
    row: Any = cursor.fetchone()
    if row is None or row[0] is None:
        return None
    return parse(raw=row[0], cursor_type=cursor_type or CursorType.TIMESTAMP)


def _query_eligible_target_max(
    *,
    adapter: BaseAdapter,
    connection: Any,
    target_relation: str,
    cursor_column: str,
    cursor_type: CursorType | None,
    cursor_grain: CursorGrain | None,
    target_max: CursorScalar | None,
    spec: RuntimeCursorSpec,
) -> CursorScalar | None:
    """Read the target MAX at or below the automatic-start horizon when recovery is needed."""

    config: StartCursorsConfig | None = spec.start_cursor_config
    if (
        config is None
        or config.max_ahead is None
        or cursor_type != CursorType.TIMESTAMP
        or spec.invocation_time is None
        or target_max is None
        or spec.start_cursor_override is not None
    ):
        return None
    horizon: CursorScalar = _maximum_allowed_start(
        discovered_value=target_max,
        cursor_grain=cursor_grain,
        invocation_time=spec.invocation_time,
        max_ahead=config.max_ahead,
    )
    if compare(left=target_max, right=horizon) <= 0:
        return None
    eligible_sql: str = adapter.render_max_cursor_at_or_before(
        relation=target_relation,
        cursor_column=cursor_column,
        maximum_allowed=render(value=horizon),
        cursor_type=cursor_type,
        is_date=isinstance(target_max, DateValue),
    )
    cursor: Any = adapter.execute(
        connection=connection,
        sql=eligible_sql,
    )
    row: Any = cursor.fetchone()
    return (
        None
        if row is None or row[0] is None
        else parse(raw=row[0], cursor_type=CursorType.TIMESTAMP)
    )


def _maximum_allowed_start(
    *,
    discovered_value: CursorScalar,
    cursor_grain: CursorGrain | None,
    invocation_time: datetime,
    max_ahead: str,
) -> CursorScalar:
    duration: Duration | None = Duration.parse(max_ahead)
    if duration is None and max_ahead == ZERO_DAY_CURSOR_DURATION:
        duration = Duration()
    if duration is None:
        raise ExecutorInputError(f"invalid cursors.start.max_ahead '{max_ahead}'")
    maximum: datetime = duration.add_to(invocation_time.astimezone(UTC).replace(tzinfo=None))
    normalized: TimestampValue = TimestampValue(value=maximum)
    if isinstance(discovered_value, TimestampValue) and discovered_value.value.tzinfo is not None:
        normalized = TimestampValue(value=maximum.replace(tzinfo=UTC))
    floored: CursorScalar = floor_to_grain(
        value=normalized, grain=cursor_grain or CursorGrain.SECOND
    )
    if isinstance(discovered_value, DateValue) and isinstance(floored, TimestampValue):
        return DateValue(value=floored.value.date())
    return floored


def _apply_cursor_end_ceiling(
    *,
    current_end: CursorScalar,
    end_cursor_override: CursorScalar,
    cursor_type: CursorType | None,
    effective_grain: CursorGrain | None,
) -> CursorScalar:
    """Clamp the discovered exclusive end down to the inclusive `--end-cursor-ts` override."""

    if cursor_type == CursorType.TIMESTAMP:
        grain: CursorGrain = effective_grain or CursorGrain.SECOND
        exclusive_override: CursorScalar = inclusive_to_exclusive(
            value=floor_to_grain(value=end_cursor_override, grain=grain), grain=grain
        )
        return min_bound(values=(current_end, exclusive_override), cursor_type=CursorType.TIMESTAMP)
    if cursor_type == CursorType.INTEGER:
        if not isinstance(current_end, IntegerValue) or not isinstance(
            end_cursor_override, IntegerValue
        ):
            return current_end
        return min_bound(
            values=(
                current_end,
                inclusive_to_exclusive(value=end_cursor_override, grain=None),
            ),
            cursor_type=CursorType.INTEGER,
        )
    return current_end


def _exclusive_end_from_observed(
    *, value: CursorScalar, cursor_type: CursorType | None, cursor_grain: CursorGrain | None
) -> CursorScalar:
    grain: CursorGrain | None = (
        None
        if cursor_type == CursorType.INTEGER
        else cursor_grain
        or (CursorGrain.DAY if isinstance(value, DateValue) else CursorGrain.SECOND)
    )
    return exclusive_end_from_observed_max(value=value, grain=grain)


def _resolve_typed_bounded_override(*, spec: RuntimeCursorSpec) -> CursorBounds | None:
    if spec.start_cursor_override is None or spec.end_cursor_override is None:
        return None
    grain: CursorGrain | None = (
        None if spec.cursor_type == CursorType.INTEGER else spec.cursor_grain or CursorGrain.SECOND
    )
    return CursorBounds(
        start=spec.start_cursor_override,
        end=inclusive_to_exclusive(value=spec.end_cursor_override, grain=grain),
    )
