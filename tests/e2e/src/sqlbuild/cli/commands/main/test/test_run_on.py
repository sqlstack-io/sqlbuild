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
    RunOnPrecedenceE2ETestCase,
    RunOnTextE2ETestCase,
    ScenarioRunOnFlagsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)

_DUCKDB_PROJECT: str = 'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n'
_DUCKDB_TARGET: str = (
    '\n[targets.dev]\nconnection = "main"\n\n[connections.main]\ndatabase = "orders.duckdb"\n'
)
_DUCKDB_FILES: dict[str, str] = {
    "sql/sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Raw orders.\n"
        "    schema: main\n    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: amount\n        type: INTEGER\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Orders with doubled amounts.');\n\n"
        'SELECT order_id, amount * 2 AS doubled FROM __source("raw_orders")\n'
    ),
    "tests/unit/test_orders.sql": (
        "TEST();\n\nWITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id, 5 AS amount),\n"
        "__expected__orders AS (SELECT 1 AS order_id, 10 AS doubled)\n"
        "SELECT 1\n"
    ),
}

_SNOWFLAKE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
        '[defaults]\ndatabase = "ANALYTICS"\n\n'
        '[targets.dev]\nconnection = "main"\nschema = "dev"\n\n'
        '[connections.main]\naccount = "example"\nuser = "example"\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Raw orders.\n"
        "    database: RAW\n    schema: shop\n    table: orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: NUMBER(38,0)\n"
        "      - name: status\n        type: VARCHAR\n"
    ),
    "sql/models/staging/stg_orders.sql": (
        "MODEL (description 'Orders with a status hash.');\n\n"
        "SELECT\n"
        "  order_id,\n"
        "  status,\n"
        "  HASH(status) AS status_hash\n"
        'FROM __source("raw_orders")\n'
    ),
    "sql/models/staging/stg_order_flags.sql": (
        "MODEL (description 'Orders with a hashed status from a macro.');\n\n"
        "SELECT\n"
        "  order_id,\n"
        '  @py.status_hash("status") AS status_hash\n'
        'FROM __source("raw_orders")\n'
    ),
    "sql/models/staging/_sqlbuild/_macros/python/hashing.py": (
        '"""Staging hash macros."""\n\n\n'
        "def status_hash(column: str) -> str:\n"
        '    """Hash a status column."""\n'
        '    return f"HASH({column})"\n'
    ),
    "sql/models/marts/order_status.sql": (
        "MODEL (description 'Order status flags.');\n\n"
        "SELECT\n"
        "  order_id,\n"
        "  IFF(status = 'completed', 1, 0) AS is_completed\n"
        'FROM __source("raw_orders")\n'
    ),
    "tests/unit/test_stg_orders.sql": (
        "TEST();\n\nWITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id, 'completed' AS status),\n"
        "__expected__stg_orders AS (SELECT 1 AS order_id, 'completed' AS status)\n"
        "SELECT 1\n"
    ),
    "tests/unit/test_stg_order_flags.sql": (
        "TEST();\n\nWITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id, 'completed' AS status),\n"
        "__expected__stg_order_flags AS (SELECT 1 AS order_id)\n"
        "SELECT 1\n"
    ),
    "tests/unit/test_order_status.sql": (
        "TEST();\n\nWITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id, 'completed' AS status),\n"
        "__expected__order_status AS (SELECT 1 AS order_id, 1 AS is_completed)\n"
        "SELECT 1\n"
    ),
}

_DURATION: re.Pattern[str] = re.compile(r"  \d+\.\d+s$", re.MULTILINE)


def _summary(project: Path, *command: str) -> dict[str, object]:
    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--sql", "--json", *command), project_dir=project
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return json.loads(result.stdout)["summary"]


def test_given_waffle_shop_when_testing_locally_then_unit_tests_run_in_duckdb_without_building(
    tmp_path: Path,
) -> None:
    project: Path = prepare_waffle_shop(tmp_path)

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--sql", "--run-on", "local", "--json"),
        project_dir=project,
    )

    payload: dict[str, object] = json.loads(result.stdout)
    checks: list[dict[str, object]] = payload["checks"]  # type: ignore[assignment]
    assert {check["run_on"] for check in checks} == {"local"}
    # The table function reads a built table, so its test can't run in an empty DuckDB.
    assert {
        str(check["name"]): (check["status"], check["run_on_reason"])
        for check in checks
        if check["status"] != "pass"
    } == {"returns_customer_orders": ("error", "project function: customer_orders")}
    assert len(checks) > 1
    summary: dict[str, object] = payload["summary"]  # type: ignore[assignment]
    assert (summary["local_count"], summary["warehouse_count"]) == (len(checks), 0)
    assert result.returncode == 1
    assert "Statements:" not in result.stderr
    assert not (project / "waffle_shop.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnPrecedenceE2ETestCase(
            description="DuckDB projects run locally by default",
            settings=None,
            local_settings=None,
            target=None,
            flag=None,
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="project settings choose the warehouse",
            settings="warehouse",
            local_settings=None,
            target=None,
            flag=None,
            expected_counts=(0, 1),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the local file's settings replace the project file's",
            settings="warehouse",
            local_settings="local",
            target=None,
            flag=None,
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the target replaces settings",
            settings="local",
            local_settings="local",
            target="warehouse",
            flag=None,
            expected_counts=(0, 1),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the flag replaces the target",
            settings="warehouse",
            local_settings=None,
            target="warehouse",
            flag="local",
            expected_counts=(1, 0),
        ),
        RunOnPrecedenceE2ETestCase(
            description="the local file's target replaces the project file's",
            settings=None,
            local_settings=None,
            target="warehouse",
            flag=None,
            expected_counts=(1, 0),
            local_target="local",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_run_on_set_in_several_places_when_testing_then_flag_target_settings_win_in_order(
    tmp_path: Path, test_case: RunOnPrecedenceE2ETestCase
) -> None:
    settings: str = (
        f'\n[settings]\ntest_run_on = "{test_case.settings}"\n' if test_case.settings else ""
    )
    target: str = (
        _DUCKDB_TARGET.replace(
            'connection = "main"\n', f'connection = "main"\ntest_run_on = "{test_case.target}"\n'
        )
        if test_case.target
        else _DUCKDB_TARGET
    )
    files: dict[str, str] = {
        **_DUCKDB_FILES,
        "sqlbuild_project.toml": _DUCKDB_PROJECT + settings + target,
    }
    local_file: str = ""
    if test_case.local_settings:
        local_file += f'[settings]\ntest_run_on = "{test_case.local_settings}"\n'
    if test_case.local_target:
        local_file += f'[targets.dev]\ntest_run_on = "{test_case.local_target}"\n'
    if local_file:
        files["sqlbuild_local.toml"] = local_file
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="run_on_precedence", repo_files=files
    )

    summary: dict[str, object] = _summary(
        project, *(("--run-on", test_case.flag) if test_case.flag else ())
    )

    assert (summary["local_count"], summary["warehouse_count"]) == test_case.expected_counts


@pytest.mark.parametrize(
    "test_case",
    (
        RunOnTextE2ETestCase(
            description="local mode lists SQLQuack's refusals at the model and macro call",
            command=("test", "--sql", "--run-on", "local"),
            expected_exit_code=1,
            expected_text=(
                "PASS=1  FAIL=2  TOTAL=3  (3 local, 0 warehouse)\n"
                "\n"
                "Can't run locally (2 tests)\n"
                "\n"
                "error[SQ0008]: HASH gives a different value in DuckDB\n"
                " --> sql/models/staging/stg_order_flags.sql:5:3\n"
                "  |\n"
                "5 |   HASH(status) AS status_hash\n"
                "  |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n"
                "  |\n"
                "  = note: Equal inputs still get equal hashes within one run.\n"
                "  = note: fine when: Joins, grouping, removing duplicates and spotting changed"
                " rows by hash.\n"
                "  = note: wrong when: Storing, comparing or sorting by the hash value itself:"
                " it differs, with no error.\n"
                "  = help: allow it with hash = true under [allow] in a sqlquack.toml in the"
                " project folder;\n"
                "          `sqlquack explain hash` says when it is safe\n"
                "  = expanded from @py.status_hash at"
                " sql/models/staging/stg_order_flags.sql:5:3\n"
                "  = tests: test_stg_order_flags\n"
                "\n"
                "error[SQ0008]: HASH gives a different value in DuckDB\n"
                " --> sql/models/staging/stg_orders.sql:6:3\n"
                "  |\n"
                "6 |   HASH(status) AS status_hash\n"
                "  |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n"
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
        tmp_path=tmp_path, project_name="run_on_snowflake", repo_files=_SNOWFLAKE_FILES
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_text in _DURATION.sub("", result.stdout), result.stdout


def test_given_snowflake_project_when_testing_locally_with_json_then_reports_where_and_why(
    tmp_path: Path,
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="run_on_json", repo_files=_SNOWFLAKE_FILES
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--sql", "--run-on", "local", "--json"),
        project_dir=project,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    checks: dict[str, dict[str, object]] = {
        str(check["name"]): check
        for check in payload["checks"]  # type: ignore[union-attr]
    }
    assert checks["test_order_status"]["run_on"] == "local"
    assert checks["test_order_status"]["run_on_reason"] is None
    assert checks["test_order_status"]["status"] == "pass"
    assert checks["test_stg_orders"]["run_on"] == "local"
    assert (
        checks["test_stg_orders"]["run_on_reason"]
        == "a known difference that isn't allowed: fn:HASH"
    )
    summary: dict[str, object] = payload["summary"]  # type: ignore[assignment]
    assert (summary["local_count"], summary["warehouse_count"]) == (3, 0)
    refusals: list[dict[str, object]] = payload["refusals"]  # type: ignore[assignment]
    assert {
        (refusal["code"], refusal["construct"], refusal["path"], refusal["line"])
        for refusal in refusals
    } == {
        ("SQ0008", "fn:HASH", "sql/models/staging/stg_orders.sql", 6),
        ("SQ0008", "fn:HASH", "sql/models/staging/stg_order_flags.sql", 5),
    }
    assert sorted(str(test) for refusal in refusals for test in refusal["tests"]) == [  # type: ignore[union-attr]
        "test_stg_order_flags",
        "test_stg_orders",
    ]


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
