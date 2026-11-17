"""Job results and annotations that tell an unfinished comparison from a cancelled one."""

SUCCESS_RESULT: str = "success"
FAILURE_RESULT: str = "failure"
CANCELLED_RESULT: str = "cancelled"
REGRESSED_OUTPUT: str = "true"
FAILURE_ANNOTATION_LEVEL: str = "failure"
JOB_TIMEOUT_ANNOTATION: str = "has exceeded the maximum execution time"
