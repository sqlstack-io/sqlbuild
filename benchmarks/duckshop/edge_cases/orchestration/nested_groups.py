"""A group can contain jobs, but cannot contain another group."""

from sqlbuild.orchestration import job, job_group

orders = job("orders_job", description="Build orders.", select="orders")
daily = job_group("daily_orders", description="Build daily orders.", jobs=[orders])
weekly = job_group("weekly_orders", description="Unsupported nested membership.", jobs=[daily])
