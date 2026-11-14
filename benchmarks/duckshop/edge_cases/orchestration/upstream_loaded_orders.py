"""A job that reaches a managed source through `+`, so its run selects the source's loader chain.

`+orders` selects the model, its managed source raw_shipments, the raw_shipments loader and the
intermediate stage_shipments loader it depends on. The run records each loader as the source it
writes, which is the node its load reports under.
"""

from sqlbuild.orchestration import job, schedule

annual_orders = job(
    "annual_orders",
    description="Build orders and everything upstream of it, loaders included.",
    select="+orders",
    schedule=schedule(cron="0 0 * * *", timezone="UTC", starts_at="2000-01-01T00:00:00Z"),
)
