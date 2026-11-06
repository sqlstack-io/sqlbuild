"""Development defaults pause code-enabled triggers without changing their definitions."""

from sqlbuild.orchestration import job, schedule

annual = schedule(cron="0 0 1 1 *", timezone="UTC", starts_at="2099-01-01T00:00:00Z")
annual_orders = job(
    "annual_orders", description="Build annual orders.", select="+orders", schedule=annual
)
