"""Negative fixture: mutually exclusive build flags must fail before publication."""

from sqlbuild.orchestration import job

orders = job(
    "orders",
    description="Refresh orders.",
    select="orders",
    settings={"load_sources": False, "reload": True},
)
