"""Unit tests the run could not place, refused locally or with no warehouse, reported as errors."""

from __future__ import annotations

from collections.abc import Callable, Sequence

from sqlbuild._native import SqlTestPlanEntry, sql_test_resource_id
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)


def report_unplaced_sql_tests(
    *,
    test_entries: Sequence[SqlTestPlanEntry],
    code: str,
    message: str,
    run_id: str,
    on_test_start: Callable[[SqlTestPlanEntry], None],
    on_test_complete: Callable[[SqlTestExecutionResult], None],
) -> tuple[SqlTestExecutionResult, ...]:
    """One ERROR result per test, each reported through its own resource attempt."""

    results: list[SqlTestExecutionResult] = []
    for entry in test_entries:
        with ResourceAttemptLifecycle(
            resource_id=sql_test_resource_id(
                test_name=entry.name,
                source_path=entry.source_path,
                block_index=entry.block_index,
                case_name=entry.case_name,
            ),
            resource_kind="test",
            resource_name=entry.name,
            run_id=run_id,
        ) as lifecycle:
            on_test_start(entry)
            result: SqlTestExecutionResult = _unplaced_result(
                test_entry=entry, code=code, message=message
            )
            lifecycle.failed(error_code=code)
        on_test_complete(result)
        results.append(result)
    return tuple(results)


def _unplaced_result(
    *, test_entry: SqlTestPlanEntry, code: str, message: str
) -> SqlTestExecutionResult:
    """An ERROR result that names why the test did not run, with no warehouse or DuckDB work."""

    return SqlTestExecutionResult(
        test_name=test_entry.name,
        outcome=SqlTestOutcome.ERROR,
        source_path=test_entry.source_path,
        block_index=test_entry.block_index,
        parent_name=test_entry.parent_name,
        case_name=test_entry.case_name,
        case_index=test_entry.case_index,
        case_fingerprint=test_entry.case_fingerprint,
        parameter_schema=test_entry.parameter_schema,
        parameter_values=test_entry.parameter_values,
        error_code=code,
        error_message=message,
        real_model_count=test_entry.real_model_count,
    )
