"""Runtime execution-limit constants."""

from sqlbuild.runtime.execution_limits.types import CancellationOutcome

OUTCOME_SEVERITY: dict[CancellationOutcome, int] = {
    CancellationOutcome.NOT_NEEDED: 0,
    CancellationOutcome.CONFIRMED: 1,
    CancellationOutcome.UNKNOWN: 2,
    CancellationOutcome.UNSUPPORTED: 3,
    CancellationOutcome.FAILED: 4,
}
REPORTED_QUERY_ID_LIMIT: int = 5
CANCELLATION_TEXT: dict[CancellationOutcome, str] = {
    CancellationOutcome.CONFIRMED: "confirmed; the running statement stopped",
    CancellationOutcome.UNKNOWN: "unknown",
    CancellationOutcome.FAILED: "failed",
    CancellationOutcome.UNSUPPORTED: "not supported",
    CancellationOutcome.NOT_NEEDED: "not needed; no statement was running",
}
ITEM_TIMEOUT_CODE: str = "C417"
EXPIRY_REPORT_WAIT_SECONDS: float = 30.0
BACKSTOP_MINIMUM_GRACE_SECONDS: float = 30.0
BACKSTOP_GRACE_FRACTION: float = 0.5
WATCHDOG_MINIMUM_POLL_SECONDS: float = 0.02
WATCHDOG_MAXIMUM_POLL_SECONDS: float = 0.5
SERVER_LIMIT_TOLERANCE_SECONDS: float = 0.05
CONNECTION_PROBE_SQL: str = "SELECT 1"
