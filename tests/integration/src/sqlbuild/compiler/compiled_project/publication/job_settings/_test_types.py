"""Cases of an ad-hoc build that names a job, previewed and run through `sqb server`."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class JobPreviewRunTestCase:
    description: str
    command: tuple[str, ...]
    expected_actions: dict[str, tuple[str, str]]
    expected_job_keys: dict[str, Any]
    expected_error_code: str | None
    expected_run_status: str
    expected_amounts: list[tuple[int]]
