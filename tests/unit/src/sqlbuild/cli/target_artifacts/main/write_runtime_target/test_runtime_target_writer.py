from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import PlanOutput, write_runtime_target
from sqlbuild.executor.build.models import BuildExecutionResult
from tests.unit.src.sqlbuild.cli.commands.main.compile.helpers import (
    build_runtime_target_execution_result,
    build_target_writer_plan_output,
    read_target_files,
)
from tests.unit.src.sqlbuild.cli.target_artifacts.main.write_runtime_target._test_types import (
    TargetWriterTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWriterTestCase(
            description="writes runtime SQL from executed statements",
            expected_files={
                "run/sql/models/staging/orders.sql": (
                    "DROP TABLE IF EXISTS analytics.orders__staging;\n\n"
                    "CREATE OR REPLACE TABLE analytics.orders__staging AS SELECT 1 AS order_id;\n"
                ),
                "run/sql/functions/sql/is_completed_order.sql": (
                    "CREATE OR REPLACE FUNCTION analytics.is_completed_order"
                    "(order_status VARCHAR) RETURNS BOOLEAN;\n"
                ),
                "run/sql/functions/python/is_completed_order_py.sql": (
                    "REGISTER PYTHON FUNCTION analytics.is_completed_order_py"
                    "(VARCHAR) RETURNS BOOLEAN;\n"
                ),
            },
        ),
        TargetWriterTestCase(
            description="preserves existing runtime files outside current result scope",
            initial_files={
                "run/sql/models/marts/fact_orders.sql": "SELECT 1;\n",
            },
            expected_files={
                "run/sql/models/marts/fact_orders.sql": "SELECT 1;\n",
                "run/sql/models/staging/orders.sql": (
                    "DROP TABLE IF EXISTS analytics.orders__staging;\n\n"
                    "CREATE OR REPLACE TABLE analytics.orders__staging AS SELECT 1 AS order_id;\n"
                ),
                "run/sql/functions/sql/is_completed_order.sql": (
                    "CREATE OR REPLACE FUNCTION analytics.is_completed_order"
                    "(order_status VARCHAR) RETURNS BOOLEAN;\n"
                ),
                "run/sql/functions/python/is_completed_order_py.sql": (
                    "REGISTER PYTHON FUNCTION analytics.is_completed_order_py"
                    "(VARCHAR) RETURNS BOOLEAN;\n"
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_execution_result_when_writing_runtime_target_then_expected_files_are_written(
    test_case: TargetWriterTestCase,
    tmp_path: Path,
) -> None:
    plan_output: PlanOutput = build_target_writer_plan_output()
    result: BuildExecutionResult = build_runtime_target_execution_result()

    relative_path: str
    contents: str
    for relative_path, contents in test_case.initial_files.items():
        target_file: Path = (tmp_path / "target") / relative_path
        target_file.parent.mkdir(parents=True, exist_ok=True)
        target_file.write_text(contents, encoding="utf-8")

    write_runtime_target(
        target_dir=tmp_path / "target",
        plan_output=plan_output,
        result=result,
    )

    assert (
        read_target_files(tmp_path / "target", test_case.expected_files) == test_case.expected_files
    )
