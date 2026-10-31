"""Bounded expected-output difference sampling."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping
from dataclasses import replace
from typing import Any, Final

from sqlbuild._native import ChainStep, SqlTestPlanEntry, render_sql_test_difference_sample_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.diagnostics.classes.diagnostic_record_redactor import DiagnosticRecordRedactor
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.executor.testing.models import (
    SqlTestColumnDifference,
    SqlTestDifferenceSample,
    StepResult,
)
from sqlbuild.executor.testing.types import SqlTestDifferenceDirection, SqlTestOutcome
from sqlbuild.runtime.execution_limits.exceptions import ItemTimeoutError

_ROW_LIMIT: Final[int] = 3
_COLUMN_LIMIT: Final[int] = 12
_VALUE_LIMIT: Final[int] = 120
_OMITTED_COLUMN_NAME: Final[str] = "..."
_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")


def add_difference_samples(
    *,
    step_results: list[StepResult],
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    relations: Mapping[int, str] | None = None,
    execute: Callable[[str], Any] | None = None,
) -> list[StepResult]:
    """Attach bounded, redacted samples to failed steps, run by `execute` when given."""

    sampled_results: list[StepResult] = []
    expected_steps: tuple[tuple[int, ChainStep], ...] = tuple(
        (index, step)
        for index, step in enumerate(test_entry.chain)
        if step.expected_cte_sql is not None
    )
    stored: Mapping[int, str] = relations or {}
    for step_index, step_result in enumerate(step_results):
        if step_index >= len(expected_steps) or step_result.outcome != SqlTestOutcome.FAIL:
            sampled_results.append(step_result)
            continue
        chain_index, step = expected_steps[step_index]
        actual_relation: str | None = stored.get(chain_index)
        unexpected_samples: tuple[SqlTestDifferenceSample, ...] = ()
        missing_samples: tuple[SqlTestDifferenceSample, ...] = ()
        if step_result.unexpected_row_count is not None and step_result.unexpected_row_count > 0:
            unexpected_samples = _best_effort_difference_samples(
                test_entry=test_entry,
                step=step,
                direction=SqlTestDifferenceDirection.UNEXPECTED,
                adapter=adapter,
                connection=connection,
                actual_relation=actual_relation,
                execute=execute,
            )
        if step_result.missing_row_count is not None and step_result.missing_row_count > 0:
            missing_samples = _best_effort_difference_samples(
                test_entry=test_entry,
                step=step,
                direction=SqlTestDifferenceDirection.MISSING,
                adapter=adapter,
                connection=connection,
                actual_relation=actual_relation,
                execute=execute,
            )
        sampled_results.append(
            replace(
                step_result,
                unexpected_samples=unexpected_samples,
                missing_samples=missing_samples,
                column_differences=_align_single_difference(
                    step_result=step_result,
                    unexpected_samples=unexpected_samples,
                    missing_samples=missing_samples,
                ),
            )
        )
    return sampled_results


def _align_single_difference(
    *,
    step_result: StepResult,
    unexpected_samples: tuple[SqlTestDifferenceSample, ...],
    missing_samples: tuple[SqlTestDifferenceSample, ...],
) -> tuple[SqlTestColumnDifference, ...]:
    """Align values only when each set-difference direction contains exactly one row."""

    if step_result.unexpected_row_count != 1 or step_result.missing_row_count != 1:
        return ()
    if len(unexpected_samples) != 1 or len(missing_samples) != 1:
        return ()
    actual_values: tuple[tuple[str, str], ...] = unexpected_samples[0].values
    expected_values: tuple[tuple[str, str], ...] = missing_samples[0].values
    if any(name == _OMITTED_COLUMN_NAME for name, _ in (*actual_values, *expected_values)):
        return ()
    if tuple(name for name, _ in actual_values) != tuple(name for name, _ in expected_values):
        return ()
    return tuple(
        SqlTestColumnDifference(name=name, actual=actual, expected=expected)
        for (name, actual), (_, expected) in zip(actual_values, expected_values, strict=True)
        if actual != expected
    )


def _fetch_difference_samples(
    *,
    test_entry: SqlTestPlanEntry,
    step: ChainStep,
    direction: SqlTestDifferenceDirection,
    adapter: BaseAdapter,
    connection: Any,
    actual_relation: str | None,
    execute: Callable[[str], Any] | None,
) -> tuple[SqlTestDifferenceSample, ...]:
    sql: str = render_sql_test_difference_sample_sql(
        test_entry=test_entry,
        step=step,
        direction=direction,
        sample_limit=_ROW_LIMIT,
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
        actual_relation=actual_relation,
    )
    cursor: Any = (
        execute(sql) if execute is not None else adapter.execute(connection=connection, sql=sql)
    )
    description: Any | None = getattr(cursor, "description", None)
    if description is None:
        return ()
    column_names: tuple[str, ...] = tuple(str(column[0]) for column in description)
    if not column_names:
        return ()
    rows: list[Any] = cursor.fetchall()
    samples: list[SqlTestDifferenceSample] = []
    for row in rows[:_ROW_LIMIT]:
        values: list[tuple[str, str]] = [
            (column_name, _safe_sample_value(name=column_name, value=value))
            for column_name, value in list(zip(column_names, row, strict=False))[:_COLUMN_LIMIT]
        ]
        omitted_columns: int = max(0, len(column_names) - _COLUMN_LIMIT)
        if omitted_columns:
            values.append((_OMITTED_COLUMN_NAME, f"{omitted_columns} columns omitted"))
        samples.append(SqlTestDifferenceSample(values=tuple(values)))
    return tuple(samples)


def _best_effort_difference_samples(
    *,
    test_entry: SqlTestPlanEntry,
    step: ChainStep,
    direction: SqlTestDifferenceDirection,
    adapter: BaseAdapter,
    connection: Any,
    actual_relation: str | None,
    execute: Callable[[str], Any] | None,
) -> tuple[SqlTestDifferenceSample, ...]:
    """Return samples when available without replacing a known comparison failure."""

    try:
        return _fetch_difference_samples(
            test_entry=test_entry,
            step=step,
            direction=direction,
            adapter=adapter,
            connection=connection,
            actual_relation=actual_relation,
            execute=execute,
        )
    except ItemTimeoutError:
        raise
    except Exception as error:
        log_debug_event(
            logger=_LOGGER,
            message="SQL test difference sampling failed; preserving comparison result",
            test_name=test_entry.name,
            model_name=step.model_name,
            direction=direction.value,
            sqlbuild_error=DiagnosticRecordRedactor.text(str(error)),
        )
        return ()


def _safe_sample_value(*, name: str, value: object) -> str:
    safe_value: object = DiagnosticRecordRedactor.value(name=name, value=value)
    rendered: str = "NULL" if safe_value is None else " ".join(str(safe_value).split())
    if len(rendered) <= _VALUE_LIMIT:
        return rendered
    return f"{rendered[: _VALUE_LIMIT - 3]}..."
