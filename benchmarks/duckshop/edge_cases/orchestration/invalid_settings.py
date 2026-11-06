"""Negative fixture: job settings must not override the frozen selection."""

from sqlbuild.orchestration import job

orders = job(
    "orders",
    description="Refresh orders.",
    select="orders",
    settings={"select": "customers"},
)
