import subprocess
import sys
from pathlib import Path
from textwrap import dedent

import pytest

from tests.unit.src.sqlbuild.compiler.planner.classes._test_types import PlanningExitTestCase

_SLOW_TEST_PLANNING: str = dedent(
    """
    import sys
    import threading
    import time
    import types

    import sqlbuild._native as native
    from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


    class SlowFinalizer:
        def __del__(self):
            print("finalizing", sys.is_finalizing(), file=sys.stderr, flush=True)
            time.sleep(2.0)


    holder = types.ModuleType("holder")
    holder.finalizer = SlowFinalizer()
    sys.modules["holder"] = holder
    set_difference_operator = DuckDbAdapter.render_set_difference_operator


    def slow_set_difference_operator(self):
        if threading.current_thread() is not threading.main_thread():
            time.sleep(1.0)
        return set_difference_operator(self)


    DuckDbAdapter.render_set_difference_operator = slow_set_difference_operator
    """
)
_FAILED_BUILD: str = dedent(
    """
    from sqlbuild.cli.entry.main.entry import main

    sys.exit(main(["--project-dir", sys.argv[1], "--no-color", "build"]))
    """
)
_UNJOINED_PLANNING: str = dedent(
    """
    from sqlbuild._native import SqlTestSelection


    class SlowSqlTestsProject:
        @property
        def sql_tests(self):
            time.sleep(1.0)
            return ()


    native.start_sql_test_planning(
        project=SlowSqlTestsProject(),
        dialect=native.Dialect(DuckDbAdapter()),
        selected_keys=frozenset(),
        sql_test_selection=SqlTestSelection(),
        enabled=True,
    )
    print("planning started", file=sys.stderr, flush=True)
    """
)


@pytest.mark.parametrize(
    "test_case",
    (
        PlanningExitTestCase(
            "plan fails while SQL tests are still planning",
            _SLOW_TEST_PLANNING + _FAILED_BUILD,
            1,
            "error[S405]: cannot build selected scope",
        ),
        PlanningExitTestCase(
            "SQL test planning never joined",
            _SLOW_TEST_PLANNING + _UNJOINED_PLANNING,
            0,
            "planning started",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_test_planning_running_when_interpreter_finalizes_then_process_exits_cleanly(
    test_case: PlanningExitTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "orders_project"
    (project_dir / "sql" / "models").mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders_project"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "orders_project.duckdb"\n',
        encoding="utf-8",
    )
    (project_dir / "sql" / "sources").mkdir()
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: raw\n    table: orders\n",
        encoding="utf-8",
    )
    (project_dir / "sql" / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table);\n\n"
        'SELECT * FROM __source("raw_orders")\n',
        encoding="utf-8",
    )
    (project_dir / "tests" / "unit").mkdir(parents=True)
    (project_dir / "tests" / "unit" / "test_orders.sql").write_text(
        "TEST ();\n\nWITH\n__source__raw_orders AS (\n  SELECT 21 AS order_id\n),\n"
        "__expected__orders AS (\n  SELECT 21 AS order_id\n)\nSELECT 1\n",
        encoding="utf-8",
    )
    script: Path = tmp_path / "planning_exit.py"
    script.write_text(test_case.script, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [sys.executable, str(script), str(project_dir)],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert test_case.expected_output in result.stdout + result.stderr
    assert "finalizing True" in result.stderr
