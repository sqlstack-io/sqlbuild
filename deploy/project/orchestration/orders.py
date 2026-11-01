"""A small scheduled project baked into the reference location image."""

from sqlbuild.orchestration import job, schedule

orders = job(
    "orders",
    select="orders",
    description="Refresh orders every minute.",
    schedule=schedule(cron="* * * * *", timezone="UTC"),
)
