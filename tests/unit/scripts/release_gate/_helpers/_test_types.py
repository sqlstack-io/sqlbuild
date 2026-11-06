from dataclasses import dataclass, field
from typing import Any

from scripts.release_gate.models import CheckRun, CommitStatus

VALID_RELEASE_CHECKS_JOB: dict[str, Any] = {
    "needs": "release-please",
    "steps": [
        {
            "uses": "actions/checkout@v4",
            "with": {"ref": "${{ needs.release-please.outputs.release_branch }}"},
        },
        {"run": 'post_status success Verify "passed"'},
    ],
}


@dataclass(frozen=True)
class RepositoryGateTestCase:
    description: str
    expected_errors: tuple[str, ...]


@dataclass(frozen=True)
class GateViolationTestCase:
    description: str
    verify_job: dict[str, Any]
    expected_errors: tuple[str, ...]
    release_checks_job: dict[str, Any] = field(
        default_factory=lambda: dict(VALID_RELEASE_CHECKS_JOB)
    )


@dataclass(frozen=True)
class VerifyCheckNameTestCase:
    description: str
    event_name: str
    head_ref: str | None
    head_repository: str | None
    expected_name: str


@dataclass(frozen=True)
class RequiredCheckTestCase:
    description: str
    check_runs: tuple[CheckRun, ...]
    statuses: tuple[CommitStatus, ...]
    expected_passes: bool


@dataclass(frozen=True)
class VerificationSuitesTestCase:
    description: str
    selected: str
    checks: str
    native_checks: str
    expected_exit_code: int


@dataclass(frozen=True)
class ExpressionTestCase:
    description: str
    expression: str
    expected_value: Any


@dataclass(frozen=True)
class TemplateTestCase:
    description: str
    template: str
    expected_value: str


@dataclass(frozen=True)
class UnsupportedExpressionTestCase:
    description: str
    expression: str
    expected_message: str
