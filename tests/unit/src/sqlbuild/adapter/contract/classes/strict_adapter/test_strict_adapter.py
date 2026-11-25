from __future__ import annotations

import inspect
from collections import defaultdict
from pathlib import Path
from typing import Any, cast

import pytest

from sqlbuild._native import (
    ChainStep,
    CompiledObjectKey,
    CompiledResourceType,
    PlanOutput,
    SqlTestPlanEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import FunctionDefinition, TemporaryFunction
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.pipeline._helpers.testing import run_test_pipeline
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.observability import EventDispatcher, LifecycleEvent, dispatcher_scope
from tests.unit.src.sqlbuild.adapter.contract.classes.strict_adapter._test_types import (
    FirstClassAdapterImplementationContractTestCase,
    OptionalStrictMethodTestCase,
    StrictAdapterContractTestCase,
    StrictAdapterInlineSqlTestTestCase,
    TableFreshnessMetadataUnsupportedTestCase,
)
from tests.unit.src.sqlbuild.adapter.contract.classes.strict_adapter.helpers import (
    delegating_strict_adapter_class,
    first_class_adapter_contract_violations,
)


@pytest.mark.parametrize(
    "test_case",
    [
        StrictAdapterContractTestCase(
            description="strict adapter covers every public base adapter method",
            expected_missing_methods=frozenset(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_base_adapter_public_methods_when_checking_strict_contract_then_all_are_abstract(
    test_case: StrictAdapterContractTestCase,
) -> None:
    attributes_by_function_status: defaultdict[bool, list[str]] = defaultdict(list)
    for name, value in BaseAdapter.__dict__.items():
        attributes_by_function_status[inspect.isfunction(value)].append(name)
    methods_by_private_status: defaultdict[bool, set[str]] = defaultdict(set)
    for name in attributes_by_function_status[True]:
        methods_by_private_status[name.startswith("_")].add(name)
    base_public_methods: frozenset[str] = frozenset(methods_by_private_status[False])

    abstract_methods: frozenset[str] = cast(Any, StrictAdapter).__abstractmethods__
    missing_methods: frozenset[str] = base_public_methods.difference(abstract_methods)

    assert missing_methods == test_case.expected_missing_methods


@pytest.mark.parametrize(
    "test_case",
    [
        FirstClassAdapterImplementationContractTestCase(
            description="first-class adapters implement strict contract below base adapter",
            expected_violations=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_first_class_adapters_when_checking_contract_then_no_method_comes_from_base(
    test_case: FirstClassAdapterImplementationContractTestCase,
) -> None:
    violations: tuple[str, ...] = first_class_adapter_contract_violations()

    assert violations == test_case.expected_violations


@pytest.mark.parametrize(
    "test_case",
    [
        OptionalStrictMethodTestCase(
            description="unit-test output storage is optional and defaults to inline",
            expected_optional_methods=frozenset(
                {
                    "create_sql_test_output",
                    "cursor_type_range",
                    "drop_sql_test_output",
                    "dynamic_listing",
                    "render_alter_dynamic",
                    "render_create_dynamic",
                    "render_create_temporary_function",
                    "render_drop_schema",
                    "render_dynamic_operation",
                    "render_track_changes",
                    "session_schema",
                    "session_warehouse",
                    "use_session_warehouse",
                    "current_session_warehouse",
                }
            ),
            expected_output_relation=None,
            expected_temporary_function=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_strict_adapter_when_reading_optional_methods_then_defaults_keep_inline_comparison(
    test_case: OptionalStrictMethodTestCase,
) -> None:
    adapter: StrictAdapter = delegating_strict_adapter_class()()
    abstract_methods: frozenset[str] = cast(Any, StrictAdapter).__abstractmethods__

    relation: str | None = adapter.create_sql_test_output(
        connection=None, sql="SELECT 1", name="orders_output"
    )
    adapter.drop_sql_test_output(connection=None, relation="orders_output")
    temporary: TemporaryFunction | None = adapter.render_create_temporary_function(
        name="is_completed_order",
        definition=FunctionDefinition(
            destination="main.is_completed_order", arguments=(), returns="BOOLEAN", body_sql="TRUE"
        ),
    )

    with pytest.raises(AdapterUserError) as drop_error:
        _ = adapter.render_drop_schema(database=None, schema="review_orders")

    assert test_case.expected_optional_methods.isdisjoint(abstract_methods)
    assert test_case.expected_optional_methods <= frozenset(StrictAdapter.__dict__)
    assert relation is test_case.expected_output_relation
    assert temporary is test_case.expected_temporary_function
    assert drop_error.value.code == "A714"
    assert adapter.session_warehouse(connection=None) is None
    assert adapter.session_schema(connection=None) is None
    with pytest.raises(AdapterUserError) as dynamic_error:
        adapter.render_create_dynamic(
            destination="main.daily_totals",
            sql="SELECT 1",
            target_lag="DOWNSTREAM",
            refresh_mode="incremental",
            warehouse="transform_wh",
        )
    assert dynamic_error.value.code == "A716"


@pytest.mark.parametrize(
    "test_case",
    [
        StrictAdapterInlineSqlTestTestCase(
            description="project strict adapter without output storage compares inline",
            expected_total=1,
            expected_outcome="pass",
            expected_statement_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_strict_adapter_without_optional_methods_when_running_unit_test_then_inline_passes(
    tmp_path: Path, test_case: StrictAdapterInlineSqlTestTestCase
) -> None:
    adapter: StrictAdapter = delegating_strict_adapter_class()()
    test_entry: SqlTestPlanEntry = SqlTestPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SQL_TEST, name="order_totals"),
        name="order_totals",
        chain=(
            ChainStep(
                model_name="orders",
                resolved_sql="SELECT 1 AS order_id, 10 AS total",
                expected_cte_sql="SELECT 1 AS order_id, 10 AS total",
            ),
        ),
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        results: tuple[SqlTestExecutionResult, ...] = run_test_pipeline(
            plan=PlanOutput(test_entries=(test_entry,)),
            connection_config={"database": str(tmp_path / "orders.duckdb")},
            adapter=cast(Any, adapter),
            run_id="strict-run",
        )

    assert len(results) == test_case.expected_total
    assert results[0].outcome == test_case.expected_outcome, results[0].error_message
    assert [event.event_type for event in events].count(
        "statement_started"
    ) == test_case.expected_statement_count


@pytest.mark.parametrize(
    "test_case",
    [
        TableFreshnessMetadataUnsupportedTestCase(
            description="duckdb table freshness metadata is unsupported by default",
            expected_supports_metadata=False,
            expected_error_fragment="does not support table freshness metadata",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unsupported_adapter_when_getting_table_freshness_metadata_then_raises_clear_error(
    test_case: TableFreshnessMetadataUnsupportedTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()

    assert adapter.supports_table_freshness_metadata() is test_case.expected_supports_metadata
    with pytest.raises(AdapterUserError, match=test_case.expected_error_fragment):
        adapter.get_table_freshness_metadata(
            connection=None,
            database=None,
            schema="main",
            name="raw_orders",
        )
