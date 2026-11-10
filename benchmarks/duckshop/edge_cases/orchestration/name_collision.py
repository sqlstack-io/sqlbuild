"""A job may not reuse a model's name: jobs share the project's one resource namespace."""

from sqlbuild.orchestration import job

orders = job("orders", description="Build orders.", select="orders")
