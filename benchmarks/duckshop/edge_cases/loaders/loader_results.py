"""Two loaders and a task that reads both of their results.

A build that selects only one loader still reads the other loader's latest result, which may come
from an earlier build. On Snowflake the adapter has no default schema, so the results must go to
the target's schema for the task to find them.
"""

from sqlbuild.assets import asset
from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.loaders import loader
from sqlbuild.refs import source


@loader
def raw_order_events(ctx: LoaderContext) -> list[dict[str, object]]:
    """Order events."""
    return [{"event_id": 1}]


@loader
def raw_customer_events(ctx: LoaderContext) -> list[dict[str, object]]:
    """Customer events."""
    return [{"event_id": 1}]


@asset(depends_on=[source("raw_order_events"), source("raw_customer_events")])
def announce_events(ctx):
    """Record the run each loader result came from."""
    return ctx.result(
        payload={
            "orders": ctx.result_of(node_function=raw_order_events).run_id,
            "customers": ctx.result_of(node_function=raw_customer_events).run_id,
        }
    )
