"""E2E coverage of omitted versus explicit `[rules] select` and the default Rules notice."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import DefaultRuleSelectionCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
)
_HEADER: str = "MODEL (description 'Test model orders.', materialized table);\n"
_NULL_COMPARISON_SQL: str = "WITH o AS (SELECT 1 AS id)\nSELECT o.id FROM o WHERE o.id = NULL\n"
_CLEAN_SQL: str = "SELECT 1 AS id\n"
_NOTICE: str = "Using default rules. To disable configurable checks, add to sqlbuild_project.toml:"
_FINDING: str = "SQLCLEAN001"


@pytest.mark.parametrize(
    "test_case",
    [
        DefaultRuleSelectionCase(
            description="omitted select runs the default Rules and notes how to turn them off",
            rules_section="",
            sql=_NULL_COMPARISON_SQL,
            command=("compile", "--no-cache"),
            expected_exit=1,
            expected_finding=True,
            expected_notices=1,
        ),
        DefaultRuleSelectionCase(
            description="omitted select notes the defaults once when build stops on a finding",
            rules_section="",
            sql=_NULL_COMPARISON_SQL,
            command=("build",),
            expected_exit=1,
            expected_finding=True,
            expected_notices=1,
        ),
        DefaultRuleSelectionCase(
            description="explicit empty select turns off configurable Rules",
            rules_section="\n[rules]\nselect = []\n",
            sql=_NULL_COMPARISON_SQL,
            command=("compile", "--no-cache"),
            expected_exit=0,
            expected_finding=False,
            expected_notices=0,
        ),
        DefaultRuleSelectionCase(
            description="explicit selection reports its finding without the notice",
            rules_section='\n[rules]\nselect = ["SQLCLEAN001"]\n',
            sql=_NULL_COMPARISON_SQL,
            command=("compile", "--no-cache"),
            expected_exit=1,
            expected_finding=True,
            expected_notices=0,
        ),
        DefaultRuleSelectionCase(
            description="omitted select on clean SQL prints no notice",
            rules_section="",
            sql=_CLEAN_SQL,
            command=("compile", "--no-cache"),
            expected_exit=0,
            expected_finding=False,
            expected_notices=0,
        ),
        DefaultRuleSelectionCase(
            description="JSON compile output carries the finding without the notice",
            rules_section="",
            sql=_NULL_COMPARISON_SQL,
            command=("compile", "--no-cache", "--json"),
            expected_exit=1,
            expected_finding=True,
            expected_notices=0,
        ),
        DefaultRuleSelectionCase(
            description="JSON plan error carries the finding without the notice",
            rules_section="",
            sql=_NULL_COMPARISON_SQL,
            command=("plan", "--json"),
            expected_exit=1,
            expected_finding=True,
            expected_notices=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_selection_when_running_commands_then_defaults_and_notice_follow_it(
    test_case: DefaultRuleSelectionCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML + test_case.rules_section,
            "models/orders.sql": _HEADER + test_case.sql,
        },
    )

    ran: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )

    output: str = ran.stdout + ran.stderr
    assert (ran.returncode, _FINDING in output, output.count(_NOTICE)) == (
        test_case.expected_exit,
        test_case.expected_finding,
        test_case.expected_notices,
    ), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
