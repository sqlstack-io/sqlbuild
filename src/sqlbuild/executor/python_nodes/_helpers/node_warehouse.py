"""The runtime one Python node runs with: on its job's warehouse, or on the build's."""

from __future__ import annotations

from dataclasses import replace

from sqlbuild.executor.python_nodes.models import PythonNodeRuntime
from sqlbuild.executor.scheduling.classes.session_warehouses import (
    SessionWarehouses,
    with_session_warehouse,
)


def node_runtime(runtime: PythonNodeRuntime, name: str) -> PythonNodeRuntime:
    """Put the shared connection on node `name`'s warehouse; code opening its own connections
    from `connection_config` gets that warehouse too."""

    session_warehouses: SessionWarehouses | None = runtime.session_warehouses
    if session_warehouses is None:
        return runtime
    warehouse: str | None = runtime.node_warehouses.get(name)
    session_warehouses.use(connection=runtime.connection, warehouse=warehouse)
    if warehouse is None:
        return runtime
    return replace(
        runtime, connection_config=with_session_warehouse(runtime.connection_config, warehouse)
    )
