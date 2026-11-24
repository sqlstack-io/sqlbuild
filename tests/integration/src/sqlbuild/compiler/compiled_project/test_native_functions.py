"""Functions attached by the native model loop and assembled into the native compiled project."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledFunctionView, CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    NativeFunctionAttachmentTestCase,
    NativeFunctionErrorTestCase,
    PythonTableFunctionCompileTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    FUNCTION_PROJECT_FILES,
    TABLE_FUNCTION_PROJECT_FILES,
    compile_function_project,
    compile_snowflake_project,
    function_summary,
    table_function_summary,
)

if TYPE_CHECKING:
    from sqlbuild._native import FunctionInput


_SMOOTH_SUMMARY: tuple[object, ...] = (
    "table_fn",
    "python",
    "TABLE",
    (("device_id", "INTEGER"), ("smoothed", "FLOAT")),
    "main",
)
_RANK_SUMMARY: tuple[object, ...] = (
    "table_fn",
    "python",
    "TABLE",
    (("position", "INTEGER"),),
    "main",
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeFunctionAttachmentTestCase(
            description="scalar SQL function with templated types under the default schema",
            function_name="order_total",
            expected_summary=(
                "NativeCompiledFunction",
                "udf",
                (),
                (("amount", "DECIMAL(10, 2)"),),
                "DECIMAL(10, 2)",
                (),
                (),
                "sql",
                "udfs_eu.order_total",
                "udfs_eu.order_total",
                (None, (), ("finance",), "Order total."),
            ),
            expected_usages=(),
        ),
        NativeFunctionAttachmentTestCase(
            description="table SQL function reading a model and an enum in its own schema",
            function_name="open_orders",
            expected_summary=(
                "NativeCompiledFunction",
                "table_fn",
                ("orders",),
                (),
                "TABLE",
                (("order_id", "INTEGER"), ("status", "VARCHAR")),
                ("orders",),
                "sql",
                "reporting.open_orders",
                "reporting.open_orders",
                (None, (), (), "Open orders."),
            ),
            expected_usages=("enum:order_status",),
        ),
        NativeFunctionAttachmentTestCase(
            description="Python function fingerprinted in the default schema it does not inherit",
            function_name="score",
            expected_summary=(
                "NativeCompiledFunction",
                "udf",
                (),
                (("value", "INTEGER"),),
                "INTEGER",
                (),
                (),
                "python",
                "score",
                "udfs_eu.score",
                ("3.11", ("numpy",), ("ml",), "Score."),
            ),
            expected_usages=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_function_when_compiling_then_native_project_holds_its_attachment(
    test_case: NativeFunctionAttachmentTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project, inputs = compile_function_project(
        project_dir=tmp_path, files=FUNCTION_PROJECT_FILES, monkeypatch=monkeypatch
    )

    function: CompiledFunctionView = {item.name: item for item in project.functions}[
        test_case.function_name
    ]
    function_input: FunctionInput = {item.name: item for item in inputs.sql_function_inputs}[
        test_case.function_name
    ]
    assert function_summary(function) == test_case.expected_summary
    assert type(function_input).__name__ == "NativeFunctionInput"
    assert (
        tuple(
            f"{usage.declaration.kind.value}:{usage.declaration.name}"
            for usage in function_input.declaration_usages
        )
        == test_case.expected_usages
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeFunctionErrorTestCase(
            description="table function called with the wrong argument count",
            files={
                "sql/functions/sql/customer_orders.sql": (
                    "FUNCTION (description 'Customer orders.', arguments (customer_id INTEGER),"
                    " returns table (order_id INTEGER));\nSELECT customer_id AS order_id\n"
                ),
                "sql/models/report.sql": (
                    "MODEL (description 'Report.');\n"
                    'SELECT * FROM __table_fn("customer_orders")(1, 2)\n'
                ),
            },
            expected_message=(
                "Model file sql/models/report.sql table function 'customer_orders' expects 1"
                " argument but received 2"
            ),
        ),
        NativeFunctionErrorTestCase(
            description="function body reading an unknown model",
            files={
                "sql/functions/sql/lookup.sql": (
                    "FUNCTION (description 'Lookup.', returns INTEGER);\n"
                    'SELECT COUNT(*) FROM __ref("missing_orders")\n'
                ),
            },
            expected_message=(
                "SQL function file sql/functions/sql/lookup.sql references unknown model"
                " 'missing_orders'"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_function_when_compiling_then_native_attachment_rejects_it(
    test_case: NativeFunctionErrorTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with pytest.raises(CompileInputError) as raised:
        _ = compile_function_project(
            project_dir=tmp_path,
            files={**FUNCTION_PROJECT_FILES, **test_case.files},
            monkeypatch=monkeypatch,
        )

    assert test_case.expected_message in str(raised.value)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonTableFunctionCompileTestCase(
            description="polars by default and pandas from a pd.DataFrame annotation",
            files=TABLE_FUNCTION_PROJECT_FILES,
            expected_summaries={
                "smooth": (*_SMOOTH_SUMMARY, "polars"),
                "rank_readings": (*_RANK_SUMMARY, "pandas"),
            },
        ),
        PythonTableFunctionCompileTestCase(
            description="a pandas project default applies to an unannotated function",
            files={
                **TABLE_FUNCTION_PROJECT_FILES,
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "snowflake"\n'
                    '[defaults]\nfunction_dataframe = "pandas"\n'
                ),
                "sql/functions/python/smooth.py": TABLE_FUNCTION_PROJECT_FILES[
                    "sql/functions/python/smooth.py"
                ].replace("readings: pl.DataFrame", "readings"),
            },
            expected_summaries={
                "smooth": (*_SMOOTH_SUMMARY, "pandas"),
                "rank_readings": (*_RANK_SUMMARY, "pandas"),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_table_functions_when_compiling_then_each_keeps_its_columns_and_frame(
    test_case: PythonTableFunctionCompileTestCase, tmp_path: Path
) -> None:
    project: CompiledProjectView = compile_snowflake_project(
        project_dir=tmp_path, files=test_case.files
    )

    assert {
        function.name: table_function_summary(function) for function in project.functions
    } == test_case.expected_summaries


@pytest.mark.parametrize(
    "test_case",
    [
        NativeFunctionErrorTestCase(
            description="a Python table function called without a partition",
            files={
                "sql/models/smoothed.sql": (
                    "MODEL (description 'Smoothed.');\nSELECT s.* FROM __ref(\"readings\") r,\n"
                    '  __table_fn("smooth")(r.device_id, r.reading) OVER () s\n'
                ),
            },
            expected_message=(
                "Model file sql/models/smoothed.sql calls Python table function 'smooth' without"
                " OVER (PARTITION BY ...)"
            ),
        ),
        NativeFunctionErrorTestCase(
            description="a SQL test calling a Python table function without a partition",
            files={
                "tests/unit/test_smooth.sql": (
                    'TEST (mode table_fn, name "smooth_rows");\n'
                    "WITH\nreadings AS (SELECT 1 AS device_id, 2.0::FLOAT AS reading),\n"
                    "__table_fn_actual__ AS (\n  SELECT s.device_id, s.smoothed FROM readings r,\n"
                    '    __table_fn("smooth")(r.device_id, r.reading) s\n),\n'
                    "__table_fn_expected__ AS (\n"
                    "  SELECT 1 AS device_id, 2.0::FLOAT AS smoothed\n)\n"
                ),
            },
            expected_message=(
                "SQL test file tests/unit/test_smooth.sql calls Python table function 'smooth'"
                " without OVER (PARTITION BY ...)"
            ),
        ),
        NativeFunctionErrorTestCase(
            description="a polars table function with a fractional NUMBER argument",
            files={
                "sql/functions/python/smooth.py": TABLE_FUNCTION_PROJECT_FILES[
                    "sql/functions/python/smooth.py"
                ].replace("'reading': 'FLOAT'}", "'reading': 'NUMBER(10, 2)'}"),
            },
            expected_message=(
                "Python function sql/functions/python/smooth.py argument 'reading' is NUMBER(10, 2),"
                " which Snowflake passes to a polars frame as Python Decimal objects"
            ),
        ),
        NativeFunctionErrorTestCase(
            description="a table function returning a scalar type",
            files={
                "sql/functions/python/rank_readings.py": TABLE_FUNCTION_PROJECT_FILES[
                    "sql/functions/python/rank_readings.py"
                ].replace("returns={'position': 'INTEGER'}", "returns='INTEGER'"),
            },
            expected_message=(
                "sql/functions/python/rank_readings.py returns must map each column name to its type"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_python_table_function_when_compiling_then_compile_rejects_it(
    test_case: NativeFunctionErrorTestCase, tmp_path: Path
) -> None:
    with pytest.raises(CompileInputError) as raised:
        _ = compile_snowflake_project(
            project_dir=tmp_path, files={**TABLE_FUNCTION_PROJECT_FILES, **test_case.files}
        )

    assert test_case.expected_message in str(raised.value)
