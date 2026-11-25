"""A singular audit whose reads the target doesn't have is not run, counted on its own, and the build succeeds."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import NotRunAuditTestCase
from tests.integration.src.sqlbuild.cli.commands.main.helpers import write_not_run_audit_project


@pytest.mark.parametrize(
    "test_case",
    [
        NotRunAuditTestCase(
            description="in a fresh schema the audit is not run, named with its missing read",
            setup=(),
            command=("build", "orders"),
            expected_exit_code=0,
            expected_texts=(
                "NOT RUN",
                "NOT_RUN=1",
                "orders_have_payments  (audit) reads payments, which doesn't exist in the target",
            ),
            expected_absent=("SKIP=1", "FAIL=1"),
        ),
        NotRunAuditTestCase(
            description="when every read exists the audit runs against what is there",
            setup=(("build", "payments"),),
            command=("build", "orders"),
            expected_exit_code=0,
            expected_texts=("PASS=2",),
            expected_absent=("NOT RUN", "NOT_RUN"),
        ),
        NotRunAuditTestCase(
            description="audit: with every read in the target plans that audit and no models",
            setup=(("build", "orders", "payments"),),
            command=("plan", "audit:orders_have_payments"),
            expected_exit_code=0,
            expected_texts=("Plan ready  0 selected, 1 audit", "Audits (1)"),
            expected_absent=("Models (", "not run"),
        ),
        NotRunAuditTestCase(
            description="selecting the audit itself fails planning with S301",
            setup=(),
            command=("build", "audit:orders_have_payments"),
            expected_exit_code=1,
            expected_texts=(
                "S301",
                "audit 'orders_have_payments' reads 'payments'",
            ),
        ),
        NotRunAuditTestCase(
            description="the plan lists the audit as not run, with its missing read",
            setup=(),
            command=("plan", "orders"),
            expected_exit_code=0,
            expected_texts=(
                "1 audit not run",
                "orders_have_payments  reads payments, which doesn't exist in the target",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_singular_audit_reading_an_unbuilt_model_when_building_then_it_is_not_run(
    test_case: NotRunAuditTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_not_run_audit_project(tmp_path)
    for setup in test_case.setup:
        main(["--project-dir", str(tmp_path), "--no-color", *setup])
    capsys.readouterr()

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command])
    captured: CaptureResult[str] = capsys.readouterr()
    output: str = captured.out + captured.err

    assert exit_code == test_case.expected_exit_code, captured.out + captured.err
    assert {text: text in output for text in test_case.expected_texts} == dict.fromkeys(
        test_case.expected_texts, True
    ), output
    assert {text: text in output for text in test_case.expected_absent} == dict.fromkeys(
        test_case.expected_absent, False
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        NotRunAuditTestCase(
            description="the plan JSON lists the audit under audits_not_run, with its missing read",
            setup=(),
            command=("plan", "--json", "orders"),
            expected_exit_code=0,
            expected_texts=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_singular_audit_reading_an_unbuilt_model_when_planning_json_then_it_is_listed(
    test_case: NotRunAuditTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_not_run_audit_project(tmp_path)

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command])
    captured: CaptureResult[str] = capsys.readouterr()
    plan: dict[str, Any] = json.loads(captured.out)

    assert (exit_code, plan["audits_not_run"]) == (
        test_case.expected_exit_code,
        [{"name": "orders_have_payments", "missing": ["payments"]}],
    ), test_case.description
