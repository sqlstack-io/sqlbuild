from __future__ import annotations

import pytest

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.models import (
    NodeResultEnvelope,
    NodeResultQuery,
    NodeResultRecord,
)
from sqlbuild.executor.node_results.types import NodeResultStatus
from tests.unit.src.sqlbuild.executor.node_results.classes._test_types import (
    MissingNodeResultTestCase,
    NodeResultHistoryTestCase,
    NodeResultLookupTestCase,
)
from tests.unit.src.sqlbuild.executor.node_results.classes.helpers import (
    InMemoryNodeResultStore,
    build_node_result_record,
    envelope_from_record,
)

_CACHED_SUCCESS: NodeResultRecord = build_node_result_record(
    run_id="cached-success",
    status=NodeResultStatus.SUCCESS.value,
    payload={"rows": 3},
    materialized=True,
)
_CACHED_FAILED: NodeResultRecord = build_node_result_record(
    run_id="cached-failed",
    status=NodeResultStatus.FAILED.value,
    error_message="failed",
    materialized=False,
)
_PERSISTED_SUCCESS: NodeResultEnvelope = envelope_from_record(
    record=build_node_result_record(
        run_id="persisted-success",
        status=NodeResultStatus.SUCCESS.value,
        payload={"rows": 2},
        materialized=True,
    )
)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultLookupTestCase(
            description="latest lookup skips failed cached result and returns cached success",
            cached_records=(_CACHED_SUCCESS, _CACHED_FAILED),
            persisted_results=(_PERSISTED_SUCCESS,),
            run_id=None,
            expected_result=envelope_from_record(record=_CACHED_SUCCESS),
            expected_queries=(),
        ),
        NodeResultLookupTestCase(
            description="run id lookup returns matching failed cached result",
            cached_records=(_CACHED_SUCCESS, _CACHED_FAILED),
            persisted_results=(_PERSISTED_SUCCESS,),
            run_id="cached-failed",
            expected_result=envelope_from_record(record=_CACHED_FAILED),
            expected_queries=(),
        ),
        NodeResultLookupTestCase(
            description="latest persisted lookup requests successful result",
            cached_records=(),
            persisted_results=(_PERSISTED_SUCCESS,),
            run_id=None,
            expected_result=_PERSISTED_SUCCESS,
            expected_queries=(
                NodeResultQuery(
                    node_type="asset",
                    node_name="orders",
                    target_database="analytics",
                    target_schema="state",
                    target_name="dev",
                    statuses=(NodeResultStatus.SUCCESS.value,),
                    run_id=None,
                    limit=1,
                ),
            ),
        ),
        NodeResultLookupTestCase(
            description="run id persisted lookup permits every status",
            cached_records=(),
            persisted_results=(_PERSISTED_SUCCESS,),
            run_id="persisted-success",
            expected_result=_PERSISTED_SUCCESS,
            expected_queries=(
                NodeResultQuery(
                    node_type="asset",
                    node_name="orders",
                    target_database="analytics",
                    target_schema="state",
                    target_name="dev",
                    statuses=None,
                    run_id="persisted-success",
                    limit=1,
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cached_and_persisted_results_when_reading_one_then_contract_is_preserved(
    test_case: NodeResultLookupTestCase,
) -> None:
    store: InMemoryNodeResultStore = InMemoryNodeResultStore(
        persisted_results=test_case.persisted_results
    )
    record: NodeResultRecord
    for record in test_case.cached_records:
        store.write(record)

    result: NodeResultEnvelope | object = store.result_of(
        node_type="asset",
        node_name="orders",
        run_id=test_case.run_id,
    )

    assert result == test_case.expected_result
    assert tuple(store.queries) == test_case.expected_queries


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultHistoryTestCase(
            description="sufficient cache returns newest successful history without storage read",
            cached_records=(_CACHED_SUCCESS, _CACHED_FAILED, _CACHED_SUCCESS),
            persisted_results=(_PERSISTED_SUCCESS,),
            limit=2,
            expected_run_ids=("cached-success", "cached-success"),
            expected_queries=(),
        ),
        NodeResultHistoryTestCase(
            description="insufficient cache delegates complete successful history to storage",
            cached_records=(_CACHED_SUCCESS,),
            persisted_results=(_PERSISTED_SUCCESS,),
            limit=2,
            expected_run_ids=("persisted-success",),
            expected_queries=(
                NodeResultQuery(
                    node_type="asset",
                    node_name="orders",
                    target_database="analytics",
                    target_schema="state",
                    target_name="dev",
                    statuses=(NodeResultStatus.SUCCESS.value,),
                    run_id=None,
                    limit=2,
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cached_and_persisted_results_when_reading_history_then_contract_is_preserved(
    test_case: NodeResultHistoryTestCase,
) -> None:
    store: InMemoryNodeResultStore = InMemoryNodeResultStore(
        persisted_results=test_case.persisted_results
    )
    record: NodeResultRecord
    for record in test_case.cached_records:
        store.write(record)

    results: tuple[NodeResultEnvelope, ...] = store.results_of(
        node_type="asset",
        node_name="orders",
        limit=test_case.limit,
    )

    assert tuple(result.run_id for result in results) == test_case.expected_run_ids
    assert tuple(store.queries) == test_case.expected_queries


@pytest.mark.parametrize(
    "test_case",
    [
        MissingNodeResultTestCase(
            description="missing result returns explicit default and otherwise raises",
            default={"fallback": True},
            expected_default={"fallback": True},
            expected_error_fragment="No persisted result found for Python node 'missing'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_result_when_reading_then_default_and_error_contract_are_preserved(
    test_case: MissingNodeResultTestCase,
) -> None:
    store: InMemoryNodeResultStore = InMemoryNodeResultStore()

    assert (
        store.result_of(
            node_type="asset",
            node_name="missing",
            default=test_case.default,
        )
        == test_case.expected_default
    )
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        store.result_of(node_type="asset", node_name="missing")
