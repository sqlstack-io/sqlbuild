"""Build connections move to a node's warehouse and back only when the warehouse changes."""

from __future__ import annotations

from typing import Any, cast

import pytest

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scheduling.classes.session_warehouses import SessionWarehouses
from tests.unit.src.sqlbuild.executor.scheduling.classes._test_types import (
    SessionWarehousesTestCase,
    SessionWithoutWarehouseTestCase,
)
from tests.unit.src.sqlbuild.executor.scheduling.classes.helpers import (
    RecordingWarehouseAdapter,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SessionWarehousesTestCase(
            description="nodes on the build's warehouse issue nothing, however it is spelled",
            configured="BUILD_WH",
            session_warehouse=None,
            steps=(("a", None), ("a", "BUILD_WH"), ("a", "build_wh")),
            expected_statements=(),
            expected_session_reads=0,
        ),
        SessionWarehousesTestCase(
            description="a job's nodes switch once, the next job's again, a plain node back",
            configured="BUILD_WH",
            session_warehouse=None,
            steps=(("a", "ORDERS_WH"), ("a", "ORDERS_WH"), ("a", "REFUNDS_WH"), ("a", None)),
            expected_statements=(("a", "ORDERS_WH"), ("a", "REFUNDS_WH"), ("a", "BUILD_WH")),
            expected_session_reads=0,
        ),
        SessionWarehousesTestCase(
            description="each connection keeps its own warehouse",
            configured="BUILD_WH",
            session_warehouse=None,
            steps=(("a", "ORDERS_WH"), ("b", "ORDERS_WH"), ("b", None), ("a", "ORDERS_WH")),
            expected_statements=(
                ("a", "ORDERS_WH"),
                ("b", "ORDERS_WH"),
                ("b", "BUILD_WH"),
                ("a", "BUILD_WH"),
            ),
            expected_session_reads=0,
        ),
        SessionWarehousesTestCase(
            description="restoring returns every connection still on a job's warehouse",
            configured="BUILD_WH",
            session_warehouse=None,
            steps=(("a", "ORDERS_WH"), ("b", "REFUNDS_WH")),
            expected_statements=(
                ("a", "ORDERS_WH"),
                ("b", "REFUNDS_WH"),
                ("a", "BUILD_WH"),
                ("b", "BUILD_WH"),
            ),
            expected_session_reads=0,
        ),
        SessionWarehousesTestCase(
            description="without a configured warehouse the session's is read once, then used",
            configured=None,
            session_warehouse="USER_WH",
            steps=(("a", "ORDERS_WH"), ("b", "ORDERS_WH"), ("a", None)),
            expected_statements=(
                ("a", "ORDERS_WH"),
                ("b", "ORDERS_WH"),
                ("a", "USER_WH"),
                ("b", "USER_WH"),
            ),
            expected_session_reads=1,
        ),
        SessionWarehousesTestCase(
            description="a job on the session's own warehouse issues nothing",
            configured=None,
            session_warehouse="ORDERS_WH",
            steps=(("a", "ORDERS_WH"), ("a", None)),
            expected_statements=(),
            expected_session_reads=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nodes_on_job_warehouses_when_taking_connections_then_switches_only_on_change(
    test_case: SessionWarehousesTestCase,
) -> None:
    adapter: RecordingWarehouseAdapter = RecordingWarehouseAdapter(
        session_warehouse=test_case.session_warehouse
    )
    warehouses: SessionWarehouses = SessionWarehouses(
        adapter=cast(BaseAdapter, adapter), home=test_case.configured
    )
    connections: dict[str, Any] = {"a": object(), "b": object()}
    for name, warehouse in test_case.steps:
        warehouses.use(connection=connections[name], warehouse=warehouse)
    warehouses.restore()
    names: dict[int, str] = {id(connection): name for name, connection in connections.items()}

    assert (
        tuple((names[id(connection)], warehouse) for connection, warehouse in adapter.statements),
        adapter.session_reads,
    ) == (test_case.expected_statements, test_case.expected_session_reads)


@pytest.mark.parametrize(
    "test_case",
    [
        SessionWithoutWarehouseTestCase(
            description="a session without any warehouse cannot be returned to one",
            job_warehouse="ORDERS_WH",
            expected_error="cannot return a connection to the build's warehouse",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_session_without_any_warehouse_when_returning_to_it_then_it_fails(
    test_case: SessionWithoutWarehouseTestCase,
) -> None:
    adapter: RecordingWarehouseAdapter = RecordingWarehouseAdapter(session_warehouse=None)
    warehouses: SessionWarehouses = SessionWarehouses(adapter=cast(BaseAdapter, adapter), home=None)
    connection: object = object()
    warehouses.use(connection=connection, warehouse=test_case.job_warehouse)

    with pytest.raises(ExecutorInputError, match=test_case.expected_error):
        warehouses.use(connection=connection, warehouse=None)
