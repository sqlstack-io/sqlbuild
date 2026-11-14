"""A job asks for more memory than its launcher's max_memory and fewer CPUs than the launcher default."""

from sqlbuild.orchestration import job, resources, schedule

annual_orders = job(
    "annual_orders",
    select="orders",
    description="Build orders.",
    resources=resources(memory="2g", cpus=0.5),
    schedule=schedule(cron="0 0 * * *", timezone="UTC", starts_at="2000-01-01T00:00:00Z"),
)
