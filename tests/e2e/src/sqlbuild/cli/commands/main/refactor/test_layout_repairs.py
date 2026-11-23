"""CLI e2e coverage for `sqb mv` repairing a project that already fails layout rules."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    LayoutRepairDryRunE2ETestCase,
    LayoutRepairE2ETestCase,
    RefusedBaselineE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    MIXED_OWNER,
    MIXED_OWNER_CHILD,
    OVER_FULL_STAGING,
    RESERVED_CTE_FACT_ORDERS,
    SIBLING_OWNERS,
    layout_view,
    project_text,
    sqb,
    write_orders_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LayoutRepairE2ETestCase(
            description="moving a model out of an over-full folder keeps the reduced failure",
            files=OVER_FULL_STAGING,
            arguments=("stg_order_notes", "sql/models/support/"),
            expected_status="applied",
            expected_baseline={"before": 1, "remaining": 1, "resolved": 0},
            expected_blocking=(),
        ),
        LayoutRepairE2ETestCase(
            description="moving two models out repairs the folder",
            files=OVER_FULL_STAGING,
            arguments=(
                "stg_order_notes",
                "sql/models/support/",
                "stg_order_returns",
                "sql/models/support/",
            ),
            expected_status="applied",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 1},
            expected_blocking=(),
        ),
        LayoutRepairE2ETestCase(
            description="moving a model into the over-full folder worsens it",
            files=OVER_FULL_STAGING,
            arguments=("fact_orders", "sql/models/staging/"),
            expected_status="compile_failed",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 0},
            expected_blocking=("XSQBRLAYOUT001 sql/models/staging holds 6 models; the maximum is 3",),
        ),
        LayoutRepairE2ETestCase(
            description="filling another folder past the limit is a new failure",
            files=OVER_FULL_STAGING,
            arguments=(
                "stg_order_notes",
                "sql/models/marts/",
                "stg_order_returns",
                "sql/models/marts/",
            ),
            expected_status="compile_failed",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 1},
            expected_blocking=("XSQBRLAYOUT001 sql/models/marts holds 4 models; the maximum is 3",),
        ),
        LayoutRepairE2ETestCase(
            description="moving every direct model of a mixed owner into a child repairs it",
            files=MIXED_OWNER,
            arguments=(
                "orders__stg__lines",
                MIXED_OWNER_CHILD,
                "orders__stg__refunds",
                MIXED_OWNER_CHILD,
            ),
            expected_status="applied",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 1},
            expected_blocking=(),
        ),
        LayoutRepairE2ETestCase(
            description="a count that grows at a file nothing moves into blocks",
            files=SIBLING_OWNERS,
            arguments=("orders__stg__c", "sql/models/orders/staging/fin_c/"),
            expected_status="compile_failed",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 0},
            expected_blocking=("SQBRPROJECT204 3 sibling owner directories",),
        ),
        LayoutRepairE2ETestCase(
            description="a rename that adds another error is blocked despite the baseline",
            files=OVER_FULL_STAGING,
            command="rename",
            arguments=("stg_order_notes", "__expected__notes"),
            expected_status="compile_failed",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 0},
            expected_blocking=("D016",),
        ),
        LayoutRepairE2ETestCase(
            description="moving only one direct model of a mixed owner still mixes it",
            files=MIXED_OWNER,
            arguments=("orders__stg__lines", MIXED_OWNER_CHILD),
            expected_status="compile_failed",
            expected_baseline={"before": 1, "remaining": 0, "resolved": 1},
            expected_blocking=("SQBRPROJECT202",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_rule_failures_when_moving_then_only_new_or_worse_failures_block(
    tmp_path: Path, test_case: LayoutRepairE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, test_case.command, *test_case.arguments, "--json"
    )
    payload: dict[str, Any] = json.loads(moved.stdout)

    baseline: dict[str, list[dict[str, Any]]] = payload["rule_baseline"]
    blocking: tuple[str, ...] = tuple(
        f"{error['code']} {error['message']}".strip() for error in payload["compile"]["errors"]
    )
    assert payload["status"] == test_case.expected_status, moved.stdout + moved.stderr
    assert {key: len(items) for key, items in baseline.items()} == test_case.expected_baseline
    assert len(blocking) == len(test_case.expected_blocking), blocking
    assert all(
        found.startswith(expected)
        for found, expected in zip(blocking, test_case.expected_blocking, strict=True)
    ), blocking
    assert moved.returncode == int(test_case.expected_status != "applied")
    assert (project_text(project_dir) == before) == (test_case.expected_status != "applied")


@pytest.mark.parametrize(
    "test_case",
    [
        LayoutRepairDryRunE2ETestCase(
            description="an over-full folder losing a model",
            files=OVER_FULL_STAGING,
            arguments=("stg_order_notes", "sql/models/support/"),
            expected_lines=(
                "Existing rule failures",
                "The project had 1 rule failure before this change; 0 resolved, 1 still failing.",
                "└── XSQBRLAYOUT001  sql/models/staging:1  sql/models/staging holds 4 models; "
                "the maximum is 3",
                "Existing rule failures do not block the change; it adds or worsens none.",
                "Dry run: compiles, 1 file would change; nothing written",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_rule_failures_when_dry_run_then_baseline_and_result_are_explained(
    tmp_path: Path, test_case: LayoutRepairDryRunE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", *test_case.arguments, "--dry-run"
    )

    assert moved.returncode == 0, moved.stdout + moved.stderr
    assert "Compiled project with 1 existing rule failure" in moved.stdout, moved.stdout
    assert set(test_case.expected_lines) <= set(moved.stdout.splitlines()), moved.stdout
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        RefusedBaselineE2ETestCase(
            description="an over-full folder and a reference to a missing model",
            files={
                **OVER_FULL_STAGING,
                "sql/models/marts/order_gaps.sql": layout_view("order_gaps", '__ref("missing_orders")'),
            },
            command=("mv", "stg_order_notes", "sql/models/support/"),
            expected_codes=("P001",),
        ),
        RefusedBaselineE2ETestCase(
            description="an over-full folder and a unit test that cannot be planned",
            files={**OVER_FULL_STAGING, "sql/models/marts/fact_orders.sql": RESERVED_CTE_FACT_ORDERS},
            command=("mv", "customer_totals", "sql/models/support/"),
            expected_codes=("XSQBRLAYOUT001", "P001"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_failure_and_other_error_when_moving_then_project_is_refused(
    tmp_path: Path, test_case: RefusedBaselineE2ETestCase
) -> None:
    """Existing rule failures are tolerated, but any other error still stops the move."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command, "--json")
    payload: dict[str, Any] = json.loads(moved.stdout)

    assert moved.returncode == 1, moved.stdout + moved.stderr
    assert payload["status"] == "project_failed"
    assert tuple(error["code"] for error in payload["compile"]["errors"]) == (
        test_case.expected_codes
    ), payload
    assert project_text(project_dir) == before
