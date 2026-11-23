"""CLI e2e coverage for `sqb rename` and `sqb mv` on projects that do not compile."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    InvalidRequestJsonE2ETestCase,
    ProjectFailureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    project_text,
    sqb,
    write_orders_project,
)

_DUPLICATE_MACROS: dict[str, str] = {
    "sql/macros/python/cents.py": 'def to_cents(expression: str) -> str:\n    return f"({expression}) * 100"\n',
    "sql/macros/python/money.py": 'def to_cents(expression: str) -> str:\n    return f"({expression}) * 100"\n',
}
_UNKNOWN_REFERENCE: dict[str, str] = {
    "sql/models/marts/order_gaps.sql": (
        "MODEL (description 'Test model order_gaps.',\n  materialized view,\n);\n\n"
        'SELECT order_id FROM __ref("missing_orders")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectFailureE2ETestCase(
            description="mv on a project with a duplicate macro name",
            command=("mv", "fact_orders", "sql/models/reporting/"),
            expected_codes=("P001",),
        ),
        ProjectFailureE2ETestCase(
            description="rename on a project with a duplicate macro name",
            command=("rename", "fact_orders", "order_facts"),
            expected_codes=("P001",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_input_stage_failure_when_refactoring_then_compile_error_is_reported_cleanly(
    tmp_path: Path, test_case: ProjectFailureE2ETestCase
) -> None:
    """A project failing before analysis is refused with its error, in text and as JSON."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=_DUPLICATE_MACROS)
    before: dict[str, str] = project_text(project_dir)

    text: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command)
    machine: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command, "--json")
    payload: dict[str, Any] = json.loads(machine.stdout)

    output: str = text.stdout + text.stderr
    assert text.returncode == 1, output
    assert "Traceback" not in output and "native field" not in output, output
    assert "Project does not compile; fix it before" in text.stdout, output
    assert all(f"error[{code}]" in text.stderr for code in test_case.expected_codes), output
    assert machine.returncode == 1, machine.stdout + machine.stderr
    assert (payload["status"], payload["error"]["code"], payload["compile"]["ok"]) == (
        "project_failed",
        "C959",
        False,
    )
    assert tuple(error["code"] for error in payload["compile"]["errors"]) == (
        test_case.expected_codes
    )
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectFailureE2ETestCase(
            description="rename on a project with an unknown reference",
            command=("rename", "fact_orders", "order_facts"),
            expected_codes=("P001",),
        ),
        ProjectFailureE2ETestCase(
            description="mv on a project with an unknown reference",
            command=("mv", "fact_orders", "sql/models/reporting/"),
            expected_codes=("P001",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reference_error_when_refactoring_with_json_then_stdout_is_an_error_document(
    tmp_path: Path, test_case: ProjectFailureE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=_UNKNOWN_REFERENCE)

    machine: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command, "--json")
    payload: dict[str, Any] = json.loads(machine.stdout)

    assert machine.returncode == 1, machine.stdout + machine.stderr
    assert payload["status"] == "project_failed"
    assert payload["error"]["message"].startswith("Project does not compile")
    assert tuple(error["code"] for error in payload["compile"]["errors"]) == (
        test_case.expected_codes
    ), payload
    assert "missing_orders" in payload["compile"]["errors"][0]["message"], payload


@pytest.mark.parametrize(
    "test_case",
    [
        InvalidRequestJsonE2ETestCase(
            description="rename of a model that does not exist",
            command=("rename", "missing_orders", "order_facts"),
            expected_code="C951",
            expected_message="no model named 'missing_orders' in this project",
        ),
        InvalidRequestJsonE2ETestCase(
            description="mv without a destination",
            command=("mv", "fact_orders"),
            expected_code="C954",
            expected_message="'fact_orders' has no DESTINATION",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_request_when_refactoring_with_json_then_stdout_is_an_error_document(
    tmp_path: Path, test_case: InvalidRequestJsonE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path)

    machine: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command, "--json")
    payload: dict[str, Any] = json.loads(machine.stdout)

    assert machine.returncode == 1, machine.stdout + machine.stderr
    assert (payload["error"]["code"], payload["error"]["message"]) == (
        test_case.expected_code,
        test_case.expected_message,
    )
