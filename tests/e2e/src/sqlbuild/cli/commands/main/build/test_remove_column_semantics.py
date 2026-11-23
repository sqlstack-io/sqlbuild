"""E2E tests for remove-column mutation semantics."""

from __future__ import annotations

import json
import subprocess
from collections import defaultdict
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    RemoveColumnSemanticsBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_waffle_shop, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        RemoveColumnSemanticsBuildE2ETestCase(
            description=(
                "remove column mutation reports query_changed for non-enforced output change"
            ),
            mutate_file="sql/models/intermediate/order_status_index.sql",
            before_text="  ordered_at,\n",
            after_text="",
            command=(
                "plan",
                "--json",
                "--selection-diagnostics",
                "--select",
                "order_status_index",
            ),
            expected_exit_code=0,
            expected_reason="query_changed",
            expected_backfill_action="bounded",
            expected_backfill_duration="7d",
            expected_warning_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_remove_column_mutation_when_planning_then_query_change_semantics_are_reported(
    test_case: RemoveColumnSemanticsBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    initial_build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial_build_result.returncode == test_case.expected_exit_code, (
        initial_build_result.stdout + initial_build_result.stderr
    )

    model_path: Path = project_dir / test_case.mutate_file
    original_text: str = model_path.read_text(encoding="utf-8")
    model_path.write_text(
        original_text.replace(test_case.before_text, test_case.after_text),
        encoding="utf-8",
    )
    try:
        plan_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        assert plan_result.returncode == test_case.expected_exit_code, (
            plan_result.stdout + plan_result.stderr
        )
        payload: dict[str, object] = json.loads(plan_result.stdout)
        model: dict[str, object] = payload["models"][0]
        assert model["reason"] == test_case.expected_reason
        assert model["backfill"]["action"] == test_case.expected_backfill_action
        assert model["backfill"]["duration"] == test_case.expected_backfill_duration
        warnings_by_stale_status: defaultdict[bool, list[dict[str, object]]] = defaultdict(list)
        for warning in payload["warnings"]:
            warnings_by_stale_status[
                str(warning["message"]).startswith("Stale inputs detected")
            ].append(warning)
        non_stale_warnings: list[dict[str, object]] = warnings_by_stale_status[False]
        assert len(non_stale_warnings) == test_case.expected_warning_count
    finally:
        model_path.write_text(original_text, encoding="utf-8")
