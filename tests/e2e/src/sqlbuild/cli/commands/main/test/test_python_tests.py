"""E2E tests for the project Python tests `sqb test` runs with pytest."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    PythonTestsE2ETestCase,
    PythonTestsJsonE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)

PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "python_tests_project"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "python_tests_project.duckdb"\n'
    ),
    "sql/models/orders.sql": (
        'MODEL (description "Test model orders.");\n\nSELECT status FROM __source("raw_orders")\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n    table: raw_orders\n"
    ),
    "tests/unit/test_orders.sql": (
        'TEST (name "order_status");\n\n'
        "WITH\n"
        "__source__raw_orders AS (SELECT 'open' AS status),\n"
        "__expected__orders AS (SELECT 'open' AS status)\n"
        "SELECT 1\n"
    ),
    "python/nodes/orders/storefront_import/node.py": (
        "from sqlbuild.assets import asset\n\n\n"
        "@asset(description='Import storefront orders.', tags=['ingest'])\n"
        "def storefront_import(ctx):\n    return None\n"
    ),
    "python/nodes/orders/storefront_import/_helpers/row_mapping.py": (
        "def map_row(raw):\n    return {'status': str(raw['state']).lower()}\n"
    ),
    "python/nodes/inventory/stock_sync/node.py": (
        "from sqlbuild.assets import asset\n\n\n"
        "@asset(description='Sync stock.')\n"
        "def stock_sync(ctx):\n    return None\n"
    ),
}

PASSING_PYTHON_TEST: dict[str, str] = {
    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py": (
        "from python.nodes.orders.storefront_import._helpers.row_mapping import map_row\n\n\n"
        "def test_given_paid_order_when_mapping_then_status_is_paid():\n"
        "    assert map_row({'state': 'PAID'}) == {'status': 'paid'}\n"
    ),
}

ORCHESTRATION_PYTHON_TEST: dict[str, str] = {
    "orchestration/sensors/orders/_helpers/restock_window.py": (
        "def restock_due(hours_since_publish):\n    return hours_since_publish >= 6\n"
    ),
    "tests/orchestration/sensors/orders/_helpers/test_restock_window.py": (
        "from orchestration.sensors.orders._helpers.restock_window import restock_due\n\n\n"
        "def test_given_six_hours_when_checking_then_restock_is_due():\n"
        "    assert restock_due(6)\n"
    ),
}

FAILING_PYTHON_TEST: dict[str, str] = {
    "tests/python/nodes/inventory/stock_sync/test_node.py": (
        "def test_given_stock_when_syncing_then_fails():\n    assert False\n"
    ),
}

BROKEN_MODEL: dict[str, str] = {
    "sql/models/broken.sql": (
        'MODEL (description "Test model broken.");\n\nSELECT missing_column FROM __ref("orders")\n'
    ),
}

PASSING_FILE: str = "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py"
FAILING_FILE: str = "tests/python/nodes/inventory/stock_sync/test_node.py"

SQL_PASSED: str = "PASS=1  FAIL=0"
PYTHON_PASSED: str = "Python tests passed  1 file"
PYTHON_STARTED: str = "Running Python tests with pytest"
PYTEST_MISSING: str = "Python tests found but pytest isn't installed"

HIDDEN_PYTEST_SITECUSTOMIZE: str = """
import sys


class _HidePytest:
    def __init__(self, finder):
        self.finder = finder

    def find_spec(self, name, path=None, target=None):
        return None if name.split(".")[0] == "pytest" else self.finder.find_spec(name, path, target)

    def __getattr__(self, name):
        return getattr(self.finder, name)


sys.meta_path[:] = [_HidePytest(finder) for finder in sys.meta_path]
"""


@pytest.mark.parametrize(
    "test_case",
    [
        PythonTestsE2ETestCase(
            description="without Python tests only SQL tests run",
            command=("--no-color", "test"),
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
        PythonTestsE2ETestCase(
            description="with Python tests both SQL and Python tests run",
            command=("--no-color", "test"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED, PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=(),
        ),
        PythonTestsE2ETestCase(
            description="an orchestration test mirroring its sensor code runs",
            command=("--no-color", "test", "--python"),
            extra_repo_files=ORCHESTRATION_PYTHON_TEST,
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=(SQL_PASSED,),
        ),
        PythonTestsE2ETestCase(
            description="a failing Python test fails the command after the SQL tests",
            command=("--no-color", "test"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=1,
            expected_output_fragments=(
                SQL_PASSED,
                "Python tests failed  pytest exit code 1",
                "1 failed, 1 passed",
            ),
            expected_absent_fragments=(),
        ),
        PythonTestsE2ETestCase(
            description="--sql runs only SQL tests",
            command=("--no-color", "test", "--sql"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
        PythonTestsE2ETestCase(
            description="--python runs only Python tests",
            command=("--no-color", "test", "--python"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED,),
            expected_absent_fragments=(SQL_PASSED,),
        ),
        PythonTestsE2ETestCase(
            description="--python without Python tests is an error",
            command=("--no-color", "test", "--python"),
            extra_repo_files={},
            expected_exit_code=1,
            expected_output_fragments=("no Python tests found",),
            expected_absent_fragments=(SQL_PASSED,),
        ),
        PythonTestsE2ETestCase(
            description="--select of a Python node runs only the tests in its mirrored folder",
            command=("--no-color", "test", "--select", "storefront_import"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=(SQL_PASSED, "failed"),
        ),
        PythonTestsE2ETestCase(
            description="--select of a glob reaching a Python node runs its mirrored tests",
            command=("--no-color", "test", "--select", "storefront_*"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=("failed",),
        ),
        PythonTestsE2ETestCase(
            description="--select of a tag on a Python node runs its mirrored tests",
            command=("--no-color", "test", "--select", "tag:ingest"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=("failed",),
        ),
        PythonTestsE2ETestCase(
            description="--select of a Python node's path runs its mirrored tests",
            command=("--no-color", "test", "--select", "python/nodes/orders/storefront_import"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(PYTHON_PASSED, "1 passed"),
            expected_absent_fragments=("failed",),
        ),
        PythonTestsE2ETestCase(
            description="--select of a glob reaching only models leaves an unrelated broken model unanalysed",
            command=("--no-color", "test", "--select", "ord*"),
            extra_repo_files={**PASSING_PYTHON_TEST, **BROKEN_MODEL},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=("B002", PYTHON_STARTED),
        ),
        PythonTestsE2ETestCase(
            description="--select of a model runs its SQL tests and no Python tests",
            command=("--no-color", "test", "--select", "orders"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
        PythonTestsE2ETestCase(
            description="--select of a model leaves an unrelated broken model unanalysed",
            command=("--no-color", "test", "--select", "orders"),
            extra_repo_files={**PASSING_PYTHON_TEST, **BROKEN_MODEL},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=("B002", PYTHON_STARTED),
        ),
        PythonTestsE2ETestCase(
            description="--select of a unit test runs it and no Python tests",
            command=("--no-color", "test", "--select", "order_status"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
        PythonTestsE2ETestCase(
            description="--python with --inspect is an error",
            command=("--no-color", "test", "--python", "--inspect"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=1,
            expected_output_fragments=("--python cannot be combined with --inspect or --since",),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
        PythonTestsE2ETestCase(
            description="build never runs Python tests",
            command=("--no-color", "build", "--select", "storefront_import"),
            extra_repo_files={**PASSING_PYTHON_TEST, **FAILING_PYTHON_TEST},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_absent_fragments=(PYTHON_STARTED,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_python_tests_when_running_sqb_test_then_runs_the_requested_kinds(
    test_case: PythonTestsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_tests_project",
        repo_files={**PROJECT_FILES, **test_case.extra_repo_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        PythonTestsE2ETestCase(
            description="without Python tests pytest is not needed",
            command=("--no-color", "test"),
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTEST_MISSING,),
        ),
        PythonTestsE2ETestCase(
            description="Python tests without pytest fail before any test runs",
            command=("--no-color", "test"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=1,
            expected_output_fragments=(PYTEST_MISSING, "sqb test --sql"),
            expected_absent_fragments=(SQL_PASSED,),
        ),
        PythonTestsE2ETestCase(
            description="--sql runs SQL tests without pytest",
            command=("--no-color", "test", "--sql"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=0,
            expected_output_fragments=(SQL_PASSED,),
            expected_absent_fragments=(PYTEST_MISSING,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_pytest_missing_when_running_sqb_test_then_only_python_tests_need_it(
    test_case: PythonTestsE2ETestCase,
    tmp_path: Path,
) -> None:
    hidden: Path = tmp_path / "hidden_pytest"
    hidden.mkdir()
    (hidden / "sitecustomize.py").write_text(HIDDEN_PYTEST_SITECUSTOMIZE, encoding="utf-8")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_tests_project",
        repo_files={**PROJECT_FILES, **test_case.extra_repo_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"PYTHONPATH": os.pathsep.join((str(hidden), os.environ.get("PYTHONPATH", "")))},
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        PythonTestsJsonE2ETestCase(
            description="SQL and Python tests passing give one passing document",
            command=("--no-color", "test", "--json"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=0,
            expected_status="success",
            expected_python_status="success",
            expected_python_files=(PASSING_FILE,),
        ),
        PythonTestsJsonE2ETestCase(
            description="a failing Python test fails the document whose SQL tests passed",
            command=("--no-color", "test", "--json"),
            extra_repo_files=FAILING_PYTHON_TEST,
            expected_exit_code=1,
            expected_status="failed",
            expected_python_status="failed",
            expected_python_files=(FAILING_FILE,),
        ),
        PythonTestsJsonE2ETestCase(
            description="--python still writes the document with no SQL tests",
            command=("--no-color", "test", "--json", "--python"),
            extra_repo_files=PASSING_PYTHON_TEST,
            expected_exit_code=0,
            expected_status="success",
            expected_python_status="success",
            expected_python_files=(PASSING_FILE,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_json_output_when_running_python_tests_then_stdout_is_one_document_with_them(
    test_case: PythonTestsJsonE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_tests_project",
        repo_files={**PROJECT_FILES, **test_case.extra_repo_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    document: dict[str, object] = json.loads(result.stdout)
    assert document["status"] == test_case.expected_status, document
    assert document["python_tests"] == {
        "status": test_case.expected_python_status,
        "exit_code": test_case.expected_exit_code,
        "files": list(test_case.expected_python_files),
    }, document
    assert "1 passed" in result.stderr or "1 failed" in result.stderr, result.stderr
