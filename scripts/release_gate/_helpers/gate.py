"""Check that only the trusted release checks can satisfy a release pull request's Verify."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

from scripts.release_gate._helpers.expressions import evaluate_template
from scripts.release_gate._helpers.resolution import required_check_passes
from scripts.release_gate.constants import (
    ALWAYS_CONDITION,
    GATE_SCENARIOS,
    PULL_REQUEST_EVENT,
    RELEASE_BRANCH_OUTPUT,
    RELEASE_CHECKS_JOB,
    RELEASE_PLEASE_JOB,
    RELEASE_VERIFY_SUCCESS,
    RELEASE_WORKFLOW,
    REQUIRED_CHECK,
    SCENARIO_HEADS,
    VERIFY_JOB,
    VERIFY_WORKFLOW,
)
from scripts.release_gate.exceptions import ReleaseGateError
from scripts.release_gate.models import CheckRun, GateScenario, WorkflowEvent


def get_release_gate_errors(root: Path) -> tuple[str, ...]:
    """Return every way the workflows in `root` let something other than release checks pass."""
    verify: dict[str, Any] = _job(root=root, workflow=VERIFY_WORKFLOW, job=VERIFY_JOB)
    errors: list[str] = [
        _scenario_error(scenario)
        for scenario in GATE_SCENARIOS
        if _mergeable(job=verify, scenario=scenario) is not scenario.expected_mergeable
    ]
    if verify.get("if") != ALWAYS_CONDITION:
        errors.append(
            f"{VERIFY_WORKFLOW} job {VERIFY_JOB} must run with `if: {ALWAYS_CONDITION}`: "
            "a skipped job satisfies a required check."
        )
    errors.extend(_release_checks_errors(root))
    return tuple(errors)


def verify_check_name(*, job: dict[str, Any], event: WorkflowEvent) -> str:
    """Return the check run name the Verify workflow's aggregate job reports for `event`."""
    pull_request: dict[str, Any] = {
        "head": {"ref": event.head_ref, "repo": {"full_name": event.head_repository}}
    }
    contexts: dict[str, Any] = {
        "github": {
            "event_name": event.event_name,
            "repository": event.repository,
            "event": {
                "pull_request": pull_request,
                "action": event.action,
                "label": {"name": event.label},
            }
            if event.event_name == PULL_REQUEST_EVENT
            else {},
        },
        "needs": {"changes": {"outputs": {"partial": event.partial}}},
    }
    return evaluate_template(template=str(job.get("name", VERIFY_JOB)), contexts=contexts).strip()


def _mergeable(*, job: dict[str, Any], scenario: GateScenario) -> bool:
    name: str = verify_check_name(job=job, event=scenario.event)
    check_runs: tuple[CheckRun, ...] = tuple(
        CheckRun(sha=sha, name=name, conclusion=scenario.verify_conclusion)
        for sha in SCENARIO_HEADS
    )
    return required_check_passes(
        name=REQUIRED_CHECK,
        head_sha=scenario.head_sha,
        check_runs=check_runs,
        statuses=scenario.statuses,
    )


def _scenario_error(scenario: GateScenario) -> str:
    outcome: str = "stays blocked" if scenario.expected_mergeable else "can merge"
    return f"A {scenario.description} {outcome} under {VERIFY_WORKFLOW}."


def _release_checks_errors(root: Path) -> list[str]:
    job: dict[str, Any] = _job(root=root, workflow=RELEASE_WORKFLOW, job=RELEASE_CHECKS_JOB)
    needs: Any = job.get("needs")
    errors: list[str] = []
    if RELEASE_PLEASE_JOB not in (needs if isinstance(needs, list) else [needs]):
        errors.append(
            f"{RELEASE_WORKFLOW} job {RELEASE_CHECKS_JOB} must need {RELEASE_PLEASE_JOB}, "
            "so it checks the head after the lockfile refresh."
        )
    steps: list[dict[str, Any]] = job.get("steps", [])
    if not any(step.get("with", {}).get("ref") == RELEASE_BRANCH_OUTPUT for step in steps):
        errors.append(
            f"{RELEASE_WORKFLOW} job {RELEASE_CHECKS_JOB} must check out the release branch, "
            "so it posts Verify on the branch's current head."
        )
    if not any(RELEASE_VERIFY_SUCCESS in str(step.get("run", "")) for step in steps):
        errors.append(
            f"{RELEASE_WORKFLOW} job {RELEASE_CHECKS_JOB} never posts a successful "
            f"{REQUIRED_CHECK} status, so release pull requests could never merge."
        )
    return errors


def _job(*, root: Path, workflow: str, job: str) -> dict[str, Any]:
    document: Any = yaml.safe_load((root / workflow).read_text(encoding="utf-8"))
    jobs: Any = document.get("jobs", {}) if isinstance(document, dict) else {}
    found: Any = jobs.get(job) if isinstance(jobs, dict) else None
    if not isinstance(found, dict):
        raise ReleaseGateError(f"{workflow} has no job {job}")
    return found
