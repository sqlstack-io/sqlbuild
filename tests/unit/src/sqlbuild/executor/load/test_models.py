"""Tests for source loader execution models."""

from __future__ import annotations

import logging

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.models import LoaderContext, LoaderRelationRef
from tests.unit.src.sqlbuild.executor.load._test_types import (
    LoaderContextHelperTestCase,
    LoaderReferenceRejectionTestCase,
)
from tests.unit.src.sqlbuild.executor.load.helpers import (
    LoaderContextTestAdapter,
    plain_function,
    score_orders,
    scored_orders_export,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderContextHelperTestCase(
            description="records helper calls and qualifies names using destination namespace",
            raw_name="scratch_orders",
            database="override_db",
            schema="override_schema",
            expected_qualified_name="override_db.override_schema.scratch_orders",
            expected_target_schema_name="target_db.target_schema.scratch_orders",
            expected_execute_result="result:CREATE TABLE scratch_orders AS SELECT 1",
            expected_query_result="result:SELECT * FROM scratch_orders",
            expected_recorded_events=(
                "CREATE TABLE scratch_orders AS SELECT 1",
                "SELECT * FROM scratch_orders",
                "loading scratch orders",
            ),
            expected_logger_name="sqlbuild.loader.raw_orders_loader",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_context_when_using_helpers_then_records_and_qualifies_names(
    test_case: LoaderContextHelperTestCase,
) -> None:
    adapter: LoaderContextTestAdapter = LoaderContextTestAdapter()
    statement_recorder: StatementRecorder = StatementRecorder()
    context: LoaderContext = LoaderContext(
        adapter=adapter,
        connection_config={},
        connection=object(),
        destination="target_db.target_schema.raw_orders",
        destination_database="target_db",
        destination_schema="target_schema",
        destination_name="raw_orders",
        run_id="test_run",
        target="dev",
        vars={},
        is_reload=False,
        use_color=False,
        current_cursor_value=None,
        logger=logging.getLogger(test_case.expected_logger_name),
        statement_recorder=statement_recorder,
    )

    execute_result: object = context.execute_sql("CREATE TABLE scratch_orders AS SELECT 1")
    query_result: object = context.query("SELECT * FROM scratch_orders")
    context.log("loading scratch orders")
    qualified_name: str = context.qualify_name(
        name=test_case.raw_name,
        database=test_case.database,
        schema=test_case.schema,
    )
    destination_schema_name: str = context.qualify_in_destination_schema(test_case.raw_name)
    already_qualified_name: str = context.qualify_name(name="custom.schema.table")

    assert execute_result == test_case.expected_execute_result
    assert query_result == test_case.expected_query_result
    assert qualified_name == test_case.expected_qualified_name
    assert destination_schema_name == test_case.expected_target_schema_name
    assert already_qualified_name == "custom.schema.table"
    assert context.logger.name == test_case.expected_logger_name
    events: tuple[LifeCycleEvent, ...] = statement_recorder.snapshot()
    assert tuple(event.content for event in events) == test_case.expected_recorded_events


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderContextHelperTestCase(
            description="resolves loader and source relation refs",
            raw_name="scratch_orders",
            database=None,
            schema=None,
            expected_qualified_name="target_db.target_schema.scratch_orders",
            expected_target_schema_name="target_db.target_schema.scratch_orders",
            expected_execute_result="max-value",
            expected_query_result="max-value",
            expected_recorded_events=(
                "SELECT MAX(event_at) FROM target_db.target_schema.fetch_events",
                "SELECT MAX(loaded_at) FROM target_db.target_schema.raw_events",
            ),
            expected_logger_name="sqlbuild.loader.raw_orders_loader",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_context_when_resolving_relation_refs_then_returns_cursor_values(
    test_case: LoaderContextHelperTestCase,
) -> None:
    def fetch_events(ctx: object) -> object:
        return ctx

    adapter: LoaderContextTestAdapter = LoaderContextTestAdapter()
    statement_recorder: StatementRecorder = StatementRecorder()
    loader_ref: LoaderRelationRef = LoaderRelationRef(
        name="fetch_events",
        destination="target_db.target_schema.fetch_events",
        database="target_db",
        schema="target_schema",
        table_name="fetch_events",
        cursor_column="event_at",
        adapter=adapter,
        connection=object(),
        statement_recorder=statement_recorder,
    )
    source_ref: LoaderRelationRef = LoaderRelationRef(
        name="raw_events",
        destination="target_db.target_schema.raw_events",
        database="target_db",
        schema="target_schema",
        table_name="raw_events",
        cursor_column="event_at",
        adapter=adapter,
        connection=object(),
        statement_recorder=statement_recorder,
    )
    context: LoaderContext = LoaderContext(
        adapter=adapter,
        connection_config={},
        connection=object(),
        destination="target_db.target_schema.raw_orders",
        destination_database="target_db",
        destination_schema="target_schema",
        destination_name="raw_orders",
        run_id="test_run",
        target="dev",
        vars={},
        is_reload=False,
        use_color=False,
        current_cursor_value=None,
        logger=logging.getLogger(test_case.expected_logger_name),
        statement_recorder=statement_recorder,
        loader_refs={fetch_events: loader_ref},
        source_refs={"raw_events": source_ref},
    )

    assert context.loader(fetch_events).current_cursor_value == test_case.expected_execute_result
    assert context.source("raw_events").max("loaded_at") == test_case.expected_query_result
    events: tuple[LifeCycleEvent, ...] = statement_recorder.snapshot()
    assert tuple(event.content for event in events) == test_case.expected_recorded_events


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderReferenceRejectionTestCase(
            description="a task points to ctx.result_of",
            reference=score_orders,
            expected_message=(
                "ctx.loader() takes a loader, not the task 'score_orders'; read its result with "
                "ctx.result_of(node_function=score_orders) instead"
            ),
        ),
        LoaderReferenceRejectionTestCase(
            description="an asset points to ctx.result_of",
            reference=scored_orders_export,
            expected_message=(
                "ctx.loader() takes a loader, not the asset 'scored_orders_export'; read its "
                "result with ctx.result_of(node_function=scored_orders_export) instead"
            ),
        ),
        LoaderReferenceRejectionTestCase(
            description="an undecorated function stays an unknown loader reference",
            reference=plain_function,
            expected_message="Unknown loader reference passed to ctx.loader(...)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_non_loader_when_passed_to_ctx_loader_then_the_error_names_what_to_use(
    test_case: LoaderReferenceRejectionTestCase,
) -> None:
    context: LoaderContext = LoaderContext(
        adapter=LoaderContextTestAdapter(),
        connection_config={},
        connection=object(),
        destination="target_db.target_schema.partner_scores",
        destination_database="target_db",
        destination_schema="target_schema",
        destination_name="partner_scores",
        run_id="test_run",
        target="dev",
        vars={},
        is_reload=False,
        use_color=False,
        current_cursor_value=None,
        logger=logging.getLogger("sqlbuild.loader.partner_scores"),
        statement_recorder=StatementRecorder(),
    )

    with pytest.raises(ExecutorInputError) as error_info:
        context.loader(test_case.reference)

    assert str(error_info.value) == test_case.expected_message
