"""Unit tests for build progress output helpers."""

from __future__ import annotations

import time
from dataclasses import replace
from io import StringIO

import pytest

from sqlbuild._native import (
    BuildProgressCallbacks,
    NestedCommandProgressCallbacks,
    NestedProgressChildRow,
    PlanOutput,
    build_test_expectation_rows,
    format_build_footer,
    resolve_test_name_width,
    write_execution_header,
)
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind
from sqlbuild.compiler.auditing.types import AuditEvaluationMode, AuditOutcome
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.cost.models import StatementExecutionTelemetry
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.build.models import (
    BuildExecutionResult,
    FunctionExecutionResult,
    SchedulerState,
    SeedExecutionResult,
)
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.testing.models import SqlTestExecutionResult, StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from tests.unit.src.sqlbuild.cli.commands.shared._helpers._test_types import (
    BuildFooterMeasurementTestCase,
    BuildFooterTestCase,
    BuildProgressActiveSpinnerTestCase,
    BuildProgressFailureOutputTestCase,
    BuildProgressLoadLogTestCase,
    BuildProgressLoadSkipOutputTestCase,
    BuildProgressModelOutputTestCase,
    BuildProgressSpinnerLifecycleTestCase,
    BuildProgressSqlTestRowsTestCase,
    ExecutionHeaderTestCase,
    NestedProgressChildRowsTestCase,
    NestedProgressConcurrentTestCase,
    RuntimeDiagnosticsTestCase,
    SqlTestStatusAlignmentTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.shared._helpers.helpers import (
    build_audit_result,
    build_progress_snapshot_plan_output,
    build_sql_test_plan_entry,
    build_sql_test_result,
    status_columns,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeDiagnosticsTestCase(
            description="reports constrained frontier and query identity without SQL",
            expected_fragments=(
                "Scheduler  2 running, 0 ready, 173 waiting on dependencies, limit 5",
                "DAG frontier constrained",
                "Query  model orders  phase=materialize  01c-query-id  success  1.25s",
            ),
            expected_absent_fragments=("SELECT",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_verbose_runtime_diagnostics_when_reporting_then_writes_concise_raw_output(
    test_case: RuntimeDiagnosticsTestCase,
    capsys: pytest.CaptureFixture[str],
) -> None:
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(), use_color=False, verbose=True, debug=False
    )

    callbacks.on_scheduler_state(SchedulerState(running=2, ready=0, waiting=173, limit=5))
    callbacks.on_statement_complete(
        StatementExecutionTelemetry(
            query_id="01c-query-id",
            status="success",
            elapsed_seconds=1.25,
            resource_type="model",
            resource_name="orders",
            phase="materialize",
        )
    )
    callbacks.close()

    output: str = capsys.readouterr().out
    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    absent_fragment: str
    for absent_fragment in test_case.expected_absent_fragments:
        assert absent_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeDiagnosticsTestCase(
            description="large runtime diagnostics are bounded with final identity summaries",
            expected_fragments=(
                "Scheduler diagnostics  2 transitions omitted after 50",
                "51 waiting on dependencies",
                "Query diagnostics  2 statements omitted after 25 per resource",
                "query-26",
            ),
            expected_absent_fragments=("query-25  success",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_high_volume_runtime_diagnostics_when_closing_then_output_is_bounded(
    test_case: RuntimeDiagnosticsTestCase,
    capsys: pytest.CaptureFixture[str],
) -> None:
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(), use_color=False, verbose=True, debug=False
    )

    for index in range(52):
        callbacks.on_scheduler_state(SchedulerState(running=1, ready=0, waiting=index, limit=4))
    for index in range(27):
        callbacks.on_statement_complete(
            StatementExecutionTelemetry(
                query_id=f"query-{index}",
                status="success",
                elapsed_seconds=0.1,
                resource_type="model",
                resource_name="orders",
                phase="microbatch",
            )
        )
    callbacks.on_statement_complete(
        StatementExecutionTelemetry(
            query_id="customers-query",
            status="success",
            elapsed_seconds=0.1,
            resource_type="model",
            resource_name="customers",
            phase="materialize",
        )
    )
    callbacks.close()

    output: str = capsys.readouterr().out
    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    absent_fragment: str
    for absent_fragment in test_case.expected_absent_fragments:
        assert absent_fragment not in output
    assert output.count("Scheduler  ") == 51
    assert output.count("Query  model orders") == 26
    assert "Query  model customers  phase=materialize  customers-query" in output


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeDiagnosticsTestCase(
            description="terminal scheduler state reports aborted work without frontier wording",
            expected_fragments=("3 aborted after stop",),
            expected_absent_fragments=(
                "DAG frontier constrained",
                "waiting on dependencies, limit",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_terminal_scheduler_state_when_reporting_then_output_explains_aborted_work(
    test_case: RuntimeDiagnosticsTestCase,
    capsys: pytest.CaptureFixture[str],
) -> None:
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(), use_color=False, verbose=True, debug=False
    )

    callbacks.on_scheduler_state(SchedulerState(running=0, ready=0, waiting=0, limit=4, aborted=3))
    callbacks.close()

    output: str = capsys.readouterr().out
    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    absent_fragment: str
    for absent_fragment in test_case.expected_absent_fragments:
        assert absent_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        NestedProgressChildRowsTestCase(
            description="completed item renders aligned child expectation rows without truncation",
            item_name="test_fact_orders",
            name_width=len("assertion line_totals_are_non_negative"),
            expected_fragments=(
                "test      test_fact_orders",
                "expect  expected fact_orders",
                "expect  assertion line_totals_are_non_negative",
                "expected fact_orders                           PASS",
            ),
            unexpected_fragments=("...",),
        ),
        NestedProgressChildRowsTestCase(
            description="execution error preserves status and nests diagnostic",
            item_name="test_daily_revenue_chain",
            name_width=len("test_daily_revenue_chain"),
            status_text="ERROR",
            child_status_text="ERROR",
            error_code="T002",
            error_message="execution error while running stg_orders",
            expected_fragments=(
                "test      test_daily_revenue_chain",
                "ERROR",
                "├── expect  expected fact_orders",
                "└── error   [T002] execution error while running stg_orders",
            ),
            unexpected_fragments=("0 mismatched",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_child_rows_when_completing_nested_progress_then_renders_aligned_expectations(
    test_case: NestedProgressChildRowsTestCase,
) -> None:
    stream: StringIO = StringIO()
    callbacks: NestedCommandProgressCallbacks = NestedCommandProgressCallbacks(
        total=1,
        label="test",
        stream=stream,
        use_color=False,
        name_width=test_case.name_width,
    )

    callbacks.on_item_start(group_name="fact_orders", item_name=test_case.item_name)
    callbacks.on_item_complete(
        group_name="fact_orders",
        item_name=test_case.item_name,
        status_text=test_case.status_text,
        child_rows=(
            NestedProgressChildRow(
                label="expect",
                name="expected fact_orders",
                status_text=test_case.child_status_text,
            ),
            NestedProgressChildRow(
                label="expect",
                name="assertion line_totals_are_non_negative",
                status_text=test_case.child_status_text,
            ),
        ),
        error_code=test_case.error_code,
        error_message=test_case.error_message,
    )
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        NestedProgressConcurrentTestCase(
            description="concurrent items print each group header with its completed row",
            concurrency=2,
            expected_output=(
                "stg_orders\n"
                "    test      test_stg_orders" + " " * 35 + " PASS\n"
                "\n"
                "fact_orders\n"
                "    test      test_fact_orders" + " " * 34 + " PASS\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_overlapping_items_when_completing_out_of_order_then_headers_follow_results(
    test_case: NestedProgressConcurrentTestCase,
) -> None:
    stream: StringIO = StringIO()
    callbacks: NestedCommandProgressCallbacks = NestedCommandProgressCallbacks(
        total=2, label="test", stream=stream, use_color=False, concurrency=test_case.concurrency
    )

    callbacks.on_item_start(group_name="fact_orders", item_name="test_fact_orders")
    callbacks.on_item_start(group_name="stg_orders", item_name="test_stg_orders")
    callbacks.on_item_complete(
        group_name="stg_orders", item_name="test_stg_orders", status_text="PASS"
    )
    callbacks.on_item_complete(
        group_name="fact_orders", item_name="test_fact_orders", status_text="PASS"
    )

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildFooterTestCase(
            description="failed function result includes failure count and error",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order",
                        status=ExecutionStatus.FAILED,
                        function_kind="udf",
                        error_message="warehouse said no",
                    ),
                ),
            ),
            expected_fragments=(
                "FAIL=1",
                "is_completed_order  (udf)",
                "warehouse said no",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildFooterTestCase(
            description="failed seed result includes failure count and error",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                seed_results=(
                    SeedExecutionResult(
                        seed_name="waffle_types",
                        status=ExecutionStatus.FAILED,
                        error_message="failed to load seed CSV",
                    ),
                ),
            ),
            expected_fragments=(
                "FAIL=1",
                "waffle_types  (seed)",
                "failed to load seed CSV",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildFooterTestCase(
            description="failed loader result includes complete multiline error",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                load_results=(
                    LoadExecutionResult(
                        source_name="raw__orders",
                        loader_name="load_orders",
                        status=ExecutionStatus.FAILED,
                        target="dev.raw__orders",
                        error_message="loader line one\nloader line two\nloader line three",
                    ),
                ),
            ),
            expected_fragments=(
                "FAIL=1",
                "raw__orders  (source)",
                "loader line one",
                "loader line two",
                "loader line three",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildFooterTestCase(
            description="function warning appears in summary and warnings section",
            result=BuildExecutionResult(
                status=BuildStatus.SUCCESS,
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order_py",
                        status=ExecutionStatus.SUCCESS,
                        function_kind="udf",
                        warning_messages=("fingerprint write skipped",),
                    ),
                ),
                success_count=1,
                warning_count=1,
            ),
            expected_fragments=(
                "\u2713 Completed with warnings",
                "PASS=1",
                "WARN=1",
                "Warnings:",
                "is_completed_order_py  (udf)",
                "fingerprint write skipped",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildFooterTestCase(
            description="model warning counts the model as warned and precedes the completion line",
            result=BuildExecutionResult(
                status=BuildStatus.SUCCESS,
                model_results=(
                    ModelExecutionResult(
                        model_name="orders",
                        status=ExecutionStatus.SUCCESS,
                        warning_messages=("[P008] hook 'refresh_lookup' named model:customers",),
                    ),
                ),
                success_count=1,
                warning_count=1,
            ),
            expected_fragments=(
                "Warnings:",
                "[P008] hook 'refresh_lookup' named model:customers\n\n\u2713 Completed with warnings",
                "PASS=0",
                "WARN=1",
                "TOTAL=1",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildFooterTestCase(
            description="warning footer uses semantic colors",
            result=BuildExecutionResult(
                status=BuildStatus.SUCCESS,
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order_py",
                        status=ExecutionStatus.SUCCESS,
                        function_kind="udf",
                        warning_messages=("fingerprint write skipped",),
                    ),
                ),
                success_count=1,
                warning_count=1,
            ),
            expected_fragments=(
                "\033[33m\u2713\033[0m \033[33m\033[1mCompleted with warnings\033[0m",
                "\033[33m\033[1mWarnings:\033[0m",
            ),
            use_color=True,
        ),
        BuildFooterTestCase(
            description="failure footer uses semantic colors",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order",
                        status=ExecutionStatus.FAILED,
                        function_kind="udf",
                        error_message="warehouse said no",
                    ),
                ),
            ),
            expected_fragments=(
                "\033[38;5;167m\u2717\033[0m \033[38;5;167m\033[1mCompleted with errors\033[0m",
                "\033[38;5;167m\033[1mFailures:\033[0m",
                "\033[38;5;167m\033[2merror\033[0m",
            ),
            use_color=True,
        ),
        BuildFooterTestCase(
            description="footer failure preserves the complete error text",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order",
                        status=ExecutionStatus.FAILED,
                        function_kind="udf",
                        error_message=(
                            "line one\nline two\nline three\nline four\n"
                            "line five should appear " + "x" * 170 + " final characters"
                        ),
                    ),
                ),
            ),
            expected_fragments=(
                "Failures:",
                "error     line one",
                "line two",
                "line three",
                "line four",
                "line five should appear",
                "final characters",
            ),
            unexpected_fragments=("line four...", "\033["),
        ),
        BuildFooterTestCase(
            description="footer lists skipped nodes in a dim capped rollup",
            result=BuildExecutionResult(
                status=BuildStatus.FAILED,
                model_results=(
                    ModelExecutionResult(
                        model_name="stg_orders",
                        status=ExecutionStatus.SKIPPED,
                    ),
                    ModelExecutionResult(
                        model_name="fact_orders",
                        status=ExecutionStatus.SKIPPED,
                    ),
                ),
                function_results=(
                    FunctionExecutionResult(
                        function_name="is_completed_order",
                        status=ExecutionStatus.FAILED,
                        function_kind="udf",
                        error_message="warehouse said no",
                    ),
                ),
            ),
            expected_fragments=(
                "Failures:",
                "Skipped (2): stg_orders, fact_orders",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_resource_result_when_formatting_footer_then_includes_error(
    test_case: BuildFooterTestCase,
) -> None:
    footer: str = format_build_footer(
        result=test_case.result, elapsed=1.25, use_color=test_case.use_color
    )

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in footer
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in footer


@pytest.mark.parametrize(
    "test_case",
    [
        BuildFooterMeasurementTestCase(description="integer text", value=1, expected_text="1"),
        BuildFooterMeasurementTestCase(
            description="whole float text", value=1.0, expected_text="1.0"
        ),
        BuildFooterMeasurementTestCase(
            description="scientific notation", value=0.000001, expected_text="1e-06"
        ),
        BuildFooterMeasurementTestCase(description="signed zero", value=-0.0, expected_text="-0.0"),
        BuildFooterMeasurementTestCase(
            description="missing measurement", value=None, expected_text="None"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_warning_when_formatting_footer_then_preserves_numeric_text(
    test_case: BuildFooterMeasurementTestCase,
) -> None:
    audit: AuditExecutionResult = replace(
        build_audit_result(name="order_count", outcome=AuditOutcome.WARN),
        evaluation_mode=AuditEvaluationMode.MEASUREMENT,
        measured_value=test_case.value,
    )
    result: BuildExecutionResult = BuildExecutionResult(
        status=BuildStatus.SUCCESS,
        model_results=(
            ModelExecutionResult(
                model_name="orders",
                status=ExecutionStatus.SUCCESS,
                audit_results=(audit,),
            ),
        ),
        warning_count=1,
    )

    footer: str = format_build_footer(result=result, elapsed=1.25, use_color=False)

    assert f"audit order_count measured {test_case.expected_text}\n" in footer


@pytest.mark.parametrize(
    "test_case",
    [
        ExecutionHeaderTestCase(
            description="renders no-color execution header",
            command="sqb build",
            target=None,
            concurrency=1,
            use_color=False,
            expected_output="Execution  sqb build  (concurrency: 1)\n\n",
        ),
        ExecutionHeaderTestCase(
            description="renders colored execution header",
            command="sqb build",
            target=None,
            concurrency=1,
            use_color=True,
            expected_output=(
                "\033[34m\033[1mExecution\033[0m  \033[2msqb build  (concurrency: 1)\033[0m\n\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_execution_context_when_writing_header_then_renders_expected_output(
    test_case: ExecutionHeaderTestCase,
) -> None:
    stream: StringIO = StringIO()

    write_execution_header(
        stream=stream,
        command=test_case.command,
        target=test_case.target,
        concurrency=test_case.concurrency,
        use_color=test_case.use_color,
    )

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressFailureOutputTestCase(
            description="failed seed writes error detail below result row",
            node_result=SeedExecutionResult(
                seed_name="waffle_types",
                status=ExecutionStatus.FAILED,
                duration_ms=30,
                error_message="failed to load seed CSV",
            ),
            expected_fragments=(
                "seed      waffle_types",
                "FAIL",
                "0.03s",
                "error     failed to load seed CSV",
            ),
            unexpected_fragments=("0.03s  failed to load seed CSV", "\033["),
        ),
        BuildProgressFailureOutputTestCase(
            description="failed function writes multiline error detail below result row",
            node_result=FunctionExecutionResult(
                function_name="is_completed_order",
                status=ExecutionStatus.FAILED,
                function_kind="udf",
                duration_ms=110,
                error_message=(
                    "003001 (42501): SQL access control error:\n"
                    "Insufficient privileges to operate on schema 'TEST'."
                ),
            ),
            expected_fragments=(
                "udf       is_completed_order",
                "FAIL",
                "0.11s",
                "error     003001 (42501): SQL access control error:",
                "          Insufficient privileges to operate on schema 'TEST'.",
            ),
            unexpected_fragments=("0.11s  003001", "\033["),
        ),
        BuildProgressFailureOutputTestCase(
            description="failed function colors status and error label semantically",
            node_result=FunctionExecutionResult(
                function_name="is_completed_order",
                status=ExecutionStatus.FAILED,
                function_kind="udf",
                duration_ms=110,
                error_message="warehouse said no",
            ),
            expected_fragments=(
                "\033[38;5;167mFAIL\033[0m",
                "\033[38;5;167m\033[2merror\033[0m",
                "warehouse said no",
            ),
            use_color=True,
        ),
        BuildProgressFailureOutputTestCase(
            description="failed model keeps phase on row and writes error below",
            node_result=ModelExecutionResult(
                model_name="fact_orders",
                status=ExecutionStatus.FAILED,
                failed_phase=ExecutionPhase.STAGING,
                duration_ms=420,
                error_message="relation raw_orders does not exist",
            ),
            expected_fragments=(
                "table     fact_orders",
                "FAIL",
                "0.42s  staging",
                "error     relation raw_orders does not exist",
            ),
            unexpected_fragments=("staging  relation raw_orders does not exist", "\033["),
        ),
        BuildProgressFailureOutputTestCase(
            description="live error detail wraps to two terminal lines",
            node_result=FunctionExecutionResult(
                function_name="is_completed_order",
                status=ExecutionStatus.FAILED,
                function_kind="udf",
                duration_ms=110,
                error_message=(
                    "line one with enough content to wrap across the available terminal width "
                    + "middle diagnostic context " * 10
                    + "final detail should not appear"
                ),
            ),
            expected_fragments=(
                "error     line one with enough content",
                "terminal width middle diagnostic context middle diagnostic...",
            ),
            unexpected_fragments=("final detail should not appear", "\033["),
            terminal_width=80,
        ),
        BuildProgressFailureOutputTestCase(
            description="failing test writes its row and error inline without a host model",
            node_result=SqlTestExecutionResult(
                test_name="test_daily_revenue_chain",
                outcome=SqlTestOutcome.ERROR,
                step_results=(
                    StepResult(
                        model_name="stg_orders",
                        outcome=SqlTestOutcome.ERROR,
                    ),
                ),
                error_code="T002",
                error_message="execution error while running 'stg_orders'",
            ),
            expected_fragments=(
                "test      test_daily_revenu...",
                "ERROR",
                "├── expect  expected stg_orders",
                "└── error   [T002] execution error while running 'stg_orders'",
            ),
            unexpected_fragments=("0 mismatched", "\033["),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_top_level_node_when_reporting_progress_then_writes_error_detail_below_row(
    test_case: BuildProgressFailureOutputTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    monkeypatch.setenv("COLUMNS", str(test_case.terminal_width))
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=test_case.use_color,
    )

    callbacks.on_node_complete(test_case.node_result)
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressModelOutputTestCase(
            description="completed snapshot row shows strategy and historical shape annotation",
            node_result=ModelExecutionResult(
                model_name="customer_snapshot",
                status=ExecutionStatus.SUCCESS,
                duration_ms=120,
            ),
            plan_output=build_progress_snapshot_plan_output(
                observed_at_column="loaded_at",
                historical_input="changes",
            ),
            expected_fragments=(
                "snapshot  customer_snapshot  (timestamp, historical changes)",
                "OK",
                "0.12s",
            ),
            unexpected_fragments=("table",),
        ),
        BuildProgressModelOutputTestCase(
            description="completed model row uses semantic status color",
            node_result=ModelExecutionResult(
                model_name="fact_orders",
                status=ExecutionStatus.SUCCESS,
                duration_ms=100,
            ),
            plan_output=PlanOutput(),
            expected_fragments=("\033[32mOK\033[0m", "0.10s"),
            use_color=True,
        ),
        BuildProgressModelOutputTestCase(
            description="microbatch model streams batch count range and row count",
            node_result=ModelExecutionResult(
                model_name="events",
                status=ExecutionStatus.SUCCESS,
                duration_ms=84400,
                batch_count=5,
                rows_affected=32689379,
                cursor_range_start="2014-01-01",
                cursor_range_end="2015-01-01",
                cursor_type="timestamp",
                cursor_grain="day",
            ),
            plan_output=PlanOutput(),
            expected_fragments=("5 batches", "2014-01-01", "2014-12-31", "32.7M rows"),
        ),
        BuildProgressModelOutputTestCase(
            description="non-microbatch model does not stream batch summary",
            node_result=ModelExecutionResult(
                model_name="dim_customers",
                status=ExecutionStatus.SUCCESS,
                duration_ms=100,
            ),
            plan_output=PlanOutput(),
            expected_fragments=("OK",),
            unexpected_fragments=("batches", "range"),
        ),
        BuildProgressModelOutputTestCase(
            description="completed model row renders reused audit detail",
            node_result=ModelExecutionResult(
                model_name="fact_orders",
                status=ExecutionStatus.SUCCESS,
                duration_ms=100,
                audit_results=(
                    build_audit_result(
                        name="orders_id_not_null",
                        outcome=AuditOutcome.PASS,
                        target_name="fact_orders",
                        reused=True,
                    ),
                ),
            ),
            plan_output=PlanOutput(),
            expected_fragments=("audit", "orders_id_not_null", "PASS", "proof reused"),
            unexpected_fragments=("rows",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_node_when_reporting_progress_then_writes_materialization_label(
    test_case: BuildProgressModelOutputTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=test_case.plan_output,
        use_color=test_case.use_color,
    )

    callbacks.on_node_complete(test_case.node_result)
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressLoadLogTestCase(
            description="completed source load renders multiline lifecycle logs",
            expected_fragments=(
                "source    raw_orders",
                "log  ingestr stdout",
                "         Starting data ingestion",
                "         Rows: 3",
            ),
            unexpected_fragments=("\033[",),
        ),
        BuildProgressLoadLogTestCase(
            description="completed source load colors log label and content semantically",
            expected_fragments=(
                "\033[2m    log  \033[0m",
                "\033[2mingestr stdout\033[0m",
                "\033[2m         Starting data ingestion\033[0m",
            ),
            use_color=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_load_logs_when_reporting_progress_then_writes_indented_log_block(
    test_case: BuildProgressLoadLogTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=test_case.use_color,
    )

    callbacks.on_node_complete(
        LoadExecutionResult(
            source_name="raw_orders",
            loader_name="ingestr__raw_orders",
            status=ExecutionStatus.SUCCESS,
            target="raw_orders",
            lifecycle_events=(
                LifeCycleEvent(
                    kind=LifeCycleEventKind.LOG,
                    content="ingestr stdout\nStarting data ingestion\nRows: 3",
                ),
            ),
        )
    )
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressLoadSkipOutputTestCase(
            description="skipped loader renders skip mode and reason",
            node_result=LoadExecutionResult(
                source_name="raw_orders",
                loader_name="load_raw_orders",
                status=ExecutionStatus.SKIPPED,
                target="raw_orders",
                resource_kind=ExecutionResourceKind.LOADER,
                skip_mode=SkipMode.SOFT,
                skip_reason="no source changes",
            ),
            expected_fragments=("SKIP", "soft skip: no source changes"),
            unexpected_fragments=("rows=0",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_skipped_source_load_when_reporting_progress_then_writes_skip_detail(
    test_case: BuildProgressLoadSkipOutputTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=test_case.use_color,
    )

    callbacks.on_node_complete(test_case.node_result)
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressSqlTestRowsTestCase(
            description="completed model renders sql unit test expectation rows",
            expected_fragments=(
                "test      test_fact_orders",
                "expect  expected fact_orders",
                "expect  assertion line_totals_are_non_negative PASS",
            ),
            unexpected_fragments=("...",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_unit_test_result_when_reporting_model_progress_then_writes_expectation_rows(
    test_case: BuildProgressSqlTestRowsTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=False,
    )

    callbacks.on_node_complete(
        SqlTestExecutionResult(
            test_name="test_fact_orders",
            outcome=SqlTestOutcome.PASS,
            step_results=(
                StepResult(model_name="fact_orders", outcome=SqlTestOutcome.PASS),
                StepResult(
                    model_name="assertion line_totals_are_non_negative",
                    outcome=SqlTestOutcome.PASS,
                ),
            ),
        )
    )
    callbacks.on_node_complete(
        ModelExecutionResult(
            model_name="fact_orders",
            status=ExecutionStatus.SUCCESS,
            duration_ms=100,
        )
    )
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


class _TtyStringIO(StringIO):
    def isatty(self) -> bool:
        return True


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressActiveSpinnerTestCase(
            description="active UDF row uses spinner glyph instead of ellipsis",
            node_name="is_completed_order",
            node_type=ExecutionResourceKind.UDF,
            expected_fragments=("udf", "is_completed_order", "⠋"),
            unexpected_fragments=("...",),
        ),
        BuildProgressActiveSpinnerTestCase(
            description="active view row uses spinner glyph instead of ellipsis",
            node_name="stg_customers",
            node_type=ExecutionResourceKind.VIEW,
            expected_fragments=("view", "stg_customers", "⠋"),
            unexpected_fragments=("...",),
        ),
        BuildProgressActiveSpinnerTestCase(
            description="active snapshot row uses snapshot resource type",
            node_name="customer_snapshot",
            node_type=ExecutionResourceKind.SNAPSHOT,
            expected_fragments=("snapshot", "customer_snapshot", "⠋"),
            unexpected_fragments=("table",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_active_top_level_node_when_reporting_progress_then_uses_spinner_glyph(
    test_case: BuildProgressActiveSpinnerTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: _TtyStringIO = _TtyStringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=False,
    )

    callbacks.on_node_start(name=test_case.node_name, resource_kind=test_case.node_type)
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildProgressSpinnerLifecycleTestCase(
            description="active spinner advances frames over time before completion",
            node_name="stg_orders",
            node_type=ExecutionResourceKind.VIEW,
            timeout_seconds=1.0,
            completion_duration_ms=1200,
            expected_fragments=("view", "stg_orders", "OK", "\033[?25l", "\033[?25h"),
            expected_spinner_frames=("⠋", "⠙", "⠹"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_active_top_level_node_when_waiting_then_spinner_advances_frames(
    test_case: BuildProgressSpinnerLifecycleTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    stream: _TtyStringIO = _TtyStringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(),
        use_color=False,
    )

    callbacks.on_node_start(name=test_case.node_name, resource_kind=test_case.node_type)
    try:
        time.sleep(test_case.timeout_seconds)
    finally:
        callbacks.on_node_complete(
            ModelExecutionResult(
                model_name=test_case.node_name,
                status=ExecutionStatus.SUCCESS,
                duration_ms=test_case.completion_duration_ms,
            )
        )
    output: str = stream.getvalue()

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in output
    spinner_frame: str
    for spinner_frame in test_case.expected_spinner_frames:
        assert spinner_frame in output


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestStatusAlignmentTestCase(
            description="short names share one status column",
            test_name="test_daily_revenue_chain",
            expected_models=("stg_orders", "daily_revenue"),
            assertion_names=("revenue_is_non_negative",),
            expected_row_count=4,
        ),
        SqlTestStatusAlignmentTestCase(
            description="longest name is a child assertion",
            test_name="test_daily_revenue_chain",
            expected_models=("stg_orders",),
            assertion_names=("daily_revenue_is_non_negative_for_every_completed_order_day",),
            expected_row_count=3,
        ),
        SqlTestStatusAlignmentTestCase(
            description="longest name is the parent test",
            test_name="test_daily_revenue_chain_with_refunds_and_partial_payment_retries",
            expected_models=("stg_orders", "daily_revenue"),
            assertion_names=(),
            expected_row_count=3,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failed_sql_test_when_reporting_build_progress_then_child_statuses_align_with_parent(
    test_case: SqlTestStatusAlignmentTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(
            test_entries=(
                build_sql_test_plan_entry(
                    test_name=test_case.test_name,
                    expected_models=test_case.expected_models,
                    assertion_names=test_case.assertion_names,
                ),
            )
        ),
        use_color=False,
    )

    callbacks.on_node_complete(
        build_sql_test_result(
            test_name=test_case.test_name,
            expected_models=test_case.expected_models,
            assertion_names=test_case.assertion_names,
            outcome=SqlTestOutcome.FAIL,
        )
    )
    columns: tuple[int, ...] = status_columns(stream.getvalue())

    assert len(columns) == test_case.expected_row_count, stream.getvalue()
    assert len(set(columns)) == 1, stream.getvalue()


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestStatusAlignmentTestCase(
            description="longest name is a child assertion",
            test_name="test_daily_revenue_chain",
            expected_models=("stg_orders",),
            assertion_names=("daily_revenue_is_non_negative_for_every_completed_order_day",),
            expected_row_count=4,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_passing_sql_test_when_reporting_host_model_then_test_rows_align_with_model_status(
    test_case: SqlTestStatusAlignmentTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    stream: StringIO = StringIO()
    monkeypatch.setattr("sys.stdout", stream)
    callbacks: BuildProgressCallbacks = BuildProgressCallbacks(
        plan=PlanOutput(
            test_entries=(
                build_sql_test_plan_entry(
                    test_name=test_case.test_name,
                    expected_models=test_case.expected_models,
                    assertion_names=test_case.assertion_names,
                ),
            )
        ),
        use_color=False,
    )

    callbacks.on_node_complete(
        build_sql_test_result(
            test_name=test_case.test_name,
            expected_models=test_case.expected_models,
            assertion_names=test_case.assertion_names,
            outcome=SqlTestOutcome.PASS,
        )
    )
    callbacks.on_node_complete(
        ModelExecutionResult(
            model_name=test_case.expected_models[0],
            status=ExecutionStatus.SUCCESS,
            duration_ms=100,
        )
    )
    columns: tuple[int, ...] = status_columns(stream.getvalue())

    assert len(columns) == test_case.expected_row_count, stream.getvalue()
    assert len(set(columns)) == 1, stream.getvalue()


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestStatusAlignmentTestCase(
            description="short names share one status column",
            test_name="test_daily_revenue_chain",
            expected_models=("stg_orders", "daily_revenue"),
            assertion_names=("revenue_is_non_negative",),
            expected_row_count=4,
        ),
        SqlTestStatusAlignmentTestCase(
            description="longest name is a child assertion",
            test_name="test_daily_revenue_chain",
            expected_models=("stg_orders",),
            assertion_names=("daily_revenue_is_non_negative_for_every_completed_order_day",),
            expected_row_count=3,
        ),
        SqlTestStatusAlignmentTestCase(
            description="longest name is the parent test",
            test_name="test_daily_revenue_chain_with_refunds_and_partial_payment_retries",
            expected_models=("stg_orders", "daily_revenue"),
            assertion_names=(),
            expected_row_count=3,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failed_sql_test_when_reporting_test_command_progress_then_child_statuses_align(
    test_case: SqlTestStatusAlignmentTestCase,
) -> None:
    stream: StringIO = StringIO()
    callbacks: NestedCommandProgressCallbacks = NestedCommandProgressCallbacks(
        total=1,
        label="test",
        stream=stream,
        use_color=False,
        name_width=resolve_test_name_width(
            (
                build_sql_test_plan_entry(
                    test_name=test_case.test_name,
                    expected_models=test_case.expected_models,
                    assertion_names=test_case.assertion_names,
                ),
            )
        ),
    )
    result: SqlTestExecutionResult = build_sql_test_result(
        test_name=test_case.test_name,
        expected_models=test_case.expected_models,
        assertion_names=test_case.assertion_names,
        outcome=SqlTestOutcome.FAIL,
    )

    callbacks.on_item_start(group_name="daily_revenue", item_name=test_case.test_name)
    callbacks.on_item_complete(
        group_name="daily_revenue",
        item_name=test_case.test_name,
        status_text="FAIL",
        child_rows=build_test_expectation_rows(result),
    )
    columns: tuple[int, ...] = status_columns(stream.getvalue())

    assert len(columns) == test_case.expected_row_count, stream.getvalue()
    assert len(set(columns)) == 1, stream.getvalue()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
