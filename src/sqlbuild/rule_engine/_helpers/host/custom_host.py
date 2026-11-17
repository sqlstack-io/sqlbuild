"""Bounded subprocess host for repository-defined Rules."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import time
from dataclasses import asdict
from pathlib import Path, PurePath
from typing import Any

from sqlbuild._native import (
    CUSTOM_HOST_LAUNCH_MODULE,
    CUSTOM_HOST_PROTOCOL_VERSION,
    CUSTOM_HOST_RUNTIME_VERSION,
    CustomHostPartition,
    CustomRuleRun,
    Finding,
    HostCancelledError,
    Rule,
    RulesConfig,
    RulesError,
    build_rule_catalogue,
)
from sqlbuild.rule_engine._helpers.host.custom_evaluation import evaluate_custom_rules
from sqlbuild.rule_engine._helpers.host.host_payload import decode_host_payload
from sqlbuild.rule_engine.classes.runtime_guard import RuntimeGuard
from sqlbuild.rule_engine.constants import (
    CUSTOM_HOST_CANCELLED_EXIT_CODE,
    CUSTOM_HOST_CANCELLED_MESSAGE,
    CUSTOM_HOST_EXEC_OS_NAME,
    CUSTOM_HOST_HASH_SEED,
    CUSTOM_HOST_INPUT_POLL_SECONDS,
    CUSTOM_HOST_INPUT_WAIT_SECONDS,
    CUSTOM_HOST_MAX_TRACKED_READS,
)
from sqlbuild.rule_engine.types import CustomRulePlan, RuleFactProject


def main() -> int:
    """Read one Fensu host request and write exactly one response."""

    if sys.flags.hash_randomization:
        return _write_error(
            f"custom host must be started by {CUSTOM_HOST_LAUNCH_MODULE} "
            f"with PYTHONHASHSEED={CUSTOM_HOST_HASH_SEED}"
        )
    request: object = json.load(sys.stdin)
    if not isinstance(request, dict):
        return _write_error("custom host request must be an object")
    protocol: object = request.get("protocol")
    runtime_version: object = request.get("runtime_version")
    if protocol != CUSTOM_HOST_PROTOCOL_VERSION or runtime_version != CUSTOM_HOST_RUNTIME_VERSION:
        return _write_error("unsupported custom host protocol or runtime")
    guard: RuntimeGuard | None = None
    try:
        payload: object = request["payload"]
        if not isinstance(payload, dict):
            raise RulesError("custom host payload must be an object")
        cancel_marker: Path | None = _cancel_marker(payload)
        if cancel_marker is not None and cancel_marker.exists():
            raise HostCancelledError(CUSTOM_HOST_CANCELLED_MESSAGE)
        project, config = _decode_inputs(payload=payload, cancel_marker=cancel_marker)
        project_dir: Path = Path(str(payload["project_dir"])).resolve()
        dialect: str = str(payload.get("dialect", "generic"))
        verify_determinism: bool = payload.get("verify_determinism") is True
        detect_state: bool = payload.get("detect_state") is True
        track_reads: bool = payload.get("track_reads") is True
        plan: CustomRulePlan = _decode_plan(payload["plan"])
        os.environ.clear()
        os.chdir(project_dir)
        guard = RuntimeGuard(project_dir=project_dir)
        sys.addaudithook(guard)
        guard.guard_filesystem_metadata()
        messages: io.StringIO = io.StringIO()
        with contextlib.redirect_stdout(messages):
            catalogue: tuple[Rule, ...] = build_rule_catalogue(
                config=config, project_dir=project_dir, include_custom=True
            )
            guard.raise_violation()
            by_code: dict[str, Rule] = {rule.code: rule for rule in catalogue}
            selected: tuple[Rule, ...] = tuple(by_code[code] for code in sorted(plan))
            run: CustomRuleRun = evaluate_custom_rules(
                project=project,
                config=config,
                project_dir=project_dir,
                selected_rules=selected,
                dialect=dialect,
                plan=plan,
                track_reads=track_reads,
                verify_determinism=verify_determinism,
                guard=guard,
                partition=(
                    None
                    if cancel_marker is None
                    else CustomHostPartition(
                        cancelled=cancel_marker.exists, detect_state=detect_state
                    )
                ),
            )
    except HostCancelledError:
        sys.stderr.write(CUSTOM_HOST_CANCELLED_MESSAGE)
        return CUSTOM_HOST_CANCELLED_EXIT_CODE
    except Exception as error:
        violation: Exception | None = None if guard is None else guard.violation
        return _write_error(str(violation or error))
    response: dict[str, object] = {
        "protocol": CUSTOM_HOST_PROTOCOL_VERSION,
        "runtime_version": CUSTOM_HOST_RUNTIME_VERSION,
        "error": None,
        "payload": _run_payload(run=run, guard_ns=0 if guard is None else guard.check_ns),
        "messages": messages.getvalue().splitlines(),
    }
    sys.stdout.write(json.dumps(response, sort_keys=True))
    return 0


def _decode_inputs(
    *, payload: dict[str, Any], cancel_marker: Path | None
) -> tuple[RuleFactProject, RulesConfig]:
    project_dir: Path = Path(str(payload["project_dir"])).resolve()
    input_root: Path = (project_dir / "target" / "rules-cache" / "host-inputs").resolve()
    input_path: Path = Path(str(payload["project_payload_path"]))
    if not input_path.is_absolute() or not input_path.parent.resolve().is_relative_to(input_root):
        raise RulesError("custom host project payload path is invalid")
    await_inputs(
        path=input_path,
        cancel_markers=tuple(
            marker
            for marker in (cancel_marker, _input_marker(payload=payload, key="abandoned_marker"))
            if marker is not None
        ),
        parent_pid=_expected_parent(payload),
    )
    if input_path.is_symlink() or not input_path.resolve().is_relative_to(input_root):
        raise RulesError("custom host project payload path is invalid")
    return decode_host_payload(input_path.read_bytes())


def await_inputs(*, path: Path, cancel_markers: tuple[Path, ...], parent_pid: int) -> None:
    """Wait for the payload the parent publishes after starting this host, while it still runs."""

    deadline: float = time.monotonic() + CUSTOM_HOST_INPUT_WAIT_SECONDS
    while not path.exists():
        if any(marker.exists() for marker in cancel_markers):
            raise HostCancelledError(CUSTOM_HOST_CANCELLED_MESSAGE)
        if os.getppid() != parent_pid:
            raise RulesError("custom host parent exited before writing the project payload")
        if time.monotonic() > deadline:
            raise RulesError("custom host project payload was never written")
        time.sleep(CUSTOM_HOST_INPUT_POLL_SECONDS)


def _expected_parent(payload: dict[str, Any]) -> int:
    """Return the parent that started this host; a relaying launcher stands in for it."""

    value: object = payload.get("parent_pid")
    if os.name == CUSTOM_HOST_EXEC_OS_NAME and isinstance(value, int) and value > 0:
        return value
    return os.getppid()


def _input_marker(*, payload: dict[str, Any], key: str) -> Path | None:
    """Return a signal file the run placed beside the project payload, if it named one."""

    value: object = payload.get(key)
    if value is None:
        return None
    project_dir: Path = Path(str(payload["project_dir"])).resolve()
    input_root: Path = (project_dir / "target" / "rules-cache" / "host-inputs").resolve()
    marker: Path = Path(str(value))
    if not marker.is_absolute() or not marker.parent.resolve().is_relative_to(input_root):
        raise RulesError(f"custom host {key} path is invalid")
    return marker


def _cancel_marker(payload: dict[str, Any]) -> Path | None:
    """Return the run's cancellation marker, which must live beside the project payload."""

    return _input_marker(payload=payload, key="cancel_marker")


def _decode_plan(value: object) -> CustomRulePlan:
    if not isinstance(value, dict):
        raise RulesError("custom host plan must be an object")
    plan: CustomRulePlan = {}
    for code, subjects in value.items():
        if subjects is not None and not isinstance(subjects, list):
            raise RulesError("custom host plan subjects must be a list or null")
        plan[str(code)] = (
            None if subjects is None else frozenset(str(subject) for subject in subjects)
        )
    return plan


def _run_payload(*, run: CustomRuleRun, guard_ns: int) -> dict[str, object]:
    """Encode invocations with read sets deduplicated across subjects and bounded in size."""

    readsets: dict[frozenset[tuple[object, ...]], int] = {}
    evaluations: list[dict[str, object]] = []
    untracked: set[str] = set(run.untracked_codes)
    for evaluation in run.evaluations:
        index: int | None = None
        if evaluation.reads is not None and evaluation.code not in untracked:
            index = readsets.setdefault(evaluation.reads, len(readsets))
        evaluations.append(
            {
                "code": evaluation.code,
                "subject": evaluation.subject,
                "findings": [_finding_payload(finding) for finding in evaluation.findings],
                "reads": index,
            }
        )
    bounded: bool = sum(map(len, readsets)) > CUSTOM_HOST_MAX_TRACKED_READS
    if bounded:
        untracked.update(str(item["code"]) for item in evaluations if item["reads"] is not None)
        readsets = {}
        for item in evaluations:
            item["reads"] = None
    untracked_reads: dict[str, set[tuple[object, ...]]] = {code: set() for code in untracked}
    for evaluation in run.evaluations:
        if evaluation.code in untracked_reads and evaluation.reads is not None:
            untracked_reads[evaluation.code].update(evaluation.reads)
    return {
        "evaluations": evaluations,
        "readsets": [sorted(map(fact_key_payload, readset)) for readset in readsets],
        "untracked": {
            code: sorted(map(fact_key_payload, reads)) for code, reads in untracked_reads.items()
        },
        "uncacheable": sorted(run.uncacheable_codes),
        "observed": sorted([list(key), digest] for key, digest in run.observed),
        "bounded": bounded,
        "stateful": sorted(run.stateful_codes),
        "timings": {
            "rules": {code: list(timing) for code, timing in sorted(run.rule_timings.items())},
            "monitor_ns": run.monitor_ns,
            "guard_ns": guard_ns,
        },
    }


def _finding_payload(finding: Finding) -> dict[str, object]:
    payload: dict[str, object] = asdict(finding)
    payload.pop("affected_rules", None)
    payload.pop("fixable", None)
    payload["path"] = finding.path.as_posix()
    return payload


def _write_error(message: str) -> int:
    response: dict[str, object] = {
        "protocol": CUSTOM_HOST_PROTOCOL_VERSION,
        "runtime_version": CUSTOM_HOST_RUNTIME_VERSION,
        "error": message or "custom host failed",
        "payload": None,
        "messages": [],
    }
    sys.stdout.write(json.dumps(response, sort_keys=True))
    return 0


def fact_key_payload(key: tuple[object, ...]) -> list[str]:
    """Return the JSON form of one recorded fact key."""

    return [value.as_posix() if isinstance(value, PurePath) else str(value) for value in key]


if __name__ == "__main__":
    raise SystemExit(main())
