"""Tests for Python task and asset runtime contexts."""

from __future__ import annotations

import logging
from datetime import datetime

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.assets import AssetContext
from sqlbuild.assets import SkipMode as AssetSkipMode
from sqlbuild.assets import SkipMode as TaskSkipMode
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus, SkipMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.models import NodeResultEnvelope
from sqlbuild.executor.python_nodes.models import (
    PythonNodeResult,
    PythonNodeSkipResult,
)
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.refs import model, source
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    PublicSkipModeExportTestCase,
    PythonNodeContextHelperTestCase,
    PythonNodeRunStateTestCase,
    PythonNodeSkipModeInputTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    PythonNodeContextTestAdapter,
    PythonNodeContextTestResultStore,
    assert_base_context_fields,
    build_asset_context,
    build_task_context,
    skipped_upstream_task,
    upstream_task,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeContextHelperTestCase(
            description="task context records helper calls and qualifies names",
            raw_name="scratch_orders",
            database="override_db",
            schema="override_schema",
            expected_qualified_name="override_db.override_schema.scratch_orders",
            expected_execute_result="result:CREATE TABLE scratch_orders AS SELECT 1",
            expected_query_result="result:SELECT * FROM scratch_orders",
            expected_recorded_events=(
                "CREATE TABLE scratch_orders AS SELECT 1",
                "SELECT * FROM scratch_orders",
                "loading scratch orders",
            ),
            expected_logger_name="sqlbuild.task.fetch_orders",
            expected_run_id="test_run",
            expected_target="dev",
            expected_vars={"batch": "hourly"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_context_when_using_helpers_then_records_and_qualifies_names(
    test_case: PythonNodeContextHelperTestCase,
    caplog: pytest.LogCaptureFixture,
) -> None:
    adapter: PythonNodeContextTestAdapter = PythonNodeContextTestAdapter()
    statement_recorder: StatementRecorder = StatementRecorder()
    context: AssetContext = build_task_context(
        adapter=adapter,
        statement_recorder=statement_recorder,
        logger_name=test_case.expected_logger_name,
    )

    with caplog.at_level(logging.INFO, logger=test_case.expected_logger_name):
        execute_result: object = context.execute_sql("CREATE TABLE scratch_orders AS SELECT 1")
        query_result: object = context.query("SELECT * FROM scratch_orders")
        context.log("loading scratch orders")
    qualified_name: str = context.qualify_name(
        name=test_case.raw_name,
        database=test_case.database,
        schema=test_case.schema,
    )
    default_qualified_name: str = context.qualify_name(name=test_case.raw_name)
    already_qualified_name: str = context.qualify_name(name="custom.schema.table")

    assert execute_result == test_case.expected_execute_result
    assert query_result == test_case.expected_query_result
    assert qualified_name == test_case.expected_qualified_name
    assert default_qualified_name == "default_db.default_schema.scratch_orders"
    assert already_qualified_name == "custom.schema.table"
    assert context.logger.name == test_case.expected_logger_name
    assert context.run_id == test_case.expected_run_id
    assert context.target == test_case.expected_target
    assert context.vars == test_case.expected_vars
    assert "loading scratch orders" in caplog.messages
    events: tuple[LifeCycleEvent, ...] = statement_recorder.snapshot()
    assert tuple(event.content for event in events) == test_case.expected_recorded_events
    assert_base_context_fields(context)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeContextHelperTestCase(
            description="asset context returns asset result and skip signal",
            raw_name="published_customers",
            database=None,
            schema=None,
            expected_qualified_name="default_db.default_schema.published_customers",
            expected_execute_result="s3://exports/customers.parquet",
            expected_query_result="No new source rows",
            expected_recorded_events=(),
            expected_logger_name="sqlbuild.asset.export_customers",
            expected_run_id="test_run",
            expected_target="dev",
            expected_vars={"batch": "hourly"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_asset_context_when_building_results_then_returns_result_and_skip_models(
    test_case: PythonNodeContextHelperTestCase,
) -> None:
    adapter: PythonNodeContextTestAdapter = PythonNodeContextTestAdapter()
    context: AssetContext = build_asset_context(
        adapter=adapter,
        statement_recorder=StatementRecorder(),
        logger_name=test_case.expected_logger_name,
    )

    result: PythonNodeResult = context.result(
        payload=test_case.expected_execute_result,
        metadata={"format": "parquet"},
        materialized=False,
    )
    skip_result: PythonNodeSkipResult = context.skip(
        reason=test_case.expected_query_result,
        mode=SkipMode.SOFT,
        metadata={"cursor": "2026-05-30"},
    )

    assert result == PythonNodeResult(
        payload=test_case.expected_execute_result,
        metadata={"format": "parquet"},
        materialized=False,
    )
    assert skip_result == PythonNodeSkipResult(
        reason=test_case.expected_query_result,
        mode=SkipMode.SOFT,
        metadata={"cursor": "2026-05-30"},
    )
    assert context.qualify_name(name=test_case.raw_name) == test_case.expected_qualified_name
    assert context.run_id == test_case.expected_run_id
    assert context.target == test_case.expected_target
    assert context.vars == test_case.expected_vars
    assert_base_context_fields(context)


@pytest.mark.parametrize(
    "test_case",
    [
        PublicSkipModeExportTestCase(
            description="task and asset modules export skip mode",
            expected_task_export=True,
            expected_asset_export=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_public_task_and_asset_modules_when_importing_skip_mode_then_export_same_enum(
    test_case: PublicSkipModeExportTestCase,
) -> None:
    assert test_case.description
    assert (TaskSkipMode is SkipMode) is test_case.expected_task_export
    assert (AssetSkipMode is SkipMode) is test_case.expected_asset_export


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeSkipModeInputTestCase(
            description="string skip mode normalizes to skip mode enum",
            raw_mode="soft",
            expected_mode=SkipMode.SOFT,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_context_skip_when_mode_is_string_then_normalizes_to_skip_mode(
    test_case: PythonNodeSkipModeInputTestCase,
) -> None:
    context: AssetContext = build_task_context(
        adapter=PythonNodeContextTestAdapter(),
        statement_recorder=StatementRecorder(),
        logger_name="sqlbuild.task.fetch_orders",
    )

    skip_result: PythonNodeSkipResult = context.skip(reason="optional", mode=test_case.raw_mode)

    assert skip_result.mode is test_case.expected_mode


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeContextHelperTestCase(
            description="returns declared SQL relation and rejects undeclared SQL relation",
            raw_name="fact_orders",
            database=None,
            schema=None,
            expected_qualified_name="dev.fact_orders",
            expected_execute_result="raw.orders",
            expected_query_result="ignored",
            expected_recorded_events=(),
            expected_logger_name="sqlbuild.task.profile_orders",
            expected_run_id="test_run",
            expected_target="dev",
            expected_vars={"batch": "hourly"},
            expected_error_fragment="must be declared in depends_on before use",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_context_when_resolving_sql_relations_then_validates_declared_refs(
    test_case: PythonNodeContextHelperTestCase,
) -> None:
    model_ref: SqlResourceRef = model(test_case.raw_name)
    source_ref: SqlResourceRef = source("orders")
    context: AssetContext = AssetContext(
        adapter=PythonNodeContextTestAdapter(),
        connection_config={"warehouse": "dev"},
        connection=object(),
        run_id=test_case.expected_run_id,
        target=test_case.expected_target,
        vars=test_case.expected_vars,
        is_reload=False,
        logger=logging.getLogger(test_case.expected_logger_name),
        statement_recorder=StatementRecorder(),
        default_database="default_db",
        default_schema="default_schema",
        relation_targets={
            model_ref: test_case.expected_qualified_name,
            source_ref: test_case.expected_execute_result,
        },
        allowed_sql_refs=frozenset((model_ref,)),
    )

    assert context.relation(model_ref) == test_case.expected_qualified_name
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        context.relation(source_ref)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeRunStateTestCase(
            description="reads persisted upstream result envelope",
            expected_payload={"file": "orders.json"},
            expected_metadata={"row_count": 3},
            expected_default={"fallback": True},
            expected_error_fragment="No persisted result found for Python node",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_context_with_result_store_when_reading_upstream_outputs_then_returns_values(
    test_case: PythonNodeRunStateTestCase,
) -> None:
    expected_result: NodeResultEnvelope = NodeResultEnvelope(
        node_type=PythonNodeKind.ASSET.value,
        node_name="upstream_task",
        run_id="test_run",
        status=PythonNodeStatus.SUCCESS.value,
        payload=test_case.expected_payload,
        metadata=test_case.expected_metadata,
        error_message=None,
        materialized=None,
        ts=datetime(2026, 1, 1),
    )
    context: AssetContext = build_asset_context(
        adapter=PythonNodeContextTestAdapter(),
        statement_recorder=StatementRecorder(),
        logger_name="sqlbuild.asset.export_customers",
        result_store=PythonNodeContextTestResultStore(
            {(PythonNodeKind.ASSET.value, "upstream_task"): (expected_result,)}
        ),
    )

    result: object = context.result_of(node_function=upstream_task)
    assert result == expected_result
    assert context.result_of(
        node_function=skipped_upstream_task, default=test_case.expected_default
    ) == (test_case.expected_default)
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        context.result_of(node_function=skipped_upstream_task)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeRunStateTestCase(
            description="raises when no result store is available",
            expected_payload=None,
            expected_metadata={},
            expected_default="fallback",
            expected_error_fragment="No Python node result store is available",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_context_without_result_store_when_reading_result_then_raises_or_returns_default(
    test_case: PythonNodeRunStateTestCase,
) -> None:
    context: AssetContext = build_task_context(
        adapter=PythonNodeContextTestAdapter(),
        statement_recorder=StatementRecorder(),
        logger_name="sqlbuild.task.fetch_orders",
    )

    assert context.result_of(node_function=upstream_task, default=test_case.expected_default) == (
        test_case.expected_default
    )
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        context.result_of(node_function=upstream_task)
