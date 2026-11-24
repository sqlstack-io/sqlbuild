"""E2E: macro, UDF and table-function tests are split into CTEs under DuckDB's lexical rules.

These tests are extracted natively before and after expansion, so comments, DuckDB
nested comments and escape strings hide calls the same way, and the strict authoring rules (plain CTE
names, no logic calls outside the actual CTE, P012 for malformed reference calls) report the same
located errors.
"""

from __future__ import annotations

from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    DirectLogicExtractionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_MACRO_TEST: str = "tests/unit/test_tidy_label.sql"
_UDF_TEST: str = "tests/unit/test_order_label.sql"
_MODEL_TEST: str = "tests/unit/test_order_summary.sql"
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "direct_logic"\nadapter = "duckdb"\n\n[connection]\ndatabase = "direct_logic.duckdb"\n'
    ),
    "sql/models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table);\n\n"
        "SELECT 1 AS order_id, 'Placed ' AS status\n"
    ),
    "tests/unit/_macros/python/labels.py": (
        "def tidy_label(expression: str) -> str:\n"
        '    """Trim and lower-case a label."""\n'
        '    return f"LOWER(TRIM({expression}))"\n'
    ),
    "sql/functions/sql/order_label.sql": (
        'FUNCTION (\n  description "Label an order status.",\n'
        "  arguments (raw_status VARCHAR),\n  returns VARCHAR,\n);\n\nUPPER(raw_status)\n"
    ),
    "sql/functions/sql/order_rows.sql": (
        'FUNCTION (\n  description "Orders above an id.",\n'
        "  arguments (min_id INTEGER),\n  returns table (\n    order_id INTEGER\n  ),\n);\n\n"
        "SELECT order_id FROM (VALUES (1), (2)) AS t(order_id) WHERE order_id > min_id\n"
    ),
    _MACRO_TEST: (
        "TEST (mode macro);\n\nWITH\n"
        "input_values AS (\n"
        "  SELECT ' Placed ' AS status -- @py.tidy_label(\"status\")\n"
        "),\n"
        '__macro_actual__ AS (SELECT @py.tidy_label("status") AS status FROM input_values),\n'
        "__macro_expected__ AS (SELECT 'placed' AS status)\n"
    ),
    _UDF_TEST: (
        "TEST (mode udf);\n\nWITH\n"
        "input_values AS (\n"
        "  SELECT 'placed' AS status, E'it\\'s), (' AS note -- __udf(\"order_label\")(status)\n"
        "),\n"
        '__udf_actual__ AS (SELECT __udf("order_label")(status) AS label FROM input_values),\n'
        "__udf_expected__ AS (\n"
        "  SELECT 'PLACED' AS label /* /* __udf(\"order_label\")('x') */ */\n"
        ")\n"
    ),
    "sql/models/order_summary.sql": (
        "MODEL (description 'Order summary.', materialized table);\n\n"
        'SELECT order_id, TRIM(status) AS status, order_id * 100 AS amount_cents FROM __ref("stg_orders")\n'
    ),
    _MODEL_TEST: (
        "TEST ();\n\nWITH\n"
        "__ref__stg_orders AS (SELECT 1 AS order_id, 'placed' AS status),\n"
        "__expected__order_summary AS (\n"
        "  SELECT 1 order_id, 'placed' \"status\", CAST(100 AS INTEGER) amount_cents\n"
        ")\n"
    ),
    "tests/unit/test_order_rows.sql": (
        "TEST (mode table_fn);\n\nWITH\n"
        '__table_fn_actual__ AS (SELECT order_id FROM __table_fn("order_rows")(1)),\n'
        "__table_fn_expected__ AS (SELECT 2 AS order_id)\n"
    ),
}
_QUOTED_NAME: dict[str, str] = {
    _UDF_TEST: (
        "TEST (mode udf);\n\nWITH\n"
        "\"input values\" AS (SELECT 'placed' AS status),\n"
        '__udf_actual__ AS (SELECT __udf("order_label")(status) AS label FROM "input values"),\n'
        "__udf_expected__ AS (SELECT 'PLACED' AS label)\n"
    )
}
_HELPER_CALLS_UDF: dict[str, str] = {
    _MACRO_TEST: (
        "TEST (mode macro);\n\nWITH\n"
        "input_values AS (SELECT __udf(\"order_label\")(' Placed ') AS status),\n"
        '__macro_actual__ AS (SELECT @py.tidy_label("status") AS status FROM input_values),\n'
        "__macro_expected__ AS (SELECT 'placed' AS status)\n"
    )
}
_MALFORMED_SOURCE: dict[str, str] = {
    _UDF_TEST: (
        "TEST (mode udf);\n\nWITH\n"
        'input_values AS (SELECT status FROM __source("raw", "orders")),\n'
        '__udf_actual__ AS (SELECT __udf("order_label")(status) AS label FROM input_values),\n'
        "__udf_expected__ AS (SELECT 'PLACED' AS label)\n"
    )
}
_LOCATED_MODEL_NAME: dict[str, str] = {
    _MODEL_TEST: (
        "TEST ();\n"
        "-- the café helper holds the expected rows\n"
        "    WITH\n"
        "    __ref__stg_orders AS (SELECT 1 AS order_id, 'placed' AS status),\n"
        "    \tcafé AS (SELECT 1 AS order_id),\n"
        "    __expected__order_summary AS (SELECT order_id FROM café)\n"
    )
}
_LOCATED_CASE_NAME: dict[str, str] = {
    _MACRO_TEST: (
        "TEST (\n  mode macro,\n  parameters (qty integer),\n"
        "  cases (one (qty 1)),\n);\n"
        "-- a$b is named here before its CTE\n"
        "WITH\n"
        "input_values AS (SELECT ' Placed ' AS status, @param(\"qty\") AS qty),\n"
        "  a$b AS (SELECT 1 AS x),\n"
        '__macro_actual__ AS (SELECT @py.tidy_label("status") AS status FROM input_values),\n'
        "__macro_expected__ AS (SELECT 'placed' AS status)\n"
    )
}
_REPEATED_MALFORMED_SOURCE: dict[str, str] = {
    _UDF_TEST: (
        "TEST (mode udf);\n\nWITH\n"
        'input_values AS (SELECT status FROM __source("raw", "orders")),\n'
        "checked AS (\n"
        '  SELECT status FROM input_values WHERE EXISTS (SELECT 1 FROM __source("raw", "orders"))\n'
        "),\n"
        '__udf_actual__ AS (SELECT __udf("order_label")(status) AS label FROM checked),\n'
        "__udf_expected__ AS (SELECT 'PLACED' AS label)\n"
    )
}
_OPERATOR_MODEL: str = (
    "MODEL (description 'Order summary.', materialized table);\n\n"
    "SELECT order_id, TRIM(status) AS status, order_id * 100 AS amount_cents,\n"
    "  TRIM(status) IS DISTINCT FROM 'shipped' AS is_open,\n"
    "  TRIM(status) IS NOT DISTINCT FROM 'placed' AS is_placed\n"
    'FROM __ref("stg_orders")\n'
)
_ALIASED_OPERATORS: dict[str, str] = {
    "sql/models/order_summary.sql": _OPERATOR_MODEL,
    _MODEL_TEST: (
        "TEST ();\n\nWITH\n"
        "__ref__stg_orders AS (SELECT 1 AS order_id, 'placed' AS status),\n"
        "__expected__order_summary AS (\n"
        "  SELECT 1 order_id, 'placed' \"status\", CAST(100 AS INTEGER) amount_cents,\n"
        "    'placed' IS DISTINCT FROM 'shipped' AS is_open,\n"
        "    'placed' IS NOT DISTINCT FROM 'placed' AS is_placed\n"
        ")\n"
    ),
    _MACRO_TEST: (
        "TEST (mode macro);\n\nWITH\n"
        "input_values AS (SELECT ' Placed ' AS status),\n"
        "__macro_actual__ AS (\n"
        '  SELECT @py.tidy_label("status") AS status, [1, 2] AS item_ids FROM input_values\n'
        "),\n"
        "__macro_expected__ AS (SELECT 'placed' AS status, [1, 2] AS item_ids)\n"
    ),
}
_UNALIASED_DISTINCT_FROM: dict[str, str] = {
    "sql/models/order_summary.sql": _OPERATOR_MODEL,
    _MODEL_TEST: (
        "TEST ();\n\nWITH\n"
        "__ref__stg_orders AS (SELECT 1 AS order_id, 'placed' AS status),\n"
        "__expected__order_summary AS (\n"
        "  SELECT 1 order_id, 'placed' IS DISTINCT FROM 'shipped'\n"
        ")\n"
    ),
}
_TESTS: tuple[str, ...] = (
    "test_tidy_label",
    "test_order_label",
    "test_order_rows",
    "test_order_summary",
)


_PASSING_FRAGMENTS: tuple[str, ...] = ("PASS=4  FAIL=0  TOTAL=4",)
_LOCATED_MODEL_NAME_FRAGMENTS: tuple[str, ...] = (
    "error[P001]: SQL test 'tests/unit/test_order_summary.sql' CTE name 'café' must be an "
    "unquoted identifier",
    "--> tests/unit/test_order_summary.sql:5:6",
)
_LOCATED_CASE_NAME_FRAGMENTS: tuple[str, ...] = (
    "error[P001]: SQL test 'tests/unit/test_tidy_label.sql' CTE name 'a$b' must be an "
    "unquoted identifier",
    "--> tests/unit/test_tidy_label.sql:9:3",
)
_REPEATED_MALFORMED_SOURCE_FRAGMENTS: tuple[str, ...] = (
    "tests/unit/test_order_label.sql:4:37",
    "tests/unit/test_order_label.sql:6:63",
)
_QUOTED_NAME_FRAGMENTS: tuple[str, ...] = (
    "error[P001]: SQL test 'tests/unit/test_order_label.sql' CTE name "
    '"input values" must be an unquoted identifier',
    "--> tests/unit/test_order_label.sql:4:1",
    "help: rename the CTE, for example input_values",
)
_HELPER_CALLS_UDF_FRAGMENTS: tuple[str, ...] = (
    "mode 'macro' helper CTE 'input_values' must not call udf; call reusable "
    "logic only in __macro_actual__",
)
_MALFORMED_SOURCE_FRAGMENTS: tuple[str, ...] = (
    "error[P012]:",
    "tests/unit/test_order_label.sql:4:37",
    '__source("raw", "orders") is not a valid __source() call',
)

_UNALIASED_DISTINCT_FROM_FRAGMENTS: tuple[str, ...] = (
    "SQL test 'tests/unit/test_order_summary.sql' must alias every non-trivial "
    "__expected__order_summary projection",
    "for example 'placed' IS DISTINCT FROM 'shipped' AS <name>",
)


@pytest.mark.parametrize(
    "test_case",
    [
        DirectLogicExtractionE2ETestCase(
            description="macro, UDF and table-function tests pass on native",
            overrides={},
            expected_exit_code=0,
            expected_output_fragments=_PASSING_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="a quoted CTE name is a located error on native",
            overrides=_QUOTED_NAME,
            expected_exit_code=1,
            expected_output_fragments=_QUOTED_NAME_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="a macro-test helper calling a UDF is rejected on native",
            overrides=_HELPER_CALLS_UDF,
            expected_exit_code=1,
            expected_output_fragments=_HELPER_CALLS_UDF_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="a malformed helper reference is P012 at its call on native",
            overrides=_MALFORMED_SOURCE,
            expected_exit_code=1,
            expected_output_fragments=_MALFORMED_SOURCE_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="a CTE name in an indented model test is located exactly on native",
            overrides=_LOCATED_MODEL_NAME,
            expected_exit_code=1,
            expected_output_fragments=_LOCATED_MODEL_NAME_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="a CTE name in a test with cases is located exactly on native",
            overrides=_LOCATED_CASE_NAME,
            expected_exit_code=1,
            expected_output_fragments=_LOCATED_CASE_NAME_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="each repeated malformed reference is P012 at its own call on native",
            overrides=_REPEATED_MALFORMED_SOURCE,
            expected_exit_code=1,
            expected_output_fragments=_REPEATED_MALFORMED_SOURCE_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="aliased IS [NOT] DISTINCT FROM and list projections pass on native",
            overrides=_ALIASED_OPERATORS,
            expected_exit_code=0,
            expected_output_fragments=_PASSING_FRAGMENTS,
        ),
        DirectLogicExtractionE2ETestCase(
            description="an unaliased IS DISTINCT FROM projection is rejected on native",
            overrides=_UNALIASED_DISTINCT_FROM,
            expected_exit_code=1,
            expected_output_fragments=_UNALIASED_DISTINCT_FROM_FRAGMENTS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_direct_logic_tests_when_testing_then_extraction_rules_hold(
    test_case: DirectLogicExtractionE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="direct_logic",
        repo_files={**_PROJECT_FILES, **test_case.overrides},
    )

    _ = run_sqb(command=("--no-color", "build", "--no-tests"), project_dir=project_dir)
    tested: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--select", *_TESTS), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert (
        tested.returncode,
        tuple(fragment in output for fragment in test_case.expected_output_fragments),
    ) == (
        test_case.expected_exit_code,
        tuple(True for _ in test_case.expected_output_fragments),
    ), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
