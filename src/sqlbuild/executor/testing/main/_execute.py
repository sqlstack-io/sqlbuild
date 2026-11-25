"""SQL unit test execution."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.errors.contracts.exceptions import LocalRunRefusedError
from sqlbuild.executor.testing._helpers.comparison import compare_actual_outputs
from sqlbuild.executor.testing._helpers.expected_column_probe import (
    missing_expected_columns_message,
)
from sqlbuild.executor.testing._helpers.timeout_policy import sql_test_timeout
from sqlbuild.executor.testing._helpers.timeout_result import timed_out_result
from sqlbuild.executor.testing.classes.sql_test_actual_outputs import SqlTestActualOutputs
from sqlbuild.executor.testing.constants import (
    SQL_TEST_ASSERTION_FAILED_CODE,
    SQL_TEST_EXECUTION_ERROR_CODE,
    SQL_TEST_EXPECTED_COLUMNS_CODE,
    SQL_TEST_TOO_LARGE_CODE,
)
from sqlbuild.executor.testing.main._sql_length import (
    validate_unit_test_sql_length,
)
from sqlbuild.executor.testing.models import SqlTestExecutionResult, StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.exceptions import ItemTimeoutError
from sqlbuild.runtime.execution_limits.main.item_deadline_scope import item_deadline_scope
from sqlbuild.runtime.execution_limits.models import RunItemTimeouts
from sqlbuild.runtime.execution_limits.types import ItemKind
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle
from sqlbuild.runtime.observability.models import OperationAttributes


def execute_sql_test(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    quality_scope: str = "standalone",
    timeouts: RunItemTimeouts | None = None,
) -> SqlTestExecutionResult:
    """Execute one SQL unit test within its time limit, cancelling warehouse work at expiry."""

    with item_deadline_scope(
        kind=ItemKind.TEST,
        name=test_entry.name,
        policy=sql_test_timeout(entry=test_entry, timeouts=timeouts),
        adapter=adapter,
        connection=connection,
    ) as deadline:
        try:
            result: SqlTestExecutionResult = _execute_sql_test(
                test_entry=test_entry,
                adapter=adapter,
                connection=connection,
                quality_scope=quality_scope,
            )
        except ItemTimeoutError:
            if deadline is None:
                raise
            return timed_out_result(test_entry=test_entry, report=deadline.report())
    if deadline is not None and deadline.expired and result.outcome == SqlTestOutcome.ERROR:
        return timed_out_result(test_entry=test_entry, report=deadline.report())
    return replace(result, real_model_count=test_entry.real_model_count)


def _execute_sql_test(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    quality_scope: str,
) -> SqlTestExecutionResult:
    """Execute one SQL unit test and collect bounded diagnostics for failed comparisons."""

    (comparison_sql,) = render_sql_test_comparison_sql(
        (test_entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )
    error_model_name: str = next(
        (step.model_name for step in test_entry.chain if step.expected_cte_sql is not None),
        test_entry.name,
    )
    try:
        validate_unit_test_sql_length(
            sql=comparison_sql,
            adapter=adapter,
            test_name=test_entry.name,
            model_name=error_model_name,
        )
    except CompileInputError as error:
        error_message: str = str(error)
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
                    model_name=error_model_name,
                    outcome=SqlTestOutcome.ERROR,
                    error_code=SQL_TEST_TOO_LARGE_CODE,
                    error_message=error_message,
                ),
            ),
            error_code=SQL_TEST_TOO_LARGE_CODE,
            error_message=error_message,
        )

    with OperationLifecycle(
        operation_kind="quality",
        operation_name="sql_test_assertion",
        metadata={"item_count": len(test_entry.chain) + len(test_entry.assertions)},
        attributes=OperationAttributes(phase="assert", target_kind="sql_test", scope=quality_scope),
    ) as lifecycle:
        step_results: list[StepResult] = []
        failure: tuple[Exception, str | None] | None = None
        with SqlTestActualOutputs(
            test_entry=test_entry, adapter=adapter, connection=connection
        ) as outputs:
            try:
                step_results = compare_actual_outputs(
                    test_entry=test_entry,
                    adapter=adapter,
                    connection=connection,
                    inline_sql=comparison_sql,
                    outputs=outputs,
                )
            except ItemTimeoutError:
                raise
            except Exception as error:
                failure = (
                    error,
                    missing_expected_columns_message(
                        test_entry=test_entry,
                        adapter=adapter,
                        connection=connection,
                        relations=outputs.relations,
                    ),
                )
        if failure is not None:
            error, expected_columns_message = failure
            refused: LocalRunRefusedError | None = (
                error if isinstance(error, LocalRunRefusedError) else None
            )
            error_code: str = (
                refused.code
                if refused is not None
                else SQL_TEST_EXECUTION_ERROR_CODE
                if expected_columns_message is None
                else SQL_TEST_EXPECTED_COLUMNS_CODE
            )
            lifecycle.failed(error=error, error_code=error_code)
            error_message = expected_columns_message or (
                f"test '{test_entry.name}' encountered an execution error while running "
                f"'{error_model_name}': {error}"
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
                        model_name=error_model_name,
                        outcome=SqlTestOutcome.ERROR,
                        error_code=error_code,
                        error_help=None if refused is None else refused.help,
                        error_message=error_message,
                    ),
                ),
                error_code=error_code,
                error_help=None if refused is None else refused.help,
                error_message=error_message,
                local_refusal=None
                if refused is None
                else {
                    "test": test_entry.name,
                    "category": refused.category,
                    "construct": refused.construct,
                    "reason": refused.message,
                },
            )

        overall_outcome: SqlTestOutcome = SqlTestOutcome.PASS
        step_result: StepResult
        for step_result in step_results:
            if step_result.outcome == SqlTestOutcome.FAIL:
                overall_outcome = SqlTestOutcome.FAIL
        lifecycle.completed()

    error_message: str | None = None
    if overall_outcome == SqlTestOutcome.FAIL:
        failed_models: list[str] = [
            r.model_name for r in step_results if r.outcome == SqlTestOutcome.FAIL
        ]
        error_message = f"test '{test_entry.name}' failed for models: {', '.join(failed_models)}"

    return SqlTestExecutionResult(
        test_name=test_entry.name,
        outcome=overall_outcome,
        source_path=test_entry.source_path,
        block_index=test_entry.block_index,
        parent_name=test_entry.parent_name,
        case_name=test_entry.case_name,
        case_index=test_entry.case_index,
        case_fingerprint=test_entry.case_fingerprint,
        parameter_schema=test_entry.parameter_schema,
        parameter_values=test_entry.parameter_values,
        step_results=tuple(step_results),
        error_code=SQL_TEST_ASSERTION_FAILED_CODE
        if overall_outcome == SqlTestOutcome.FAIL
        else None,
        error_message=error_message,
    )
