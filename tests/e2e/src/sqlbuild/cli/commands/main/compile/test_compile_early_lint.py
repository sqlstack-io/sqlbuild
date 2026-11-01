"""E2E tests for the speculative SQL lint started during compilation."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    EarlyLintCompileTestCase,
    EarlyLintInterruptTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    compile_json_payload,
    diagnostic_location_keys,
    restore_default_interrupt,
    slow_lint_model_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project

_PROJECT_TOML: str = 'name = "commerce"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN005"]\n'
_ORDERS_SQL: str = 'MODEL (description "Orders");\nSELECT 1 AS order_id, 2 AS amount\n'


@pytest.mark.parametrize(
    "test_case",
    (
        EarlyLintCompileTestCase(
            description="graph error discards the speculative lint",
            order_totals_sql=(
                'MODEL (description "Order totals");\n'
                "WITH\n"
                "  unused_orders AS (SELECT 1 AS order_id),\n"
                '  totals AS (SELECT order_id, missing_column FROM __ref("orders"))\n'
                "SELECT order_id FROM totals\n"
            ),
            expected_exit_code=1,
            expected_diagnostics=(("B002", "models/order_totals.sql", 4, 31),),
        ),
        EarlyLintCompileTestCase(
            description="clean graph reports every lint finding at its authored line",
            order_totals_sql=(
                'MODEL (description "Order totals");\n'
                "WITH\n"
                "  unused_orders AS (SELECT 1 AS order_id),\n"
                '  totals AS (SELECT order_id, amount FROM __ref("orders")),\n'
                "\n"
                "  unused_amounts AS (\n"
                "    SELECT amount FROM totals\n"
                "  ),\n"
                "  unused_counts AS (SELECT COUNT(*) AS order_count FROM totals)\n"
                "SELECT order_id, amount FROM totals\n"
            ),
            expected_exit_code=1,
            expected_diagnostics=(
                ("SQLCLEAN005", "models/order_totals.sql", 3, 3),
                ("SQLCLEAN005", "models/order_totals.sql", 6, 3),
                ("SQLCLEAN005", "models/order_totals.sql", 9, 3),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rules_enabled_project_when_compiling_cold_and_warm_then_lint_wait_is_attributed(
    test_case: EarlyLintCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="commerce",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "models/orders.sql": _ORDERS_SQL,
            "models/order_totals.sql": test_case.order_totals_sql,
        },
    )

    cold_exit: int
    cold: dict[str, Any]
    cold_exit, cold = compile_json_payload(project_dir=project_dir)
    warm_exit: int
    warm: dict[str, Any]
    warm_exit, warm = compile_json_payload(project_dir=project_dir)

    assert (cold_exit, warm_exit) == (test_case.expected_exit_code, test_case.expected_exit_code)
    assert diagnostic_location_keys(cold) == test_case.expected_diagnostics
    assert diagnostic_location_keys(warm) == test_case.expected_diagnostics
    for payload in (cold, warm):
        wait_ms: object = payload["compile_timings"]["early_lint_wait_ms"]
        assert isinstance(wait_ms, int)
        assert wait_ms >= 0


@pytest.mark.parametrize(
    "test_case",
    (
        EarlyLintInterruptTestCase(
            description="ctrl-c stops after the file being expanded",
            model_count=8,
            macro_seconds=3.0,
            expected_max_stop_seconds=8.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_slow_background_lint_when_interrupted_then_compile_stops_promptly(
    test_case: EarlyLintInterruptTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="commerce",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "models/orders.sql": _ORDERS_SQL,
            **slow_lint_model_files(model_count=test_case.model_count),
        },
    )
    marker: Path = tmp_path / "slow_macro_marker.txt"
    process: subprocess.Popen[str] = subprocess.Popen(
        [str(Path(sys.executable).with_name("sqb")), "--project-dir", str(project_dir), "compile"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env={
            **os.environ,
            "SLOW_MACRO_MARKER": str(marker),
            "SLOW_MACRO_SECONDS": str(test_case.macro_seconds),
        },
        preexec_fn=restore_default_interrupt,
    )
    deadline: float = time.monotonic() + 60.0
    while not marker.exists() and time.monotonic() < deadline and process.poll() is None:
        time.sleep(0.05)
    interrupted_at: float = time.monotonic()
    process.send_signal(signal.SIGINT)
    output: str = process.communicate(timeout=120)[0]
    stop_seconds: float = time.monotonic() - interrupted_at

    assert process.returncode in (-signal.SIGINT, 130), output
    assert stop_seconds < test_case.expected_max_stop_seconds, output
    assert marker.read_text(encoding="utf-8").count("expanding") < test_case.model_count
