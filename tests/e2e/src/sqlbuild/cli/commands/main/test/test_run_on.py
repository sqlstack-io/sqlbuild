"""`sqb test --run-on`: unit tests run locally in DuckDB when SQLQuack converts them."""

import json
import re
from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_scenario_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    RunOnFallbackProgressE2ETestCase,
    RunOnLocalJsonE2ETestCase,
    RunOnPrecedenceE2ETestCase,
    RunOnTextE2ETestCase,
    ScenarioRunOnFlagsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    GUARDED_FILES,
    SNOWFLAKE_FILES,
    UNMOCKED_SOURCE_FILES,
    check_placements,
    refusal_places,
    refused_tests,
    run_on_counts,
    run_on_precedence_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)

_DURATION: re.Pattern[str] = re.compile(r"  \d+\.\d+s$", re.MULTILINE)
_PROJECTS: dict[str, dict[str, str]] = {"snowflake": SNOWFLAKE_FILES}


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnLocalJsonE2ETestCase(
            description="waffle shop runs every test in DuckDB; the table function reads a table",
            project="waffle_shop",
            expected_exit_code=1,
            expected_placements={
                "returns_customer_orders": (
                    "error",
                    "local",
                    "project function: customer_orders",
                ),
                "test_fact_orders": ("pass", "local", None),
                "detects_completed_orders": ("pass", "local", None),
                "calculates_line_total_cents": ("pass", "local", None),
                "test_stg_orders": ("pass", "local", None),
            },
            expected_counts=(5, 0),
            expected_refusal_places=frozenset({("", "customer_orders", None, None)}),
            expected_refused_tests=("returns_customer_orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_testing_locally_then_unit_tests_run_in_duckdb_without_building(
    tmp_path: Path, test_case: RunOnLocalJsonE2ETestCase
) -> None:
    project: Path = prepare_waffle_shop(tmp_path)

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--sql", "--run-on", "local", "--json"),
        project_dir=project,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert check_placements(payload["checks"]) == test_case.expected_placements  # type: ignore[arg-type]
    summary: dict[str, int] = payload["summary"]  # type: ignore[assignment]
    assert (summary["local_count"], summary["warehouse_count"]) == test_case.expected_counts
    assert refused_tests(payload["refusals"]) == list(test_case.expected_refused_tests)  # type: ignore[arg-type]
    assert "Statements:" not in result.stderr
    assert not (project / "waffle_shop.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnPrecedenceE2ETestCase(
            description="DuckDB projects run locally by default",
            settings_toml="",
            target_line="",
            local_toml="",
            run_on_args=(),
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="project settings choose the warehouse",
            settings_toml='\n[settings]\ntest_run_on = "warehouse"\n',
            target_line="",
            local_toml="",
            run_on_args=(),
            expected_counts=(0, 1),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the local file's settings replace the project file's",
            settings_toml='\n[settings]\ntest_run_on = "warehouse"\n',
            target_line="",
            local_toml='[settings]\ntest_run_on = "local"\n',
            run_on_args=(),
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the target replaces settings",
            settings_toml='\n[settings]\ntest_run_on = "local"\n',
            target_line='test_run_on = "warehouse"\n',
            local_toml='[settings]\ntest_run_on = "local"\n',
            run_on_args=(),
            expected_counts=(0, 1),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the flag replaces the target",
            settings_toml='\n[settings]\ntest_run_on = "warehouse"\n',
            target_line='test_run_on = "warehouse"\n',
            local_toml="",
            run_on_args=("--run-on", "local"),
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the local file's target replaces the project file's",
            settings_toml="",
            target_line='test_run_on = "warehouse"\n',
            local_toml='[targets.dev]\ntest_run_on = "local"\n',
            run_on_args=(),
            expected_counts=(1, 0),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_run_on_set_in_several_places_when_testing_then_flag_target_settings_win_in_order(
    tmp_path: Path, test_case: RunOnPrecedenceE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="run_on_precedence",
        repo_files=run_on_precedence_files(test_case),
    )

    counts: tuple[int, int] = run_on_counts(project=project, run_on_args=test_case.run_on_args)

    assert counts == test_case.expected_counts


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnTextE2ETestCase(
            description="local mode lists SQLQuack's refusals at the model and macro call",
            command=("test", "--sql", "--run-on", "local"),
            expected_exit_code=1,
            expected_text=(
                "   = help: allow it for local tests in sqlbuild_project.toml with\n"
                "           `[testing.local] allow = { hash = true }`;"
                " `sqlquack explain hash` says when it is safe\n"
                "\n"
                "   1. @py.status_hash  macro, 1 test \u00b7 1 model\n"
                "       --> sql/models/staging/_sqlbuild/_macros/python/hashing.py:6:14\n"
                "        |\n"
                '      6 |     return f"HASH({column})"\n'
                "        |              ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n"
                "       = used by 1 model:\n"
                "         stg_order_flags (1)\n"
                "\n"
                "   2. stg_orders  1 test\n"
                "       --> sql/models/staging/stg_orders.sql:6:3\n"
            ),
        ),
        RunOnTextE2ETestCase(
            description="inspect names where each test runs and why",
            command=("test", "--sql", "--inspect"),
            expected_exit_code=0,
            expected_text=("test_order_status\n  runs on: local\n"),
        ),
        RunOnTextE2ETestCase(
            description="inspect gives the refusal as the warehouse reason",
            command=("test", "--sql", "--inspect"),
            expected_exit_code=0,
            expected_text=(
                "test_stg_orders\n"
                "  runs on: warehouse  (a known difference that isn't allowed: fn:HASH)\n"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_snowflake_project_when_tests_need_conversion_then_reports_sqlquack_refusals(
    tmp_path: Path, test_case: RunOnTextE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="run_on_snowflake", repo_files=SNOWFLAKE_FILES
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_text in _DURATION.sub("", result.stdout), result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnTextE2ETestCase(
            description="inspect sends a test reading an unmocked declared source to the warehouse",
            command=("test", "--sql", "--inspect", "--select", "test_order_customers_unmocked"),
            expected_exit_code=0,
            expected_text=(
                "test_order_customers_unmocked\n"
                "  runs on: warehouse  (warehouse relation: RAW.SHOP.CUSTOMERS)\n"
            ),
        ),
        RunOnTextE2ETestCase(
            description="--run-on local refuses a test reading an unmocked declared source",
            command=(
                "test",
                "--sql",
                "--run-on",
                "local",
                "--select",
                "test_order_customers_unmocked",
            ),
            expected_exit_code=1,
            expected_text=(
                "   \u2514\u2500\u2500 RAW.SHOP.CUSTOMERS  1 test   test_order_customers_unmocked\n"
                "   = note: the test reads RAW.SHOP.CUSTOMERS, which only the warehouse has\n"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_snowflake_test_reading_an_unmocked_source_when_testing_then_it_needs_the_warehouse(
    tmp_path: Path, test_case: RunOnTextE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="run_on_snowflake_unmocked",
        repo_files={**SNOWFLAKE_FILES, **UNMOCKED_SOURCE_FILES},
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    output: str = _DURATION.sub("", result.stdout + result.stderr)
    assert result.returncode == test_case.expected_exit_code, output
    assert test_case.expected_text in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnTextE2ETestCase(
            description="auto sends a test whose guard raises in DuckDB to the warehouse",
            command=("test", "--sql", "--select", "test_order_statuses"),
            expected_exit_code=1,
            expected_text=(
                "  note: 1 test moved to the warehouse while running: SQLQuack guard 1"
                " (--verbose for why)\n"
                "\n"
                "  Not run (1): needs the warehouse (no connection)\n"
            ),
        ),
        RunOnTextE2ETestCase(
            description="--run-on local makes a test whose guard raises an error naming the guard",
            command=("test", "--sql", "--run-on", "local", "--select", "test_order_statuses"),
            expected_exit_code=1,
            expected_text=(
                "   \u2514\u2500\u2500 array_agg_order  1 test   test_order_statuses\n"
                "   = note: SQLQuack's guard array_agg_order raised in DuckDB: ARRAY_AGG of several"
                " values in no defined order\n"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_snowflake_test_whose_guard_raises_when_testing_then_it_needs_the_warehouse(
    tmp_path: Path, test_case: RunOnTextE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="run_on_snowflake_guarded",
        repo_files={**SNOWFLAKE_FILES, **GUARDED_FILES},
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    output: str = _DURATION.sub("", result.stdout + result.stderr)
    assert result.returncode == test_case.expected_exit_code, output
    assert test_case.expected_text in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnFallbackProgressE2ETestCase(
            description="one test at a time",
            concurrency="1",
            expected_row=(
                "  note: 1 test moved to the warehouse while running: SQLQuack guard 1"
                " (--verbose for why)\n"
            ),
            expected_starts=0,
            expected_assertion_lines=0,
        ),
        RunOnFallbackProgressE2ETestCase(
            description="four tests at a time",
            concurrency="4",
            expected_row=(
                "  note: 1 test moved to the warehouse while running: SQLQuack guard 1"
                " (--verbose for why)\n"
            ),
            expected_starts=0,
            expected_assertion_lines=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_auto_test_that_falls_back_when_testing_then_only_its_warehouse_attempt_shows(
    tmp_path: Path, test_case: RunOnFallbackProgressE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="run_on_snowflake_fallback_progress",
        repo_files={**SNOWFLAKE_FILES, **GUARDED_FILES},
    )

    result: CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "test",
            "--sql",
            "--concurrency",
            test_case.concurrency,
            "--select",
            "test_order_statuses",
        ),
        project_dir=project,
    )

    output: str = _DURATION.sub("", result.stdout + result.stderr)
    assert test_case.expected_row in output, output
    assert output.count("test_order_statuses START") == test_case.expected_starts, output
    assert output.count("SQL test assertion") == test_case.expected_assertion_lines, output


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnLocalJsonE2ETestCase(
            description="Snowflake tests report where they ran, why, and SQLQuack's refusals",
            project="snowflake",
            expected_exit_code=1,
            expected_placements={
                "test_order_status": ("pass", "local", None),
                "test_stg_orders": (
                    "error",
                    "local",
                    "a known difference that isn't allowed: fn:HASH",
                ),
                "test_stg_order_flags": (
                    "error",
                    "local",
                    "a known difference that isn't allowed: fn:HASH",
                ),
            },
            expected_counts=(3, 0),
            expected_refusal_places=frozenset(
                {
                    ("SQ0008", "fn:HASH", "sql/models/staging/stg_orders.sql", 6),
                    ("SQ0008", "fn:HASH", "sql/models/staging/stg_order_flags.sql", 5),
                }
            ),
            expected_refused_tests=("test_stg_order_flags", "test_stg_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_when_testing_locally_with_json_then_reports_where_and_why(
    tmp_path: Path, test_case: RunOnLocalJsonE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="run_on_json", repo_files=_PROJECTS[test_case.project]
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--sql", "--run-on", "local", "--json"),
        project_dir=project,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert check_placements(payload["checks"]) == test_case.expected_placements  # type: ignore[arg-type]
    summary: dict[str, int] = payload["summary"]  # type: ignore[assignment]
    assert (summary["local_count"], summary["warehouse_count"]) == test_case.expected_counts
    assert refusal_places(payload["refusals"]) == test_case.expected_refusal_places  # type: ignore[arg-type]
    assert refused_tests(payload["refusals"]) == list(test_case.expected_refused_tests)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioRunOnFlagsE2ETestCase(
            description="--local is gone",
            command=("scenario", "test", "--local"),
            expected_fragment="unrecognized arguments: --local",
        ),
        ScenarioRunOnFlagsE2ETestCase(
            description="--strict is gone",
            command=("scenario", "test", "--strict"),
            expected_fragment="unrecognized arguments: --strict",
        ),
        ScenarioRunOnFlagsE2ETestCase(
            description="local runs keep no warehouse artifacts to retain",
            command=("scenario", "test", "--run-on", "local", "--retain"),
            expected_fragment="scenario test --run-on local does not support --retain",
        ),
        ScenarioRunOnFlagsE2ETestCase(
            description="snapshot sync needs scenarios that run locally",
            command=("scenario", "test", "--run-on", "warehouse", "--sync-snapshots"),
            expected_fragment="scenario snapshot sync flags need scenarios that run locally",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_run_on_flags_when_combined_then_refuses_unsupported_ones(
    tmp_path: Path, test_case: ScenarioRunOnFlagsE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_run_on",
        repo_files=build_scenario_project_files(),
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    assert result.returncode != 0
    assert test_case.expected_fragment in result.stdout + result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
