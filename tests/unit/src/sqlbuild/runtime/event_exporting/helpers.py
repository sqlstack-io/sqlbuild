"""Shared event export test data."""

from __future__ import annotations

CATALOGUED_EVENT_TYPES: tuple[str, ...] = (
    "invocation_started",
    "invocation_completed",
    "invocation_failed",
    "invocation_phase",
    "run_started",
    "run_completed",
    "run_failed",
    "resource_attempt_started",
    "resource_attempt_completed",
    "resource_attempt_failed",
    "resource_attempt_skipped",
    "operation_started",
    "operation_completed",
    "operation_failed",
    "retry_scheduled",
    "audit_completed",
    "statement_started",
    "statement_submitted",
    "statement_heartbeat",
    "statement_completed",
    "statement_failed",
    "microbatch_started",
    "microbatch_completed",
    "microbatch_failed",
)
