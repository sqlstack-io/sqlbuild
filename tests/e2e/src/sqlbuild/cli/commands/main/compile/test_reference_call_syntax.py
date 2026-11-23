"""E2E tests for reference calls that compile could never replace with a relation."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    RejectedReferenceCallTestCase,
    ReplaceableReferenceCallTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    prepare_reference_call_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_CANONICAL_SOURCE: str = '__source("raw_orders")'
_CANONICAL_REF: str = '__ref("stg_orders")'


@pytest.mark.parametrize(
    "test_case",
    [
        ReplaceableReferenceCallTestCase(
            description="builds double quoted references",
            expected_total_cents="2850",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_double_quoted_reference_calls_when_building_then_relations_resolve(
    test_case: ReplaceableReferenceCallTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_reference_call_project(
        tmp_path=tmp_path, staging_from=_CANONICAL_SOURCE, mart_from=_CANONICAL_REF
    )

    built: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=("build",))
    totals: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir,
        command=("query", "SELECT total_cents FROM main.order_totals", "--json"),
    )

    assert built.returncode == 0, built.stdout + built.stderr
    assert totals.returncode == 0, totals.stdout + totals.stderr
    assert test_case.expected_total_cents in totals.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        RejectedReferenceCallTestCase(
            description="rejects unquoted ref name",
            staging_from=_CANONICAL_SOURCE,
            mart_from="__ref(stg_orders)",
            expected_diagnostics=(
                (
                    "P012",
                    "__ref(stg_orders) is not a valid __ref() call",
                    "sql/models/marts/order_totals.sql",
                    4,
                    6,
                    '__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref("stg_orders")',
                ),
            ),
        ),
        RejectedReferenceCallTestCase(
            description="rejects comment inside ref call",
            staging_from=_CANONICAL_SOURCE,
            mart_from="__ref( /* upstream */ 'stg_orders')",
            expected_diagnostics=(
                (
                    "P012",
                    "__ref( /* upstream */ 'stg_orders') is not a valid __ref() call",
                    "sql/models/marts/order_totals.sql",
                    4,
                    6,
                    '__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref("stg_orders")',
                ),
            ),
        ),
        RejectedReferenceCallTestCase(
            description="rejects single quoted source name",
            staging_from="__source('raw_orders')",
            mart_from=_CANONICAL_REF,
            expected_diagnostics=(
                (
                    "P012",
                    "__source('raw_orders') is not a valid __source() call",
                    "sql/models/staging/stg_orders.sql",
                    3,
                    36,
                    '__source() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __source("raw_orders")',
                ),
            ),
        ),
        RejectedReferenceCallTestCase(
            description="rejects invalid calls in two files",
            staging_from="__source('raw_orders')",
            mart_from="__ref(stg_orders)",
            expected_diagnostics=(
                (
                    "P012",
                    "__ref(stg_orders) is not a valid __ref() call",
                    "sql/models/marts/order_totals.sql",
                    4,
                    6,
                    '__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref("stg_orders")',
                ),
                (
                    "P012",
                    "__source('raw_orders') is not a valid __source() call",
                    "sql/models/staging/stg_orders.sql",
                    3,
                    36,
                    '__source() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __source("raw_orders")',
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reference_call_compile_cannot_replace_when_building_then_fails_with_corrected_call(
    test_case: RejectedReferenceCallTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_reference_call_project(
        tmp_path=tmp_path, staging_from=test_case.staging_from, mart_from=test_case.mart_from
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir,
        command=("compile", "--json"),
    )
    built: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=("build",))

    diagnostics: list[dict[str, object]] = cast(
        list[dict[str, object]], json.loads(compiled.stdout)["diagnostics"]
    )
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    assert built.returncode == 1, built.stdout + built.stderr
    assert (
        tuple(
            (
                item["code"],
                item["message"],
                item["path"],
                item["line"],
                item["column"],
                item["help"],
            )
            for item in diagnostics
        )
        == test_case.expected_diagnostics
    )
    assert "error[P012]" in built.stdout + built.stderr
