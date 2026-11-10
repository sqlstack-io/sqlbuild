"""E2E coverage of `[settings] require_sql_analysis` and its exact-TOML error help."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import RequireSqlAnalysisCase
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    OPT_OUT_HELP,
    OPTIONAL_SQL_ANALYSIS_PROJECT,
    PARSEABLE_OPT_OUT_MODEL,
    PATH_DEFAULT_MODEL,
    REQUIRE_SQL_ANALYSIS_PROJECT,
    SQLSERVER_BRACKETED_CURSOR_MODEL,
    SQLSERVER_EVENTS_SOURCE,
    SQLSERVER_REQUIRE_SQL_ANALYSIS_PROJECT,
    UNPARSEABLE_OPT_OUT_MODEL,
    require_sql_analysis_output,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_COMPILE: tuple[str, ...] = ("--no-color", "compile", "--json", "--no-cache")


@pytest.mark.parametrize(
    "test_case",
    (
        RequireSqlAnalysisCase(
            description="a header opt-out on parseable SQL is an error listing its findings",
            repo_files=(
                ("sqlbuild_project.toml", REQUIRE_SQL_ANALYSIS_PROJECT),
                ("models/order_flags.sql", PARSEABLE_OPT_OUT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=1,
            expected_diagnostics=(("P009", "models/order_flags.sql", 3),),
            expected_text_fragments=(
                "sqlbuild_project.toml sets [settings] require_sql_analysis = true, which only "
                "allows `sql_analysis false` on SQL that SQLBuild cannot parse; this model "
                "parses successfully.",
                "remove `sql_analysis false` and fix the findings it was hiding: "
                "2 type mismatches (run `sqb compile` to see them)",
                OPT_OUT_HELP,
            ),
        ),
        RequireSqlAnalysisCase(
            description="a path_defaults opt-out is reported at its project file entry",
            repo_files=(
                (
                    "sqlbuild_project.toml",
                    REQUIRE_SQL_ANALYSIS_PROJECT
                    + '\n[path_defaults."marts"]\nsql_analysis = false\n',
                ),
                ("models/marts/order_epochs.sql", PATH_DEFAULT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=1,
            expected_diagnostics=(("P009", "sqlbuild_project.toml", 8),),
            expected_text_fragments=(
                "remove `sql_analysis = false` from this [path_defaults] entry and fix the "
                "findings it was hiding: 1 type mismatch",
                OPT_OUT_HELP,
            ),
        ),
        RequireSqlAnalysisCase(
            description="an opt-out on SQL the parser rejects is accepted",
            repo_files=(
                ("sqlbuild_project.toml", REQUIRE_SQL_ANALYSIS_PROJECT),
                ("models/order_lookup.sql", UNPARSEABLE_OPT_OUT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=0,
            expected_diagnostics=(),
            expected_text_fragments=(),
        ),
        RequireSqlAnalysisCase(
            description="without the setting a parseable opt-out keeps today's behaviour",
            repo_files=(
                ("sqlbuild_project.toml", OPTIONAL_SQL_ANALYSIS_PROJECT),
                ("models/order_flags.sql", PARSEABLE_OPT_OUT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=0,
            expected_diagnostics=(),
            expected_text_fragments=(),
        ),
        RequireSqlAnalysisCase(
            description="a T-SQL cursor stub after a bracket name holding -- is analysed, not skipped",
            repo_files=(
                ("sqlbuild_project.toml", SQLSERVER_REQUIRE_SQL_ANALYSIS_PROJECT),
                ("sources/raw.yml", SQLSERVER_EVENTS_SOURCE),
                ("models/bracketed_events.sql", SQLSERVER_BRACKETED_CURSOR_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=0,
            expected_diagnostics=(("L007", "models/bracketed_events.sql", 18),),
            expected_text_fragments=(),
        ),
        RequireSqlAnalysisCase(
            description="a single-run --no-sql-analysis still works",
            repo_files=(
                ("sqlbuild_project.toml", REQUIRE_SQL_ANALYSIS_PROJECT),
                ("models/order_flags.sql", PARSEABLE_OPT_OUT_MODEL),
            ),
            command=("--no-color", "compile", "--json", "--no-cache", "--no-sql-analysis"),
            expected_returncode=0,
            expected_diagnostics=(),
            expected_text_fragments=(),
        ),
        RequireSqlAnalysisCase(
            description="a local project-wide opt-out is rejected",
            repo_files=(
                ("sqlbuild_project.toml", REQUIRE_SQL_ANALYSIS_PROJECT),
                ("sqlbuild_local.toml", "[settings]\nsql_analysis = false\n"),
                ("models/order_epochs.sql", PATH_DEFAULT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=1,
            expected_diagnostics=(("D001", None, None),),
            expected_text_fragments=(
                "error[D001]:",
                "turns SQL analysis off with [settings] sql_analysis = false, which this project "
                "does not allow; sqlbuild_project.toml sets [settings] require_sql_analysis = true",
                "= help: to allow turning SQL analysis off locally, set this in "
                "sqlbuild_project.toml:\n            [settings]\n"
                "            require_sql_analysis = false",
            ),
        ),
        RequireSqlAnalysisCase(
            description="require_sql_analysis cannot be set locally",
            repo_files=(
                ("sqlbuild_project.toml", REQUIRE_SQL_ANALYSIS_PROJECT),
                ("sqlbuild_local.toml", "[settings]\nrequire_sql_analysis = false\n"),
                ("models/order_epochs.sql", PATH_DEFAULT_MODEL),
            ),
            command=_COMPILE,
            expected_returncode=1,
            expected_diagnostics=(("D001", None, None),),
            expected_text_fragments=(
                "cannot set [settings] require_sql_analysis; it is a shared project policy that "
                "only sqlbuild_project.toml can set",
                "= help: remove require_sql_analysis from sqlbuild_local.toml",
            ),
        ),
        RequireSqlAnalysisCase(
            description="an unparseable model without an opt-out shows the exact MODEL header",
            repo_files=(
                ("sqlbuild_project.toml", OPTIONAL_SQL_ANALYSIS_PROJECT),
                (
                    "models/order_lookup.sql",
                    'MODEL (description "Order lookup");\nSELECT order_id FROM orders WHERE\n',
                ),
            ),
            command=_COMPILE,
            expected_returncode=1,
            expected_diagnostics=(("P001", None, None),),
            expected_text_fragments=(
                "error[P001]: SQL syntax error in model 'order_lookup'",
                "= help: if this SQL is valid for your warehouse, skip SQL analysis for this "
                "model, add this to the MODEL header:\n"
                "            MODEL (\n"
                "              sql_analysis false,\n"
                "              ...\n"
                "            );\n"
                "          and please report the SQL so the parser can support it",
                "= help: SQL analysis is on for this project; to turn it off for every model, "
                "set this in sqlbuild_project.toml:\n            [settings]\n"
                "            sql_analysis = false",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_analysis_opt_outs_when_compiling_then_policy_and_help_match(
    test_case: RequireSqlAnalysisCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=dict(test_case.repo_files)
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=test_case.command
    )

    diagnostics, text = require_sql_analysis_output(result)
    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert diagnostics == test_case.expected_diagnostics
    assert all(fragment in text for fragment in test_case.expected_text_fragments), text


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
