"""The result of a unit test the run could not place: refused locally, or with no warehouse."""

from __future__ import annotations

from sqlbuild._native import SqlTestPlanEntry
from sqlbuild.executor.testing.models import SqlTestExecutionResult, StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome


def unplaced_sql_test_result(
    *, test_entry: SqlTestPlanEntry, code: str, message: str
) -> SqlTestExecutionResult:
    """An ERROR result that names why the test did not run, with no warehouse or DuckDB work."""

    model_name: str = next(
        (step.model_name for step in test_entry.chain if step.expected_cte_sql is not None),
        test_entry.name,
    )
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
        step_results=(
            StepResult(
                model_name=model_name,
                outcome=SqlTestOutcome.ERROR,
                error_code=code,
                error_message=message,
            ),
        ),
        error_code=code,
        error_message=message,
        real_model_count=test_entry.real_model_count,
    )
