"""Classify a compare job result as finished, cancelled from outside, or unfinished."""

from __future__ import annotations

from scripts.release_performance_outcome.constants import (
    CANCELLED_RESULT,
    FAILURE_RESULT,
    JOB_TIMEOUT_ANNOTATION,
    SUCCESS_RESULT,
)


def unfinished_reason(*, result: str, regressed: bool, annotations: tuple[str, ...]) -> str | None:
    """Return why a non-regressed compare job failed or hit its own timeout, else None."""

    if result == SUCCESS_RESULT or regressed:
        return None
    timed_out: bool = any(JOB_TIMEOUT_ANNOTATION in message for message in annotations)
    if result != FAILURE_RESULT and not (result == CANCELLED_RESULT and timed_out):
        return None
    details: str = "".join(f"\n- {message}" for message in annotations)
    return f"The comparison job ended with result `{result}`.{details}"
