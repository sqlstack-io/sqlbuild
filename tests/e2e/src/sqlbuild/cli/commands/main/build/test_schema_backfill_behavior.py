"""E2E tests for schema/backfill mutation behavior."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SchemaBackfillBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_waffle_shop,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SchemaBackfillBuildE2ETestCase(
            description="add column mutation reports bounded schema backfill",
            mutate_model_file="sql/models/intermediate/order_status_index.sql",
            model_before_text="on_schema_change append_new_columns",
            model_after_text="on_schema_change append_new_columns",
            mutate_schema_file="sql/models/intermediate/order_status_index.sql",
            schema_before_text="  columns (\n    order_id (audits [not_null, unique]),\n  ),",
            schema_after_text=(
                "  columns (\n"
                "    order_id (audits [not_null, unique]),\n"
                "    status_rank (type INTEGER),\n"
                "  ),"
            ),
            command=("plan", "--json", "--select", "order_status_index"),
            expected_exit_code=0,
            expected_plan_exit_code=0,
            expected_reason="schema_changed",
            expected_backfill_action="bounded",
            expected_backfill_duration="7d",
            expected_warning_entries=(),
        ),
        SchemaBackfillBuildE2ETestCase(
            description="add column mutation with on_schema_change fail reports error warning",
            mutate_model_file="sql/models/intermediate/order_status_index.sql",
            model_before_text="on_schema_change append_new_columns",
            model_after_text="on_schema_change fail",
            mutate_schema_file="sql/models/intermediate/order_status_index.sql",
            schema_before_text="  columns (\n    order_id (audits [not_null, unique]),\n  ),",
            schema_after_text=(
                "  columns (\n"
                "    order_id (audits [not_null, unique]),\n"
                "    status_rank (type INTEGER),\n"
                "  ),"
            ),
            command=("plan", "--json", "--select", "order_status_index"),
            expected_exit_code=0,
            expected_plan_exit_code=1,
            expected_reason="schema_changed",
            expected_backfill_action="bounded",
            expected_backfill_duration="7d",
            expected_warning_entries=(
                ("error", "schema change detected and on_schema_change is set to fail"),
            ),
        ),
        SchemaBackfillBuildE2ETestCase(
            description="add column mutation with on_schema_change ignore reports info warning",
            mutate_model_file="sql/models/intermediate/order_status_index.sql",
            model_before_text="on_schema_change append_new_columns",
            model_after_text="on_schema_change ignore",
            mutate_schema_file="sql/models/intermediate/order_status_index.sql",
            schema_before_text="  columns (\n    order_id (audits [not_null, unique]),\n  ),",
            schema_after_text=(
                "  columns (\n"
                "    order_id (audits [not_null, unique]),\n"
                "    status_rank (type INTEGER),\n"
                "  ),"
            ),
            command=("plan", "--json", "--select", "order_status_index"),
            expected_exit_code=0,
            expected_plan_exit_code=0,
            expected_reason="schema_changed",
            expected_backfill_action="bounded",
            expected_backfill_duration="7d",
            expected_warning_entries=(
                ("info", "schema change detected but on_schema_change is set to ignore"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_backfill_mutation_when_planning_then_expected_metadata_is_reported(
    test_case: SchemaBackfillBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    initial_build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial_build_result.returncode == test_case.expected_exit_code, (
        initial_build_result.stdout + initial_build_result.stderr
    )

    model_path: Path = project_dir / test_case.mutate_model_file
    original_model_text: str = model_path.read_text(encoding="utf-8")
    model_path.write_text(
        original_model_text.replace(test_case.model_before_text, test_case.model_after_text),
        encoding="utf-8",
    )

    schema_path: Path = project_dir / test_case.mutate_schema_file
    original_schema_text: str = schema_path.read_text(encoding="utf-8")
    schema_path.write_text(
        original_schema_text.replace(test_case.schema_before_text, test_case.schema_after_text),
        encoding="utf-8",
    )

    try:
        plan_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        assert plan_result.returncode == test_case.expected_plan_exit_code, (
            plan_result.stdout + plan_result.stderr
        )
        payload: dict[str, object] = json.loads(plan_result.stdout)
        model: dict[str, object] = payload["models"][0]
        assert model["reason"] == test_case.expected_reason
        assert model["backfill"]["action"] == test_case.expected_backfill_action
        assert model["backfill"]["duration"] == test_case.expected_backfill_duration
        warnings: list[dict[str, object]] = payload["warnings"]
        expected_warning: tuple[str, str]
        for expected_warning in test_case.expected_warning_entries:
            assert any(
                warning["severity"] == expected_warning[0]
                and warning["message"] == expected_warning[1]
                for warning in warnings
            )
    finally:
        model_path.write_text(original_model_text, encoding="utf-8")
        schema_path.write_text(original_schema_text, encoding="utf-8")
