"""`sqb test --since` and `sqb scenario test --since` select what a git change can affect."""

import json
from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    SinceBrokenBaseE2ETestCase,
    SinceJsonOutputE2ETestCase,
    SinceSelectionE2ETestCase,
    SinceShallowCloneE2ETestCase,
    SinceSharedBaseE2ETestCase,
    SinceUnwritableCacheE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    build_since_indirect_files,
    build_since_project_files,
    build_since_sql_macro_files,
    delete_project_files,
    prepare_since_repository,
    run_git,
    write_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_FILES: dict[str, str] = build_since_project_files()
_STAGING: str = "sql/models/stg_orders.sql"
_MACRO: str = "sql/models/_macros/python/money.py"
_EDITED_STAGING: str = _FILES[_STAGING].replace(
    "SELECT order_id, status", "SELECT order_id, LOWER(status) AS status"
)
_EDITED_MACRO: str = _FILES[_MACRO].replace("/ 100", "/ 100.0")
_REFORMATTED_STAGING: str = (
    "MODEL (description 'Cleaned and documented orders.');\n\n"
    "-- one row per order\nselect\n  order_id,\n  status,\n  amount_cents\n"
    'from __source("raw_orders")\n'
)
_ARCHIVE: str = "sql/models/order_archive.sql"
_ARCHIVE_SQL: str = (
    "MODEL (description 'Archived orders.');\n\nSELECT order_id FROM __ref(\"stg_orders\")\n"
)
_TEST_COMMAND: tuple[str, ...] = ("--no-color", "test", "--run-on", "warehouse", "--since", "main")
_BASE_REF_CONFIG: str = _FILES["sqlbuild_project.toml"] + '\n[git]\nbase_ref = "main"\n'
_INDIRECT: dict[str, str] = build_since_indirect_files()
_PROJECT_CONFIG: str = "sqlbuild_project.toml"
_SCENARIO_COMMAND: tuple[str, ...] = ("--no-color", "scenario", "test", "--since", "main")
_EXPLAINED_TESTS: tuple[str, ...] = (*_TEST_COMMAND, "--explain")
_EXPLAINED_SCENARIOS: tuple[str, ...] = (*_SCENARIO_COMMAND, "--explain")
_SQL_MACROS: dict[str, str] = build_since_sql_macro_files()
_INNER_SQL_MACRO: str = "sql/models/_macros/sql/round_cents.sql"
_OUTER_SQL_MACRO: str = "sql/models/_macros/sql/net_dollars.sql"
_SLOT_MACRO: str = "sql/models/_macros/sql/status_totals.sql"
_SLOT_MODEL: str = "sql/models/order_status_totals.sql"
_SLOT_FILES: dict[str, str] = {
    _SLOT_MACRO: (
        "MACRO (\n  description 'Order totals per grain.',\n"
        "  arguments (src RELATION (keys COLUMNS, amount_cents INTEGER)),\n);\n\n"
        "WITH totals AS (\n"
        "  SELECT @src.keys, SUM(@src.amount_cents) AS total_cents\n"
        "  FROM @src\n"
        "  GROUP BY @src.keys\n"
        ")\n"
    ),
    _SLOT_MODEL: (
        "MODEL (description 'Order totals per status.');\n\n"
        'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
        "@sql.status_totals(raw, src.keys := (status))\n"
        "SELECT * FROM totals\n"
    ),
    "tests/unit/test_order_status_totals.sql": (
        "TEST ();\n\nWITH\n"
        "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
        "__expected__order_status_totals AS (SELECT 'open' AS status, 250 AS total_cents)\n"
        "SELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    (
        SinceSelectionE2ETestCase(
            description="an uncommitted model edit selects only the test that really runs it",
            edits={_STAGING: _EDITED_STAGING},
            command=(*_TEST_COMMAND, "--explain"),
            expected_exit_code=0,
            expected_fragments=(
                "Comparing with main (merge base ",
                "Compiled base revision ",
                "Changed since main: 1 model changed",
                "Selected 1 of 3 tests",
                "test_stg_orders\n    executes stg_orders (model SQL changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("test_order_totals", "test_open_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="a committed macro edit selects the test whose model calls the macro",
            edits={_MACRO: _EDITED_MACRO},
            command=(*_TEST_COMMAND, "--explain"),
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 macro changed",
                "test_order_totals\n    executes macro cents_to_dollars (changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("test_stg_orders", "test_open_orders"),
            git_after_edits=("commit", "--quiet", "--all", "-m", "edit"),
        ),
        SinceSelectionE2ETestCase(
            description="formatting, comments and descriptions change nothing",
            edits={_STAGING: _REFORMATTED_STAGING},
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: no definition changes",
                "No tests are affected by the changes since main",
                "PASS=0  FAIL=0  TOTAL=0",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="an edited test file selects that test",
            edits={
                "tests/unit/test_open_orders.sql": _FILES["tests/unit/test_open_orders.sql"]
                + "-- covers open orders\n"
            },
            command=(*_TEST_COMMAND, "--inspect"),
            expected_exit_code=0,
            expected_fragments=(
                "Selected 1 of 3 tests",
                "test_open_orders\n",
                "  selected: test file changed",
                "Test plan inspection complete: 1 selected, 0 errors.",
            ),
            unexpected_fragments=("test_stg_orders\n", "test_order_totals\n"),
        ),
        SinceSelectionE2ETestCase(
            description="selectors intersect with the affected tests",
            edits={_STAGING: _EDITED_STAGING, _MACRO: _EDITED_MACRO},
            command=(*_TEST_COMMAND, "order_totals"),
            expected_exit_code=0,
            expected_fragments=("Selected 1 of 1 test\n", "PASS=1  FAIL=0  TOTAL=1"),
            unexpected_fragments=("test_stg_orders",),
        ),
        SinceSelectionE2ETestCase(
            description="exclude removes affected tests",
            edits={_STAGING: _EDITED_STAGING, _MACRO: _EDITED_MACRO},
            command=(*_TEST_COMMAND, "--exclude", "test:test_order_totals"),
            expected_exit_code=0,
            expected_fragments=("Selected 1 of 2 tests", "PASS=1  FAIL=0  TOTAL=1"),
            unexpected_fragments=("test_order_totals",),
        ),
        SinceSelectionE2ETestCase(
            description="a deleted model is listed and selects nothing",
            edits={},
            deletions=(_ARCHIVE,),
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: deleted: model order_archive",
                "No tests are affected by the changes since main",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a pure rename through migrate_from selects nothing",
            edits={
                "sql/models/order_history.sql": _ARCHIVE_SQL.replace(
                    "MODEL (", "MODEL (migrate_from order_archive, "
                ),
            },
            deletions=(_ARCHIVE,),
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 model renamed",
                "No tests are affected by the changes since main",
            ),
            unexpected_fragments=("deleted:",),
        ),
        SinceSelectionE2ETestCase(
            description="a missing ref fails instead of selecting nothing",
            edits={},
            command=("--no-color", "test", "--since", "origin/missing"),
            expected_exit_code=1,
            expected_fragments=(
                "error[C481]: Git ref 'origin/missing' does not name a commit in this repository",
                "git fetch origin <branch>",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="@base without a configured base ref fails",
            edits={},
            command=("--no-color", "test", "--since", "@base"),
            expected_exit_code=1,
            expected_fragments=(
                "error[C485]: --since @base needs [git] base_ref in sqlbuild_project.toml",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="--explain without --since runs every test and explains where",
            edits={},
            command=("--no-color", "test", "--explain"),
            expected_exit_code=0,
            expected_fragments=("Tests ready  3 selected, 3 local", "PASS=3  FAIL=0  TOTAL=3"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_changes_since_ref_when_testing_then_runs_only_affected_tests(
    tmp_path: Path, test_case: SinceSelectionE2ETestCase
) -> None:
    project: Path = prepare_since_repository(
        tmp_path=tmp_path, extra_files={_ARCHIVE: _ARCHIVE_SQL}
    )
    write_project_files(project_dir=project, files=test_case.edits)
    delete_project_files(project_dir=project, relative_paths=test_case.deletions)
    run_git(repository=project, arguments=test_case.git_after_edits)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceSelectionE2ETestCase(
            description="a project variable a model reads selects that model's test",
            edits={
                _PROJECT_CONFIG: _INDIRECT[_PROJECT_CONFIG].replace(
                    'min_amount = "100"', 'min_amount = "200"'
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 model changed\n",
                "Selected 1 of 13 tests",
                "test_big_orders\n    executes big_orders (model vars changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a path default compares the resolved model configuration",
            edits={
                _PROJECT_CONFIG: _INDIRECT[_PROJECT_CONFIG]
                + '\n[path_defaults.marts]\nmaterialized = "view"\n'
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 1 of 13 tests",
                "test_order_counts\n    executes order_counts (model config changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a helper module a macro imports changes the macro",
            edits={"python/nodes/_rates.py": 'DIVISOR = "100.0"\n'},
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 macro changed\n",
                "Selected 2 of 13 tests",
                "test_order_totals\n    executes macro cents_to_dollars (changed)",
                "test_order_totals_from_source\n    executes macro cents_to_dollars (changed)",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a function body change selects the test that calls it",
            edits={
                "sql/functions/sql/is_open.sql": _INDIRECT["sql/functions/sql/is_open.sql"].replace(
                    "order_status = 'open'", "LOWER(order_status) = 'open'"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 function changed\n",
                "Selected 1 of 13 tests",
                "test_order_flags\n    executes function is_open (changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="an audit definition change selects tests of models it is attached to",
            edits={
                "sql/audits/generic/at_least.sql": _INDIRECT[
                    "sql/audits/generic/at_least.sql"
                ].replace("@column < @minimum", "@column <= @minimum")
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 audit changed\n",
                "Selected 2 of 13 tests",
                "test_order_totals\n    executes audit at_least (changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="an upstream change selects a test that runs it but not one that mocks it",
            edits={
                _STAGING: _INDIRECT[_STAGING].replace(
                    "SELECT order_id, status", "SELECT order_id, LOWER(status) AS status"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "test_order_totals_from_source\n    executes stg_orders (model SQL changed)",
                "test_archive_counts\n    executes stg_orders (model SQL changed)",
            ),
            unexpected_fragments=("  test_order_totals\n", "test_big_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="a rename through migrate_from that also changes the SQL is a change",
            edits={
                "sql/models/order_history.sql": _INDIRECT["sql/models/order_archive.sql"]
                .replace("MODEL (", "MODEL (migrate_from order_archive, ")
                .replace('__ref("stg_orders")', '__ref("stg_orders") WHERE order_id > 0'),
                "sql/models/archive_counts.sql": _INDIRECT["sql/models/archive_counts.sql"].replace(
                    "order_archive", "order_history"
                ),
            },
            deletions=("sql/models/order_archive.sql",),
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 2 models changed; 1 model renamed",
                "test_archive_counts\n"
                "    executes order_history (renamed from order_archive, model SQL changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("deleted:",),
        ),
        SinceSelectionE2ETestCase(
            description="seed data a test mocks selects nothing",
            edits={"sql/seeds/regions.csv": "region_id,region_name\n1,south\n"},
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 seed changed\n",
                "No tests are affected by the changes since main",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="seed data selects the scenario that loads the seed",
            edits={"sql/seeds/regions.csv": "region_id,region_name\n1,south\n"},
            command=_EXPLAINED_SCENARIOS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 1 of 3 scenarios",
                "regional_orders_end_to_end\n    reads seed regions (changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a seed column type selects the scenario that loads the seed",
            edits={
                "sql/seeds/regions.yml": _INDIRECT["sql/seeds/regions.yml"].replace(
                    "type: VARCHAR", "type: TEXT"
                )
            },
            command=_EXPLAINED_SCENARIOS,
            expected_exit_code=0,
            expected_fragments=("regional_orders_end_to_end\n    reads seed regions (changed)",),
        ),
        SinceSelectionE2ETestCase(
            description="an audit definition change selects scenarios that build its model",
            edits={
                "sql/audits/generic/at_least.sql": _INDIRECT[
                    "sql/audits/generic/at_least.sql"
                ].replace("@column < @minimum", "@column <= @minimum")
            },
            command=_EXPLAINED_SCENARIOS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 1 of 3 scenarios",
                "order_totals_end_to_end\n    executes audit at_least (changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a scenario fixture edit selects that scenario",
            edits={
                "tests/scenarios/regional_orders_end_to_end.sql": _INDIRECT[
                    "tests/scenarios/regional_orders_end_to_end.sql"
                ].replace("250 AS amount_cents", "300 AS amount_cents")
            },
            command=_EXPLAINED_SCENARIOS,
            expected_exit_code=0,
            expected_fragments=("regional_orders_end_to_end\n    scenario file changed",),
            unexpected_fragments=("order_totals_end_to_end",),
        ),
        SinceSelectionE2ETestCase(
            description="a mocked source's declared column type selects the tests that mock it",
            edits={
                "sql/sources/raw.yml": _FILES["sql/sources/raw.yml"].replace(
                    "amount_cents\n        type: INTEGER", "amount_cents\n        type: BIGINT"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 source changed\n",
                "Selected 5 of 13 tests",
                "test_stg_orders\n    mocks source raw_orders (source columns changed)",
            ),
            unexpected_fragments=("test_order_flags",),
        ),
        SinceSelectionE2ETestCase(
            description="a mocked model's declared column type selects the tests that mock it",
            edits={
                _STAGING: _INDIRECT[_STAGING].replace(
                    "amount_cents (type INTEGER)", "amount_cents (type BIGINT)"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 13 of 13 tests",
                "test_big_orders\n    mocks stg_orders (model columns changed)",
                "test_archive_counts\n    executes stg_orders (model columns changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a fixture's declared column type selects the scenario that replaces it",
            edits={
                _STAGING: _INDIRECT[_STAGING].replace(
                    "amount_cents (type INTEGER)", "amount_cents (type BIGINT)"
                )
            },
            command=_EXPLAINED_SCENARIOS,
            expected_exit_code=0,
            expected_fragments=(
                "open_orders_from_fixture\n    mocks stg_orders (model columns changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a macro another project-wide macro imports changes both",
            edits={
                "sql/macros/python/taxes.py": _INDIRECT["sql/macros/python/taxes.py"].replace(
                    "+ 0", "+ 1 - 1"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 2 macros changed\n",
                "Selected 1 of 13 tests",
                "test_order_gross\n    executes macro gross (changed)\n"
                "    executes macro with_tax (changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a scoped macro another scoped macro imports changes both",
            edits={
                "sql/models/_macros/python/currency.py": _INDIRECT[
                    "sql/models/_macros/python/currency.py"
                ].replace("* 1)", "* 1.0)")
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 2 of 13 tests",
                "test_order_totals\n    executes macro cents_to_dollars (changed)\n"
                "    executes macro whole_cents (changed)",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a macro a parameterized test's own fixture calls selects every case",
            edits={
                "sql/macros/python/fixture_values.py": _INDIRECT[
                    "sql/macros/python/fixture_values.py"
                ].replace("\"''\"", "\"'x'\"")
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Selected 2 of 13 tests",
                "status_cases [open_case]\n    executes macro suffix (changed)",
                "status_cases [closed_case]\n    executes macro suffix (changed)",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a connection setting selects every test, including the one it breaks",
            edits={
                _PROJECT_CONFIG: _INDIRECT[_PROJECT_CONFIG].replace(
                    '[connection]\ndatabase = "since_orders.duckdb"\n',
                    '[connection]\ndatabase = "since_orders.duckdb"\n\n'
                    '[connection.settings]\nTimeZone = "Asia/Tokyo"\n',
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=1,
            expected_fragments=(
                "Changed since main: 1 connection and target configuration changed\n",
                "Selected 13 of 13 tests",
                "test_order_day\n    uses connection and target configuration (changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a tracked Python dependency lock file selects every test",
            edits={"uv.lock": "version = 1\n"},
            git_after_edits=("add", "uv.lock"),
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 Python environment changed\n",
                "Selected 13 of 13 tests",
                "test_order_day\n    uses Python environment (changed)",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="an untracked lock file is not part of the project",
            edits={"uv.lock": "version = 1\n", "requirements-local.txt": "pytest\n"},
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: no definition changes",
                "No tests are affected by the changes since main",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a target the base does not define compiles the base with its default",
            edits={
                _PROJECT_CONFIG: _INDIRECT[_PROJECT_CONFIG] + '\n[targets.ci]\nschema = "main"\n'
            },
            command=("--no-color", "test", "--target", "ci", "--since", "main"),
            expected_exit_code=0,
            expected_fragments=(
                "Target 'ci' is not defined at the merge base, so the base compiles with its "
                "default target",
                "Selected 13 of 13 tests",
                "PASS=13  FAIL=0  TOTAL=13",
            ),
        ),
        SinceSelectionE2ETestCase(
            description="a Python node change is listed",
            edits={
                "python/nodes/tasks/report/asset.py": _INDIRECT[
                    "python/nodes/tasks/report/asset.py"
                ].replace("payload={}", 'payload={"orders": 1}')
            },
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=("Changed since main: 1 asset changed\n",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_indirect_changes_since_ref_when_testing_then_selects_what_executes_them(
    tmp_path: Path, test_case: SinceSelectionE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files=_INDIRECT)
    write_project_files(project_dir=project, files=test_case.edits)
    delete_project_files(project_dir=project, relative_paths=test_case.deletions)
    run_git(repository=project, arguments=test_case.git_after_edits)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceBrokenBaseE2ETestCase(
            description="a base revision that does not compile fails and says what to do",
            base_files={
                "sql/models/broken.sql": (
                    "MODEL (description 'Reads a model that does not exist.');\n\n"
                    'SELECT order_id FROM __ref("missing_orders")\n'
                )
            },
            deletions=("sql/models/broken.sql",),
            command=_TEST_COMMAND,
            expected_exit_code=1,
            expected_fragments=(
                "error[C484]: Cannot compile the project at merge base ",
                "missing_orders",
                "Compare with a later ref whose project compiles, or run without --since",
            ),
            unexpected_fragments=("Traceback", 'File "'),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_base_that_does_not_compile_when_testing_since_then_fails_with_guidance(
    tmp_path: Path, test_case: SinceBrokenBaseE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files=test_case.base_files)
    delete_project_files(project_dir=project, relative_paths=test_case.deletions)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceSharedBaseE2ETestCase(
            description="a scenario run reuses the base compile of the test run",
            edits={_STAGING: _EDITED_STAGING, "sqlbuild_project.toml": _BASE_REF_CONFIG},
            test_command=("--no-color", "test", "--since", "@base"),
            scenario_command=("--no-color", "scenario", "test", "--since", "@base", "--explain"),
            expected_test_fragments=("Compiled base revision ", "Selected 1 of 3 tests"),
            expected_scenario_fragments=(
                "Using cached base revision ",
                "Changed since main: 1 model changed",
                "Selected 1 of 2 scenarios",
                "order_totals_end_to_end\n    executes stg_orders (model SQL changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_scenario_fragments=("open_orders_from_fixture",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_test_and_scenario_runs_when_comparing_then_both_share_one_base_compile(
    tmp_path: Path, test_case: SinceSharedBaseE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files={})
    write_project_files(project_dir=project, files=test_case.edits)

    tested: CompletedProcess[str] = run_sqb(command=test_case.test_command, project_dir=project)
    scenarios: CompletedProcess[str] = run_sqb(
        command=test_case.scenario_command, project_dir=project
    )

    tested_output: str = tested.stdout + tested.stderr
    scenario_output: str = scenarios.stdout + scenarios.stderr
    assert tested.returncode == 0, tested_output
    assert scenarios.returncode == 0, scenario_output
    for fragment in test_case.expected_test_fragments:
        assert fragment in tested_output, tested_output
    for fragment in test_case.expected_scenario_fragments:
        assert fragment in scenario_output, scenario_output
    for fragment in test_case.unexpected_scenario_fragments:
        assert fragment not in scenario_output, scenario_output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceJsonOutputE2ETestCase(
            description="progress and the selection go to stderr",
            edits={_STAGING: _EDITED_STAGING},
            command=("--no-color", "test", "--since", "main", "--json"),
            expected_stderr_fragments=("Comparing with main", "Selected 1 of 3 tests"),
            expected_test_names=("test_stg_orders",),
            unexpected_stdout_fragments=("test_order_totals", "Comparing with"),
            expected_selected={"test_stg_orders": ("executes stg_orders (model SQL changed)",)},
        ),
        SinceJsonOutputE2ETestCase(
            description="a scenario run lists each selected scenario's reasons",
            edits={_STAGING: _EDITED_STAGING},
            command=("--no-color", "scenario", "test", "--since", "main", "--json"),
            expected_stderr_fragments=("Comparing with main", "Selected 1 of 2 scenarios"),
            expected_test_names=("order_totals_end_to_end",),
            unexpected_stdout_fragments=("open_orders_from_fixture", "Comparing with"),
            expected_selected={
                "order_totals_end_to_end": ("executes stg_orders (model SQL changed)",)
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_json_output_when_testing_since_ref_then_stdout_stays_machine_readable(
    tmp_path: Path, test_case: SinceJsonOutputE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files={})
    write_project_files(project_dir=project, files=test_case.edits)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    assert result.returncode == 0, result.stdout + result.stderr
    document: str = json.dumps(json.loads(result.stdout))
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stderr
    for name in test_case.expected_test_names:
        assert f'"{name}"' in document, document
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout, result.stdout
    reference: str = json.loads(result.stdout)["since"]["ref"]
    selected: list[dict[str, str | list[str]]] = json.loads(result.stdout)["since"]["selected"]
    assert reference == "main", result.stdout
    assert {
        str(item["name"]): tuple(item["reasons"]) for item in selected
    } == test_case.expected_selected, result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        SinceShallowCloneE2ETestCase(
            description="a depth-one clone of both branches lacks the merge base",
            clone_arguments=("--depth", "1", "--branch", "feature", "--no-single-branch"),
            fetch_arguments=("fetch", "--quiet", "--depth", "1", "origin", "main"),
            command=("--no-color", "test", "--since", "origin/main"),
            expected_exit_code=1,
            expected_fragments=(
                "error[C482]: Cannot find the merge base of 'origin/main' and HEAD",
                "git fetch --unshallow",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_shallow_clone_without_merge_base_when_testing_since_then_asks_for_depth(
    tmp_path: Path, test_case: SinceShallowCloneE2ETestCase
) -> None:
    origin: Path = prepare_since_repository(tmp_path=tmp_path, extra_files={})
    run_git(repository=origin, arguments=("commit", "--quiet", "--allow-empty", "-m", "next"))
    clone: Path = tmp_path / "clone"
    run_git(
        repository=tmp_path,
        arguments=("clone", "--quiet", *test_case.clone_arguments, origin.as_uri(), str(clone)),
    )
    run_git(repository=clone, arguments=test_case.fetch_arguments)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=clone)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceUnwritableCacheE2ETestCase(
            description="a file where the cache directory belongs warns and the run continues",
            blocking_file="target/since",
            command=("--no-color", "test", "--since", "main", "--json"),
            expected_stderr_fragments=(
                "warning: Could not cache the base revision at ",
                "; continuing without the cache",
                "Selected 1 of 3 tests",
            ),
            expected_test_names=("test_stg_orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unwritable_cache_when_testing_since_then_warns_and_succeeds(
    tmp_path: Path, test_case: SinceUnwritableCacheE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files={})
    write_project_files(
        project_dir=project,
        files={_STAGING: _EDITED_STAGING, test_case.blocking_file: "not a directory\n"},
    )

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stderr
    assert result.stderr.count("warning: Could not cache") == 1, result.stderr
    selected: list[dict[str, str | list[str]]] = json.loads(result.stdout)["since"]["selected"]
    assert tuple(str(item["name"]) for item in selected) == test_case.expected_test_names


@pytest.mark.parametrize(
    "test_case",
    (
        SinceSelectionE2ETestCase(
            description="a sql macro edit selects the test whose model calls the macro",
            edits={_OUTER_SQL_MACRO: _SQL_MACROS[_OUTER_SQL_MACRO].replace("- 1", "- 2")},
            command=_EXPLAINED_TESTS,
            expected_exit_code=1,
            expected_fragments=(
                "Changed since main: 1 macro changed",
                "Selected 1 of 4 tests",
                "test_order_net\n    executes macro net_dollars (changed)",
                "FAIL=1",
            ),
            unexpected_fragments=("test_stg_orders", "test_order_totals", "test_open_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="a sql macro called inside another sql macro selects the outer caller's test",
            edits={_INNER_SQL_MACRO: _SQL_MACROS[_INNER_SQL_MACRO].replace("/ 100,", "/ 100.0,")},
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 macro changed",
                "Selected 1 of 4 tests",
                "test_order_net\n    executes macro round_cents (changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("test_stg_orders", "test_order_totals", "test_open_orders"),
            git_after_edits=("commit", "--quiet", "--all", "-m", "edit"),
        ),
        SinceSelectionE2ETestCase(
            description="changing a sql macro's returns form selects the test of its caller",
            edits={
                _OUTER_SQL_MACRO: _SQL_MACROS[_OUTER_SQL_MACRO].replace(
                    "returns DOUBLE", "returns TYPE OF cents"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 macro changed",
                "test_order_net\n    executes macro net_dollars (changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("test_stg_orders", "test_order_totals", "test_open_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="naming the CTEs of a CTE-list sql macro call selects the caller's test",
            edits={
                "sql/models/_macros/sql/live_orders.sql": (
                    "MACRO (\n  description 'Orders still open.',\n"
                    "  arguments (src RELATION (order_id INTEGER, status VARCHAR)),\n);\n\n"
                    "WITH live AS (SELECT * FROM @src WHERE status = 'open')\n"
                ),
                "sql/models/order_live.sql": (
                    "MODEL (description 'Live orders.');\n\n"
                    'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.live_orders(raw, __cte := open_orders)\n"
                    "SELECT order_id FROM open_orders\n"
                ),
            },
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=("Changed since main: 1 model, 1 macro added",),
            unexpected_fragments=("FAIL=1",),
        ),
        SinceSelectionE2ETestCase(
            description="reformatting, commenting and redescribing a sql macro changes nothing",
            edits={
                _OUTER_SQL_MACRO: (
                    "MACRO (description 'Net dollars after the one-dollar fee.', "
                    "arguments (cents INTEGER), returns DOUBLE);\n\n"
                    "-- the fee is one dollar\n@sql.round_cents(@cents)   -   1\n"
                )
            },
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: no definition changes",
                "No tests are affected by the changes since main",
            ),
            unexpected_fragments=("test_order_net",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_macro_changes_since_ref_when_testing_then_selects_tests_of_their_callers(
    tmp_path: Path, test_case: SinceSelectionE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files=_SQL_MACROS)
    write_project_files(project_dir=project, files=test_case.edits)
    run_git(repository=project, arguments=test_case.git_after_edits)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SinceSelectionE2ETestCase(
            description="a changed slot mapping changes the calling model",
            edits={
                _SLOT_MODEL: _SLOT_FILES[_SLOT_MODEL].replace(
                    "src.keys := (status)", "src.keys := (status, order_id)"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 model changed",
                "test_order_status_totals\n    executes order_status_totals (model SQL changed)",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            unexpected_fragments=("test_stg_orders", "test_order_totals", "test_open_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="a changed slot declaration changes the macro",
            edits={
                _SLOT_MACRO: _SLOT_FILES[_SLOT_MACRO].replace(
                    "amount_cents INTEGER", "amount_cents BIGINT"
                )
            },
            command=_EXPLAINED_TESTS,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: 1 macro changed",
                "test_order_status_totals\n    executes macro status_totals (changed)",
            ),
            unexpected_fragments=("test_stg_orders", "test_order_totals", "test_open_orders"),
        ),
        SinceSelectionE2ETestCase(
            description="a reformatted slot mapping and slot read change nothing",
            edits={
                _SLOT_MODEL: _SLOT_FILES[_SLOT_MODEL].replace(
                    "src.keys := (status)", "src.keys := ( status )"
                ),
                _SLOT_MACRO: _SLOT_FILES[_SLOT_MACRO].replace(
                    "  GROUP BY @src.keys\n", "  -- one row per grain\n  GROUP BY   @src.keys\n"
                ),
            },
            command=_TEST_COMMAND,
            expected_exit_code=0,
            expected_fragments=(
                "Changed since main: no definition changes",
                "No tests are affected by the changes since main",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_slot_changes_since_ref_when_testing_then_selects_tests_like_column_changes(
    tmp_path: Path, test_case: SinceSelectionE2ETestCase
) -> None:
    project: Path = prepare_since_repository(tmp_path=tmp_path, extra_files=_SLOT_FILES)
    write_project_files(project_dir=project, files=test_case.edits)
    run_git(repository=project, arguments=test_case.git_after_edits)

    result: CompletedProcess[str] = run_sqb(command=test_case.command, project_dir=project)

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output
