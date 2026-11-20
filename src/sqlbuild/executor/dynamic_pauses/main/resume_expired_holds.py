"""Resume the dynamic models whose holds expired: a location sync's only warehouse write."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.dynamic_pauses._helpers.resume import resume_expired
from sqlbuild.executor.dynamic_pauses.models import ResumeOutcome
from sqlbuild.executor.dynamic_pauses.types import StateLocation


def resume_expired_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    grace_seconds: float,
    recorded_by: str,
    quiet_token: str | None,
) -> ResumeOutcome:
    """Close expired holds on models no run holds live; a still-quiet pause table is not read."""

    return resume_expired(
        adapter=adapter,
        connection=connection,
        location=location,
        holds_after=(grace_seconds, recorded_by),
        quiet_token=quiet_token,
    )
