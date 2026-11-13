"""A tag-selected job whose model loads a loader chain and a hook-read source.

The build loads raw_orders and the raw_customers loader it depends on, and raw_returns, which only
the model's pre-hook reads. The job's write set claims those three and not raw_inventory.
"""

from sqlbuild.orchestration import job, schedule

annual_orders = job(
    "annual_orders",
    description="Build the marts layer and the sources it loads.",
    select="tag:marts",
    schedule=schedule(cron="0 0 * * *", timezone="UTC", starts_at="2000-01-01T00:00:00Z"),
)
