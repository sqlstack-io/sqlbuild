"""E2E tests for model version identity across SQL hook resource edits."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SqlHookIdentityBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_HOOK_PATH: str = "sql/hooks/sql/record_order_access.sql"
_HOOK_CONTENTS: str = (
    dedent(
        """
        HOOK (description "Record order table access");

        CREATE OR REPLACE TABLE main.order_access AS SELECT @label AS label
        """
    ).strip()
    + "\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlHookIdentityBuildE2ETestCase(
            description="hook description edit keeps the model unchanged",
            removed_hook_path=_HOOK_PATH,
            added_hook_path=_HOOK_PATH,
            added_hook_contents=_HOOK_CONTENTS.replace(
                "Record order table access", "Audit order reads"
            ),
            expected_reason="no_change",
        ),
        SqlHookIdentityBuildE2ETestCase(
            description="hook file move keeps the model unchanged",
            removed_hook_path=_HOOK_PATH,
            added_hook_path="sql/hooks/sql/audit/record_order_access.sql",
            added_hook_contents=_HOOK_CONTENTS,
            expected_reason="no_change",
        ),
        SqlHookIdentityBuildE2ETestCase(
            description="hook SQL edit changes the model",
            removed_hook_path=_HOOK_PATH,
            added_hook_path=_HOOK_PATH,
            added_hook_contents=_HOOK_CONTENTS.replace("@label AS label", "@label AS access"),
            expected_reason="config_changed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_model_with_sql_hook_when_editing_hook_then_plans_expected_reason(
    test_case: SqlHookIdentityBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sql_hook_identity_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "sql_hook_identity_project"
                adapter = "duckdb"

                [connection]
                database = "sql_hook_identity_project.duckdb"
                """
            ).strip()
            + "\n",
            _HOOK_PATH: _HOOK_CONTENTS,
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  post_hooks [sql("record_order_access", label: 1)]
                );

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
        },
    )
    first_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert first_build.returncode == 0, first_build.stdout + first_build.stderr

    (project_dir / test_case.removed_hook_path).unlink()
    added_hook: Path = project_dir / test_case.added_hook_path
    added_hook.parent.mkdir(parents=True, exist_ok=True)
    added_hook.write_text(test_case.added_hook_contents)
    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"),
        project_dir=project_dir,
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    payload: dict[str, object] = json.loads(plan.stdout)
    reasons_by_name: dict[str, str] = {
        str(entry["name"]): str(entry["reason"]) for entry in payload["models"]
    }
    assert reasons_by_name["orders"] == test_case.expected_reason
