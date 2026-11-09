"""Stable release gate policy constants."""

from typing import Any

from scripts.release_gate.models import CommitStatus, GateScenario, WorkflowEvent
from scripts.release_gate.types import CheckState

VERIFY_WORKFLOW: str = ".github/workflows/verify.yml"
VERIFY_JOB: str = "verify"
RELEASE_WORKFLOW: str = ".github/workflows/release-please.yml"
RELEASE_PLEASE_JOB: str = "release-please"
RELEASE_CHECKS_JOB: str = "release-checks"
RELEASE_BRANCH_OUTPUT: str = "${{ needs.release-please.outputs.release_branch }}"
REQUIRED_CHECK: str = "Verify"
ALWAYS_CONDITION: str = "always()"
RELEASE_VERIFY_SUCCESS: str = "post_status success Verify"
PASSING_STATES: frozenset[CheckState] = frozenset(
    {CheckState.SUCCESS, CheckState.SKIPPED, CheckState.NEUTRAL}
)

PULL_REQUEST_EVENT: str = "pull_request"
EXPRESSION_LITERALS: dict[str, Any] = {"true": True, "false": False, "null": None}
FALSY_VALUES: tuple[Any, ...] = (None, False, 0, "")
STARTS_WITH_FUNCTION: str = "startswith"
STARTS_WITH_ARGUMENTS: int = 2

_REPOSITORY: str = "example-org/example-project"
_RELEASE_BRANCH: str = "release-please--branches--main"
_HEAD: str = "head-1"
_NEXT_HEAD: str = "head-2"
_RELEASE_PULL_REQUEST: WorkflowEvent = WorkflowEvent(
    event_name=PULL_REQUEST_EVENT,
    repository=_REPOSITORY,
    head_ref=_RELEASE_BRANCH,
    head_repository=_REPOSITORY,
)
_FEATURE_PULL_REQUEST: WorkflowEvent = WorkflowEvent(
    event_name=PULL_REQUEST_EVENT,
    repository=_REPOSITORY,
    head_ref="feature/inventory-sync",
    head_repository=_REPOSITORY,
)
_PENDING: CommitStatus = CommitStatus(sha=_HEAD, context=REQUIRED_CHECK, state=CheckState.PENDING)
_SUCCESS: CommitStatus = CommitStatus(sha=_HEAD, context=REQUIRED_CHECK, state=CheckState.SUCCESS)
_FAILURE: CommitStatus = CommitStatus(sha=_HEAD, context=REQUIRED_CHECK, state=CheckState.FAILURE)

GATE_SCENARIOS: tuple[GateScenario, ...] = (
    GateScenario(
        description="release pull request before the trusted release checks report",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="release pull request while the trusted release checks run",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(_PENDING,),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="release pull request after the trusted release checks pass",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(_PENDING, _SUCCESS),
        head_sha=_HEAD,
        expected_mergeable=True,
    ),
    GateScenario(
        description="release pull request whose trusted release checks failed",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(_PENDING, _FAILURE),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="release pull request recovered by re-running the trusted release checks",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(_PENDING, _FAILURE, _PENDING, _SUCCESS),
        head_sha=_HEAD,
        expected_mergeable=True,
    ),
    GateScenario(
        description="release pull request pushed to after its trusted release checks passed",
        event=_RELEASE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(_PENDING, _SUCCESS),
        head_sha=_NEXT_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="feature pull request whose suites passed",
        event=_FEATURE_PULL_REQUEST,
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=True,
    ),
    GateScenario(
        description="feature pull request whose suites failed",
        event=_FEATURE_PULL_REQUEST,
        verify_conclusion=CheckState.FAILURE,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="fork pull request from a release-like branch whose suites passed",
        event=WorkflowEvent(
            event_name=PULL_REQUEST_EVENT,
            repository=_REPOSITORY,
            head_ref=_RELEASE_BRANCH,
            head_repository="contributor/example-project",
        ),
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=True,
    ),
    GateScenario(
        description="feature pull request whose fast core passed without the full matrix it needs",
        event=WorkflowEvent(
            event_name=PULL_REQUEST_EVENT,
            repository=_REPOSITORY,
            head_ref="feature/inventory-sync",
            head_repository=_REPOSITORY,
            partial="true",
        ),
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="feature pull request whose only run followed an unrelated label",
        event=WorkflowEvent(
            event_name=PULL_REQUEST_EVENT,
            repository=_REPOSITORY,
            head_ref="feature/inventory-sync",
            head_repository=_REPOSITORY,
            action="labeled",
            label="documentation",
        ),
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=False,
    ),
    GateScenario(
        description="merge group whose suites passed",
        event=WorkflowEvent(event_name="merge_group", repository=_REPOSITORY),
        verify_conclusion=CheckState.SUCCESS,
        statuses=(),
        head_sha=_HEAD,
        expected_mergeable=True,
    ),
)
SCENARIO_HEADS: tuple[str, ...] = (_HEAD, _NEXT_HEAD)
