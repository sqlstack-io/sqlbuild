"""Tests for supported keys in SQL FUNCTION headers and Python @udf decorators."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    DiscoveredPythonFunctionFile,
    DiscoveredSqlFunctionFile,
    ModelSqlParseError,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    FunctionHeaderKeysTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import (
    discover_declaration_file,
    discover_python_function_file,
)


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionHeaderKeysTestCase(
            description="sql udf with every supported key",
            file_name="normalize_status.sql",
            contents=(
                'FUNCTION (\n  description "Normalise an order status",\n'
                "  arguments (status VARCHAR),\n  returns VARCHAR,\n"
                "  database analytics,\n  schema udfs,\n  tags [orders],\n);\n\nlower(status)\n"
            ),
            expected_keys=("arguments", "database", "description", "returns", "schema", "tags"),
        ),
        FunctionHeaderKeysTestCase(
            description="table function with a returns table declaration",
            file_name="expand_order.sql",
            contents=(
                "FUNCTION (\n  arguments (order_id INTEGER),\n"
                "  returns table (order_id INTEGER),\n);\n\nSELECT order_id\n"
            ),
            expected_keys=("arguments", "returns"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_supported_function_header_keys_when_parsing_then_accepts_header(
    test_case: FunctionHeaderKeysTestCase, tmp_path: Path
) -> None:
    header_values: dict[str, object] = cast(
        DiscoveredSqlFunctionFile,
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path=f"sql/functions/sql/{test_case.file_name}",
            contents=test_case.contents,
        ),
    ).header_values

    assert tuple(sorted(header_values)) == test_case.expected_keys


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionHeaderKeysTestCase(
            description="sql udf with a replay setting",
            file_name="normalize_status.sql",
            contents=(
                "FUNCTION (\n  arguments (status VARCHAR),\n  returns VARCHAR,\n"
                "  replay_on_change full,\n);\n\nlower(status)\n"
            ),
            expected_error=(
                "FUNCTION() in 'normalize_status.sql:4' has unsupported keys: replay_on_change"
            ),
        ),
        FunctionHeaderKeysTestCase(
            description="table function with two unknown keys",
            file_name="expand_order.sql",
            contents=(
                'FUNCTION (\n  description "Expand one order",\n  arguments (order_id INTEGER),\n'
                "  returns table (order_id INTEGER),\n  owner orders_team,\n  team sales,\n);\n\n"
                "SELECT order_id\n"
            ),
            expected_error=("FUNCTION() in 'expand_order.sql:5' has unsupported keys: owner, team"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_sql_function_header_key_when_parsing_then_rejects_key_with_location(
    test_case: FunctionHeaderKeysTestCase, tmp_path: Path
) -> None:
    file_path: Path = tmp_path / "sql" / "functions" / "sql" / test_case.file_name
    with pytest.raises(ModelSqlParseError) as raised:
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path=f"sql/functions/sql/{test_case.file_name}",
            contents=test_case.contents,
        )

    assert raised.value.message == test_case.expected_error.replace(
        f"'{test_case.file_name}", f"'{file_path}"
    )
    assert raised.value.code == "D002"


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionHeaderKeysTestCase(
            description="python udf with a replay setting",
            file_name="normalize_status_py.py",
            contents=(
                "from sqlbuild.functions import udf\n\n\n@udf(\n"
                '    arguments={"status": "VARCHAR"},\n    returns="VARCHAR",\n'
                '    replay_on_change="full",\n)\ndef main(status):\n    return status\n'
            ),
            expected_error=(
                "@udf(...) in 'normalize_status_py.py:7' has unsupported keys: replay_on_change"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_python_udf_key_when_parsing_then_rejects_key_with_location(
    test_case: FunctionHeaderKeysTestCase, tmp_path: Path
) -> None:
    file_path: Path = tmp_path / "sql" / "functions" / "python" / test_case.file_name
    with pytest.raises(ModelSqlParseError) as raised:
        discover_python_function_file(
            project_dir=tmp_path, file_name=test_case.file_name, contents=test_case.contents
        )

    assert raised.value.message == test_case.expected_error.replace(
        f"'{test_case.file_name}", f"'{file_path}"
    )
    assert raised.value.help == (
        "supported keys: arguments, database, description, entry_point, packages, returns, "
        "runtime_version, schema, tags"
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionHeaderKeysTestCase(
            description="docstring describes the udf",
            file_name="is_large_order_py.py",
            contents=(
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN")\n'
                'def main(amount):\n    """Whether an order amount is large."""\n'
                "    return amount > 100\n"
            ),
            expected_description="Whether an order amount is large.",
        ),
        FunctionHeaderKeysTestCase(
            description="decorator description wins over the docstring",
            file_name="is_large_order_py.py",
            contents=(
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(description="Large order flag", arguments={"amount": "INTEGER"}, '
                'returns="BOOLEAN")\n'
                'def main(amount):\n    """Whether an order amount is large."""\n'
                "    return amount > 100\n"
            ),
            expected_description="Large order flag",
        ),
        FunctionHeaderKeysTestCase(
            description="undocumented udf has no description",
            file_name="is_large_order_py.py",
            contents=(
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN")\n'
                "def main(amount):\n    return amount > 100\n"
            ),
            expected_description=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_udf_when_parsing_then_description_comes_from_decorator_or_docstring(
    test_case: FunctionHeaderKeysTestCase, tmp_path: Path
) -> None:
    function_file: DiscoveredPythonFunctionFile = discover_python_function_file(
        project_dir=tmp_path, file_name=test_case.file_name, contents=test_case.contents
    )
    header_values: dict[str, object] = function_file.header_values

    assert header_values.get("description") == test_case.expected_description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
