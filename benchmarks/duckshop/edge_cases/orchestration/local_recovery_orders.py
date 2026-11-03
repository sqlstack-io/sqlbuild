"""Keep a populated concurrency slot until recovered terminal publication commits."""

from sqlbuild.orchestration import job, schedule

annual_orders = job(
    "annual_orders",
    description="Build orders under a shared concurrency limit.",
    select="orders",
    concurrency_groups=("orders_limit",),
    schedule=schedule(cron="0 0 1 1 *", timezone="UTC", starts_at="2000-01-01T00:00:00Z"),
)
