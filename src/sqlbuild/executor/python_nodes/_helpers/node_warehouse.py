"""The runtime one Python node runs with: on its job's warehouse, or on the build's."""

from __future__ import annotations

from dataclasses import replace

from sqlbuild.executor.python_nodes.models import PythonNodeRuntime
from sqlbuild.executor.scheduling.classes.session_warehouses import (
    SessionWarehouses,
    with_session_warehouse,
)


def node_runtime(*, runtime: PythonNodeRuntime, name: str) -> tuple[PythonNodeRuntime, str | None]:
    """The runtime of node `name` on its warehouse, and why its connection cannot move there."""

    session_warehouses: SessionWarehouses | None = runtime.session_warehouses
    if session_warehouses is None:
        return runtime, None
    warehouse: str | None = runtime.node_warehouses.get(name)
    try:
        session_warehouses.use(connection=runtime.connection, warehouse=warehouse)
    except Exception as error:  # noqa: BLE001 - the node fails with the reason
        return runtime, (
            f"cannot run on warehouse {warehouse}: {error}"
            if warehouse is not None
            else f"cannot return to the build's warehouse: {error}"
        )
    if warehouse is None:
        return runtime, None
    return (
        replace(
            runtime,
            connection_config=with_session_warehouse(
                connection_config=runtime.connection_config, warehouse=warehouse
            ),
        ),
        None,
    )
