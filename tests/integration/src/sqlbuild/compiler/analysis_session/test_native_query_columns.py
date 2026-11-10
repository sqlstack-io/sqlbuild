"""Column inference outside the analysis session: models native analysis cannot project and
table-function bodies keep the columns Python's parsed-tree inference gave them.

Expected outputs recorded from the pre-port Python `infer_columns_with_sql_analysis`.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

from sqlbuild._native import NativeColumnQuery, infer_native_query_columns
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from tests.integration.src.sqlbuild.compiler.analysis_session._test_types import (
    FallbackColumnsTestCase,
    NormalizationRejectionTestCase,
    TableFunctionColumnsTestCase,
)
from tests.integration.src.sqlbuild.compiler.lineage.helpers import compiled_project

_PROJECT: str = 'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
_HEADER: str = "MODEL (description 'Test model.', materialized view);\n"


@pytest.mark.parametrize(
    "test_case",
    [
        FallbackColumnsTestCase(
            description="CREATE TABLE AS, not a query, has unknown columns",
            query_sql="CREATE TABLE t AS SELECT 1 AS a",
            expected_columns=(),
        ),
        FallbackColumnsTestCase(
            description="INSERT ... SELECT, not a query, has unknown columns",
            query_sql="INSERT INTO t SELECT 1 AS a",
            expected_columns=(),
        ),
        FallbackColumnsTestCase(
            description="calls nested 100 deep",
            query_sql="SELECT " + "f(" * 100 + "1" + ")" * 100 + " AS deep",
            expected_columns=(("deep", None, "unknown"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unprojectable_model_when_compiling_then_parsed_tree_columns_are_kept(
    test_case: FallbackColumnsTestCase, tmp_path: Path
) -> None:
    project: CompiledProjectView = compiled_project(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": _PROJECT,
            "models/orders.sql": _HEADER + test_case.query_sql,
        },
    )
    model: CompiledModelView = project.models[0]

    assert model.inferred_columns is not None
    assert (
        tuple(
            (column.name, column.type, column.nullability.value)
            for column in model.inferred_columns
        )
        == test_case.expected_columns
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableFunctionColumnsTestCase(
            description="matching return columns",
            returns="table (id INTEGER, label VARCHAR)",
            body_sql="SELECT x AS id, 'a' AS label",
            expected_error=None,
        ),
        TableFunctionColumnsTestCase(
            description="fewer body columns than declared",
            returns="table (id INTEGER, label VARCHAR)",
            body_sql="SELECT x AS id",
            expected_error=(
                "SQL table function functions/sql/table_fn__orders.sql declares 2 return "
                "columns but its query produces 1"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_when_compiling_then_body_columns_match_declaration(
    test_case: TableFunctionColumnsTestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT,
        "functions/sql/table_fn__orders.sql": (
            "FUNCTION (description 'Test function.', arguments (x INTEGER), "
            f"returns {test_case.returns}); {test_case.body_sql}"
        ),
    }
    error: str | None = None
    try:
        _ = compiled_project(project_dir=tmp_path, files=files)
    except CompileInputError as raised:
        error = str(raised)

    assert error == test_case.expected_error


@pytest.mark.parametrize(
    "test_case",
    [
        NormalizationRejectionTestCase(
            description="a parse query whose table-function call never closes",
            queries=(("SELECT 1 AS a", "parse"), ('SELECT * FROM __table_fn("f")(1', "parse")),
            dialect="duckdb",
            expected_message="invalid table-function analysis input: Parenthesis",
        ),
        NormalizationRejectionTestCase(
            description="a legacy query whose table-function argument quote never closes",
            queries=(('SELECT * FROM __table_fn("f")(\'x)', "legacy"),),
            dialect="motherduck",
            expected_message="invalid table-function analysis input: Quote",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_normalization_rejects_when_inferring_columns_then_python_value_error_raises(
    test_case: NormalizationRejectionTestCase,
) -> None:
    queries: tuple[NativeColumnQuery, ...] = tuple(
        NativeColumnQuery(sql=sql, mode=mode) for sql, mode in test_case.queries
    )

    with pytest.raises(ValueError, match=re.escape(test_case.expected_message)):
        _ = infer_native_query_columns(
            queries=queries,
            profile=ExpressionInferenceProfile(sql_analysis_dialect=test_case.dialect),
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
