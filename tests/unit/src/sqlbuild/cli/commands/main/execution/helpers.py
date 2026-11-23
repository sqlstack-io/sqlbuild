"""Test helpers for build execution command boundaries."""

import sys
from contextlib import nullcontext
from io import StringIO
from pathlib import Path
from unittest.mock import Mock

import pytest

from sqlbuild.diagnostics.main import process_resource_reporting as resource_reporting_module
from sqlbuild.executor.pipeline.main import run as executor_run_module
from tests.unit.src.sqlbuild.presentation.helpers import use_plain_terminal


def fail_build_discovery(
    *, monkeypatch: pytest.MonkeyPatch, project_dir: Path, provider_source: str
) -> Mock:
    """Make project discovery fail in a provider module and record debug resource reporting."""

    providers: Path = project_dir / "python" / "nodes" / "python" / "providers"
    providers.mkdir(parents=True)
    _ = (project_dir / "sqlbuild_project.toml").write_text(
        'name = "shop"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    _ = (providers / "failing.py").write_text(provider_source, encoding="utf-8")
    reporter: Mock = Mock(return_value=nullcontext())
    monkeypatch.setattr(resource_reporting_module, "process_resource_reporting", reporter)
    return reporter


class ScenarioProgressStream(StringIO):
    """A progress stream that reports whether it is a terminal."""

    def __init__(self, *, tty: bool) -> None:
        super().__init__()
        self._tty: bool = tty

    def isatty(self) -> bool:
        return self._tty


_SCENARIO_PROJECT_TOML: str = (
    'name = "project"\nadapter = "duckdb"\n\n[connection]\ndatabase = "project.duckdb"\n'
)
_SCENARIO_SOURCES_YAML: str = (
    "sources:\n"
    "  - name: raw_orders\n    description: Test source raw_orders.\n"
    "    schema: main\n"
    "    table: raw_orders\n"
    "    columns:\n"
    "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
)
_SCENARIO_MODEL_SQL: str = (
    'MODEL (description "Test model orders.");\n\nSELECT order_id FROM __source("raw_orders")\n'
)
VALID_SCENARIO_SQL: str = (
    "SCENARIO (description 'Test scenario orders_paid.');\n\n"
    "WITH\n"
    "__source__raw_orders AS (SELECT 1 AS order_id),\n"
    "__expected__orders AS (SELECT 1 AS order_id)\n"
    "SELECT 1\n"
)
INVALID_SCENARIO_SQL: str = "SCENARIO(description 'Test scenario.');\n\nSELECT 1\n"


def configure_native_scenario_command(
    *,
    monkeypatch: pytest.MonkeyPatch,
    stream: ScenarioProgressStream,
    scenario_sql: str,
    project_dir: Path,
) -> None:
    """Write a one-scenario DuckDB project and isolate the command from scenario execution."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _SCENARIO_PROJECT_TOML,
        "sql/sources/raw.yml": _SCENARIO_SOURCES_YAML,
        "sql/models/orders.sql": _SCENARIO_MODEL_SQL,
        "tests/scenarios/orders_paid.sql": scenario_sql,
    }
    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    monkeypatch.setattr(sys, "stdout", stream)
    use_plain_terminal(monkeypatch)
    monkeypatch.setattr(executor_run_module, "run_scenario_test_pipeline", lambda **_: ())
