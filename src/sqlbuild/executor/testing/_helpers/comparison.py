"""Run one unit test's comparison against its actual outputs and collect step results."""

from __future__ import annotations

import logging
from collections.abc import Callable
from decimal import Decimal
from typing import Any

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import RelationReadStatus
from sqlbuild.diagnostics.classes.diagnostic_record_redactor import DiagnosticRecordRedactor
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.executor.testing._helpers.difference_samples import add_difference_samples
from sqlbuild.executor.testing.classes.sql_test_actual_outputs import SqlTestActualOutputs
from sqlbuild.executor.testing.constants import LOCAL_COMPARISON_COLUMNS
from sqlbuild.executor.testing.models import StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome

_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")


def compare_actual_outputs(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    inline_sql: str,
    outputs: SqlTestActualOutputs,
) -> list[StepResult]:
    """Store the actual outputs once where supported, then count and sample the differences."""

    relations: dict[int, str] = outputs.materialize()
    if not relations:
        return _compare(
            test_entry=test_entry,
            adapter=adapter,
            connection=connection,
            comparison_sql=inline_sql,
            relations={},
        )
    (stored_sql,) = render_sql_test_comparison_sql(
        (test_entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
        actual_relations=relations,
    )
    try:
        return _compare(
            test_entry=test_entry,
            adapter=adapter,
            connection=connection,
            comparison_sql=stored_sql,
            relations=relations,
        )
    except Exception as error:
        if not _stored_output_missing(adapter=adapter, error=error):
            raise
        log_debug_event(
            logger=_LOGGER,
            message="SQL test output was no longer readable; comparing inline",
            test_name=test_entry.name,
            sqlbuild_error=DiagnosticRecordRedactor.text(str(error)),
        )
    outputs.release()
    return _compare(
        test_entry=test_entry,
        adapter=adapter,
        connection=connection,
        comparison_sql=inline_sql,
        relations={},
    )


def _compare(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    comparison_sql: str,
    relations: dict[int, str],
) -> list[StepResult]:
    cursor: Any = adapter.execute(connection=connection, sql=comparison_sql)
    rows: list[Any] = cursor.fetchall()
    return add_difference_samples(
        step_results=build_step_results(rows),
        test_entry=test_entry,
        adapter=adapter,
        connection=connection,
        relations=relations,
    )


def _stored_output_missing(*, adapter: BaseAdapter, error: BaseException) -> bool:
    """Whether a read failed because a stored output relation no longer exists."""

    classify: Callable[[BaseException], RelationReadStatus | None] | None = getattr(
        adapter, "_classify_relation_not_found", None
    )
    if classify is None:
        return False
    current: BaseException | None = error
    seen: set[int] = set()
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        if classify(current) is RelationReadStatus.MISSING:
            return True
        current = current.__cause__ or current.__context__
    return False


def build_step_results(rows: list[Any]) -> list[StepResult]:
    """Convert comparison query rows into per-model step results."""

    step_results: list[StepResult] = []
    row: Any
    for row in sorted(rows, key=lambda item: int(item[0])):
        model_name: str = str(row[1])
        actual_count: int = int(row[2])
        expected_count: int = int(row[3])
        unexpected_count: int = int(row[4])
        missing_count: int = int(row[5])
        outcome: SqlTestOutcome
        if unexpected_count == 0 and missing_count == 0 and actual_count == expected_count:
            outcome = SqlTestOutcome.PASS
        else:
            outcome = SqlTestOutcome.FAIL
        step_results.append(
            StepResult(
                model_name=model_name,
                outcome=outcome,
                actual_row_count=actual_count,
                expected_row_count=expected_count,
                mismatched_row_count=unexpected_count,
                unexpected_row_count=unexpected_count,
                missing_row_count=missing_count,
            )
        )
    return step_results


def plan_row_mismatch(*, rows: tuple[Any, ...], test_entry: SqlTestPlanEntry) -> str | None:
    """Why comparison rows do not match the test's rendered plan, or `None` when they do."""

    expected: list[tuple[int, str]] = [
        (index, step.model_name)
        for index, step in enumerate(test_entry.chain)
        if step.expected_cte_sql is not None
    ]
    expected.extend(
        (len(test_entry.chain) + index, f"assertion {assertion.name}")
        for index, assertion in enumerate(test_entry.assertions)
    )
    if not all(_comparison_row(row) for row in rows):
        return f"a comparison row is not {LOCAL_COMPARISON_COLUMNS} counted columns"
    actual: list[tuple[int, str]] = sorted((int(row[0]), row[1]) for row in rows)
    if actual != expected:
        return f"the comparison returned steps {actual}, but the plan has steps {expected}"
    return None


def _comparison_row(row: Any) -> bool:
    if not isinstance(row, tuple) or len(row) != LOCAL_COMPARISON_COLUMNS:
        return False
    counts: tuple[Any, ...] = (row[0], *row[2:])
    return isinstance(row[1], str) and all(_count(value) for value in counts)


def _count(value: Any) -> bool:
    """A whole non-negative number, as an integer or a NUMBER(38, 0) decimal."""

    if isinstance(value, bool) or not isinstance(value, int | Decimal):
        return False
    if isinstance(value, Decimal) and not value.is_finite():
        return False
    return value >= 0 and value == int(value)
