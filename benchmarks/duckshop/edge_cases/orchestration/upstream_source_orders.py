"""Schedule a job whose selection reaches an upstream source that a build never executes."""

from sqlbuild.orchestration import job, schedule

annual_orders = job(
    "annual_orders",
    description="Build orders together with the source it reads.",
    select="+orders",
    schedule=schedule(cron="0 0 * * *", timezone="UTC", starts_at="2000-01-01T00:00:00Z"),
)
