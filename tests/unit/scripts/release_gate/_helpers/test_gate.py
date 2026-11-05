import os
import subprocess
from pathlib import Path
from typing import Any

import pytest

from scripts.release_gate._helpers.gate import get_release_gate_errors, verify_check_name
from scripts.release_gate.models import WorkflowEvent
from tests.unit.scripts.release_gate._helpers._test_types import (
    GateViolationTestCase,
    RepositoryGateTestCase,
    VerificationSuitesTestCase,
    VerifyCheckNameTestCase,
)
from tests.unit.scripts.release_gate._helpers.helpers import (
    REPOSITORY,
    REPOSITORY_ROOT,
    read_verify_job,
    write_workflows,
)

RELEASE_AWARE_NAME: str = (
    "${{ (github.event_name == 'pull_request'"
    " && github.event.pull_request.head.repo.full_name == github.repository"
    " && startsWith(github.event.pull_request.head.ref, 'release-please--branches--'))"
    " && 'Release pull request suites' || 'Verify' }}"
)


@pytest.mark.parametrize(
    "test_case",
    [
        RepositoryGateTestCase(
            description="repository workflows gate release pull requests on the release checks",
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repository_workflows_when_checking_release_gate_then_returns_no_errors(
    test_case: RepositoryGateTestCase,
) -> None:
    assert get_release_gate_errors(REPOSITORY_ROOT) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        VerifyCheckNameTestCase(
            description="Release Please pull request reports under another name",
            event_name="pull_request",
            head_ref="release-please--branches--main",
            head_repository=REPOSITORY,
            expected_name="Release pull request suites",
        ),
        VerifyCheckNameTestCase(
            description="feature pull request reports Verify",
            event_name="pull_request",
            head_ref="feature/inventory-sync",
            head_repository=REPOSITORY,
            expected_name="Verify",
        ),
        VerifyCheckNameTestCase(
            description="fork pull request from a release-like branch reports Verify",
            event_name="pull_request",
            head_ref="release-please--branches--main",
            head_repository="contributor/example-project",
            expected_name="Verify",
        ),
        VerifyCheckNameTestCase(
            description="merge group reports Verify",
            event_name="merge_group",
            head_ref=None,
            head_repository=None,
            expected_name="Verify",
        ),
        VerifyCheckNameTestCase(
            description="manual run reports Verify",
            event_name="workflow_dispatch",
            head_ref=None,
            head_repository=None,
            expected_name="Verify",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_event_when_naming_repository_verify_job_then_reports_expected_name(
    test_case: VerifyCheckNameTestCase,
) -> None:
    event: WorkflowEvent = WorkflowEvent(
        event_name=test_case.event_name,
        repository=REPOSITORY,
        head_ref=test_case.head_ref,
        head_repository=test_case.head_repository,
    )

    assert verify_check_name(job=read_verify_job(), event=event) == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    [
        GateViolationTestCase(
            description="Verify job reports Verify for release pull requests",
            verify_job={"name": "Verify", "if": "always()"},
            expected_errors=(
                "A release pull request before the trusted release checks report can merge "
                "under .github/workflows/verify.yml.",
                "A release pull request pushed to after its trusted release checks passed can "
                "merge under .github/workflows/verify.yml.",
            ),
        ),
        GateViolationTestCase(
            description="Verify job renamed for every pull request",
            verify_job={
                "name": "${{ github.event_name == 'pull_request' && 'Suites' || 'Verify' }}",
                "if": "always()",
            },
            expected_errors=(
                "A feature pull request whose suites passed stays blocked under "
                ".github/workflows/verify.yml.",
                "A fork pull request from a release-like branch whose suites passed stays "
                "blocked under .github/workflows/verify.yml.",
            ),
        ),
        GateViolationTestCase(
            description="Verify job can be skipped",
            verify_job={
                "name": RELEASE_AWARE_NAME,
                "if": "needs.changes.outputs.checks == 'true'",
            },
            expected_errors=(
                ".github/workflows/verify.yml job verify must run with `if: always()`: a skipped "
                "job satisfies a required check.",
            ),
        ),
        GateViolationTestCase(
            description="release checks never post Verify on the refreshed head",
            verify_job={"name": RELEASE_AWARE_NAME, "if": "always()"},
            release_checks_job={"steps": [{"uses": "actions/checkout@v4"}]},
            expected_errors=(
                ".github/workflows/release-please.yml job release-checks must need "
                "release-please, so it checks the head after the lockfile refresh.",
                ".github/workflows/release-please.yml job release-checks must check out the "
                "release branch, so it posts Verify on the branch's current head.",
                ".github/workflows/release-please.yml job release-checks never posts a "
                "successful Verify status, so release pull requests could never merge.",
            ),
        ),
        GateViolationTestCase(
            description="release-aware Verify job with valid release checks",
            verify_job={"name": RELEASE_AWARE_NAME, "if": "always()"},
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_workflows_when_checking_release_gate_then_reports_violations(
    tmp_path: Path, test_case: GateViolationTestCase
) -> None:
    write_workflows(
        root=tmp_path,
        verify_job=test_case.verify_job,
        release_checks_job=test_case.release_checks_job,
    )

    assert get_release_gate_errors(tmp_path) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        VerificationSuitesTestCase("both selected suites pass", "true", "success", "success", 0),
        VerificationSuitesTestCase(
            "native failure cannot be masked by static success", "true", "success", "failure", 1
        ),
        VerificationSuitesTestCase(
            "selected native skip fails closed", "true", "success", "skipped", 1
        ),
        VerificationSuitesTestCase(
            "native cancellation fails closed", "true", "success", "cancelled", 1
        ),
        VerificationSuitesTestCase(
            "static failure cannot be masked by native success", "true", "failure", "success", 1
        ),
        VerificationSuitesTestCase(
            "docs-only skips terminate successfully", "false", "skipped", "skipped", 0
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_suites_when_aggregating_then_failures_and_skips_resolve(
    test_case: VerificationSuitesTestCase,
) -> None:
    job: dict[str, Any] = read_verify_job()
    assert "native_checks" in job["needs"]
    step: dict[str, Any] = job["steps"][0]
    environment: dict[str, str] = {name: "success" for name in step["env"]}
    environment.update(
        CHECKS_SELECTED=test_case.selected,
        CHECKS_RESULT=test_case.checks,
        NATIVE_CHECKS_RESULT=test_case.native_checks,
    )
    result: subprocess.CompletedProcess[str] = subprocess.run(
        ["bash", "-e", "-c", step["run"]],
        env=os.environ | environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=5,
    )
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
