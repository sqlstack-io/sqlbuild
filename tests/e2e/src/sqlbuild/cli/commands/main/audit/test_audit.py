"""E2E tests for sqb audit command."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.audit._test_types import AuditE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_fragments_in_order,
    prepare_waffle_shop,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        AuditE2ETestCase(
            description="audit runs all audits against built relations and all pass",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=28",
            expected_stdout_fragments=(
                "Execution  sqb audit  (concurrency: 1)",
                "Connecting to duckdb...",
                "\u2713 Warehouse connected  duckdb",
            ),
            expected_ordered_stdout_fragments=(
                "Compiling project...",
                "Compiled project. (<time>)",
                "Execution  sqb audit  (concurrency: 1)",
                "Audit (28 selected, 12 models)",
                "Connecting to duckdb...",
                "\u2713 Warehouse connected  duckdb  (<time>)",
                "customer_status_snapshot",
                "PASS=<n>  WARN=<n>  FAIL=<n>  INSUFFICIENT=<n>  TOTAL=<n>",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_project_when_running_audit_then_all_audits_pass(
    test_case: AuditE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    run_sqb(command=("--no-color", "build"), project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "audit", "--concurrency", "1"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    assert_fragments_in_order(result.stdout, test_case.expected_ordered_stdout_fragments)
    assert "Inspecting warehouse state..." not in result.stdout
    assert "Generated plan." not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        AuditE2ETestCase(
            description="serial and concurrent json equivalence",
            expected_exit_code=0,
            expected_stdout_fragment="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_file_backed_project_when_auditing_serial_and_concurrent_then_json_is_equivalent(
    test_case: AuditE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    run_sqb(command=("--no-color", "build"), project_dir=project_dir)
    serial_path: Path = tmp_path / "audit-serial.json"
    concurrent_path: Path = tmp_path / "audit-concurrent.json"

    serial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "audit",
            "--concurrency",
            "1",
            "--json-output",
            str(serial_path),
        ),
        project_dir=project_dir,
    )
    concurrent_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "audit",
            "--concurrency",
            "2",
            "--json-output",
            str(concurrent_path),
        ),
        project_dir=project_dir,
    )

    assert serial_result.returncode == test_case.expected_exit_code, (
        serial_result.stdout + serial_result.stderr
    )
    assert concurrent_result.returncode == test_case.expected_exit_code, (
        concurrent_result.stdout + concurrent_result.stderr
    )
    serial_payload: dict[str, object] = json.loads(serial_path.read_text(encoding="utf-8"))
    concurrent_payload: dict[str, object] = json.loads(concurrent_path.read_text(encoding="utf-8"))
    assert concurrent_payload["checks"] == serial_payload["checks"]
    assert concurrent_payload["summary"] == serial_payload["summary"]
    assert serial_payload["execution"] == {"configured_concurrency": 1, "worker_count": 1}
    assert concurrent_payload["execution"] == {
        "configured_concurrency": 2,
        "worker_count": 2,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        AuditE2ETestCase(
            description="audit exits nonzero when an audit returns rows",
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
            expected_stdout_fragments=("PASS=27  WARN=0  FAIL=1  INSUFFICIENT=0  TOTAL=28",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_audit_when_running_audit_then_exit_code_is_nonzero(
    test_case: AuditE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    model_path: Path = project_dir / "sql" / "models" / "marts" / "daily_order_partitioned.sql"
    original_model: str = model_path.read_text(encoding="utf-8")
    assert 'expression "waffles_ordered > 0"' in original_model
    model_path.write_text(
        original_model.replace(
            'expression "waffles_ordered > 0"',
            'expression "waffles_ordered < 0", severity "error"',
        ),
        encoding="utf-8",
    )

    run_sqb(command=("--no-color", "build"), project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "audit"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        AuditE2ETestCase(
            description="attached audit reading a sibling model gates its target",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=29",
            expected_stdout_fragments=("stg_orders", "cross_model_consistency"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_attached_audit_reads_sibling_model_when_building_and_auditing_then_it_gates_its_target(
    test_case: AuditE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    model_path: Path = project_dir / "sql" / "models" / "staging" / "stg_orders.sql"
    model_sql: str = model_path.read_text(encoding="utf-8")
    model_path.write_text(
        model_sql.replace(
            "  columns (",
            "  audits [cross_model_consistency (severity error)],\n  columns (",
            1,
        ),
        encoding="utf-8",
    )
    audit_path: Path = (
        project_dir
        / "sql"
        / "models"
        / "staging"
        / "_sqlbuild"
        / "_audits"
        / "generic"
        / "cross_model_consistency.sql"
    )
    audit_path.parent.mkdir(parents=True, exist_ok=True)
    audit_path.write_text(
        'AUDIT (name "cross_model_consistency");\n\n'
        "SELECT stg.order_id\n"
        "FROM @relation stg\n"
        'LEFT JOIN __ref("stg_customers") customers USING (customer_id)\n'
        "WHERE customers.customer_id IS NULL\n",
        encoding="utf-8",
    )

    build_output_path: Path = tmp_path / "build.json"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json-output", str(build_output_path)),
        project_dir=project_dir,
    )
    audit_output_path: Path = tmp_path / "audit.json"
    audit_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "audit", "--json-output", str(audit_output_path)),
        project_dir=project_dir,
    )

    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    build_payload: dict[str, object] = json.loads(build_output_path.read_text(encoding="utf-8"))
    build_checks: list[dict[str, object]] = cast(list[dict[str, object]], build_payload["checks"])
    build_check_names: tuple[object, ...] = tuple(check["name"] for check in build_checks)
    assert build_check_names.count("cross_model_consistency") == 1
    matching_build_check: dict[str, object] = build_checks[
        build_check_names.index("cross_model_consistency")
    ]
    assert matching_build_check == {
        "kind": "audit",
        "name": "cross_model_consistency",
        "audit_definition_name": "cross_model_consistency",
        "check_id": "audit:cross_model_consistency:model:stg_orders",
        "passed": True,
        "status": "pass",
        "severity": "error",
        "row_count": 0,
        "attachment_kind": "model",
        "attached_target_kind": "model",
        "asset_name": "stg_orders",
        "run_scope_phase": "final",
        "reused": False,
        "evaluation_mode": "violations",
    }
    assert audit_result.returncode == test_case.expected_exit_code, (
        audit_result.stdout + audit_result.stderr
    )
    assert test_case.expected_stdout_fragment in audit_result.stdout
    audit_payload: dict[str, object] = json.loads(audit_output_path.read_text(encoding="utf-8"))
    audit_checks: list[dict[str, object]] = cast(list[dict[str, object]], audit_payload["checks"])
    audit_check_names: tuple[object, ...] = tuple(check["name"] for check in audit_checks)
    assert audit_check_names.count("cross_model_consistency") == 1
    matching_audit_check: dict[str, object] = audit_checks[
        audit_check_names.index("cross_model_consistency")
    ]
    assert matching_audit_check == matching_build_check
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in audit_result.stdout
