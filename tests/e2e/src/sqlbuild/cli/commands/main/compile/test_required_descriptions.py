"""E2E coverage of the P010 compile error for named resources without a description."""

from __future__ import annotations

import json
import subprocess
from dataclasses import asdict
from itertools import chain
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FormatDescriptionResolutionE2ECase,
    PathDefaultDescriptionCase,
    RequiredDescriptionAggregateCase,
    RequiredDescriptionCase,
    RequiredDescriptionExactOutputCase,
    RequiredDescriptionPlanCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    REQUIRED_DESCRIPTIONS_PROJECT,
    compile_inline_files,
    hooked_model_file,
    python_node_source,
    require_sql_analysis_output,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

DESCRIPTION_REQUIRED_INPUT_KINDS: dict[str, str] = {
    "model_files": "model",
    "scenario_files": "scenario",
    "seed_files": "seed",
    "source_files": "source",
    "sql_function_files": "function",
    "python_function_files": "function",
    "sql_hook_files": "hook",
    "hook_functions": "hook",
    "loader_functions": "loader",
    "task_functions": "task",
    "asset_functions": "asset",
    "check_functions": "check",
    "providers": "provider",
}

_SEED_CSV: str = "product_type_id,label\n1,waffle\n"
_SEED_COLUMNS: str = (
    "    columns:\n      - name: product_type_id\n        type: INTEGER\n"
    "      - name: label\n        type: VARCHAR\n"
)
_ORDER_TOTALS_MODEL: tuple[str, str] = (
    "models/order_totals.sql",
    'MODEL (description "Order count");\n\nSELECT COUNT(*) AS order_count FROM __ref("orders")\n',
)
_SCENARIO_BODY: str = (
    "WITH\n__ref__orders AS (\n  SELECT 1 AS order_id\n),\n"
    "__expected__order_totals AS (\n  SELECT 1 AS order_count\n)\nSELECT 1\n"
)


_SHADOWED_SOURCES_YAML: str = (
    "sources:\n"
    "  - name: raw_orders\n"
    "    schema: main\n"
    "    table: orders\n"
    "    columns:\n"
    "      - name: raw_refunds\n"
    "        type: INTEGER\n"
    '  - name: "raw_refunds"  # refunds feed\n'
    "    schema: main\n"
    "    table: refunds\n"
    "  - name: 'raw_customers'\n"
    "    schema: main\n"
    "    table: customers\n"
)
_TWO_SEEDS_YAML: str = (
    "seeds:\n"
    "  - name: product_types\n" + _SEED_COLUMNS + "  - name: product_labels\n" + _SEED_COLUMNS
)


_PROVIDER_DESCRIBED: str = (
    "from sqlbuild.providers import Provider\n\n\n"
    "class OrdersApi(Provider):\n"
    '    """Client for the orders API."""\n\n'
    '    base_url: str = "https://example.com"\n'
)
_PROVIDER_UNDESCRIBED: str = (
    "from sqlbuild.providers import Provider\n\n\n"
    "class OrdersApi(Provider):\n"
    '    base_url: str = "https://example.com"\n'
)


_CASES: tuple[RequiredDescriptionCase, ...] = (
    RequiredDescriptionCase(
        description="model header",
        missing_files=(
            ("models/order_totals.sql", "MODEL (materialized table);\nSELECT 1 AS id\n"),
        ),
        described_files=(
            (
                "models/order_totals.sql",
                'MODEL (description "Order totals", materialized table);\nSELECT 1 AS id\n',
            ),
        ),
        expected_diagnostic=("P010", "models/order_totals.sql", 1),
        expected_message="model 'order_totals' has no description",
        expected_help_fragments=(
            "to describe model 'order_totals', add this to the MODEL header in "
            "models/order_totals.sql:\n"
            "            MODEL (\n"
            '              description "What one row of order_totals represents",\n'
            "              ...\n"
            "            );",
        ),
    ),
    RequiredDescriptionCase(
        description="scenario header",
        missing_files=(
            (
                "tests/scenarios/orders__single_order.sql",
                "SCENARIO ();\n\n" + _SCENARIO_BODY,
            ),
            _ORDER_TOTALS_MODEL,
        ),
        described_files=(
            (
                "tests/scenarios/orders__single_order.sql",
                'SCENARIO (description "One order produces one row");\n\n' + _SCENARIO_BODY,
            ),
            _ORDER_TOTALS_MODEL,
        ),
        expected_diagnostic=("P010", "tests/scenarios/orders__single_order.sql", 1),
        expected_message="scenario 'orders__single_order' has no description",
        expected_help_fragments=(
            "add this to the SCENARIO header in tests/scenarios/orders__single_order.sql:\n"
            "            SCENARIO (\n"
            '              description "The behaviour orders__single_order proves",',
        ),
    ),
    RequiredDescriptionCase(
        description="seed YAML entry",
        missing_files=(
            ("seeds/product_types.csv", _SEED_CSV),
            ("seeds/product_types.yml", "seeds:\n  - name: product_types\n" + _SEED_COLUMNS),
        ),
        described_files=(
            ("seeds/product_types.csv", _SEED_CSV),
            (
                "seeds/product_types.yml",
                "seeds:\n  - name: product_types\n    description: Product type catalogue\n"
                + _SEED_COLUMNS,
            ),
        ),
        expected_diagnostic=("P010", "seeds/product_types.yml", 2),
        expected_message="seed 'product_types' has no description",
        expected_help_fragments=(
            "add `description` to its entry in seeds/product_types.yml:\n"
            "            - name: product_types\n"
            "              description: What one row of product_types represents",
        ),
    ),
    RequiredDescriptionCase(
        description="source YAML entry",
        missing_files=(
            (
                "sources/raw.yml",
                "sources:\n  - name: raw_customers\n    schema: main\n    table: customers\n",
            ),
        ),
        described_files=(
            (
                "sources/raw.yml",
                "sources:\n  - name: raw_customers\n    description: Customer records\n"
                "    schema: main\n    table: customers\n",
            ),
        ),
        expected_diagnostic=("P010", "sources/raw.yml", 2),
        expected_message="source 'raw_customers' has no description",
        expected_help_fragments=(
            "add `description` to its entry in sources/raw.yml:\n"
            "            - name: raw_customers\n",
        ),
    ),
    RequiredDescriptionCase(
        description="managed source described by its loader docstring",
        missing_files=(
            ("sources/raw.yml", "sources:\n  - name: raw_customers\n    managed: true\n"),
            (
                "python/loaders/raw_customers.py",
                python_node_source(module="loaders", decorator="loader", name="raw_customers"),
            ),
        ),
        described_files=(
            ("sources/raw.yml", "sources:\n  - name: raw_customers\n    managed: true\n"),
            (
                "python/loaders/raw_customers.py",
                python_node_source(
                    module="loaders",
                    decorator="loader",
                    name="raw_customers",
                    docstring_line='    """Customer records from the CRM export."""\n',
                ),
            ),
        ),
        expected_diagnostic=("P010", "sources/raw.yml", 2),
        expected_message="source 'raw_customers' has no description",
        expected_help_fragments=(
            "or describe its loader 'raw_customers' in python/loaders/raw_customers.py with a "
            'docstring or @loader(description="What one row of raw_customers represents")',
        ),
    ),
    RequiredDescriptionCase(
        description="SQL function header",
        missing_files=(
            (
                "functions/sql/is_large_order.sql",
                "FUNCTION (\n  arguments (amount INTEGER),\n  returns BOOLEAN,\n);\n\n"
                "amount > 100\n",
            ),
        ),
        described_files=(
            (
                "functions/sql/is_large_order.sql",
                'FUNCTION (\n  description "Whether an order amount is large",\n'
                "  arguments (amount INTEGER),\n  returns BOOLEAN,\n);\n\namount > 100\n",
            ),
        ),
        expected_diagnostic=("P010", "functions/sql/is_large_order.sql", 1),
        expected_message="function 'is_large_order' has no description",
        expected_help_fragments=(
            "add this to the FUNCTION header in functions/sql/is_large_order.sql:\n"
            "            FUNCTION (\n"
            '              description "What is_large_order returns",',
        ),
    ),
    RequiredDescriptionCase(
        description="table function header",
        missing_files=(
            (
                "functions/sql/order_lines.sql",
                "FUNCTION (\n  arguments (p_order_id INTEGER),\n"
                "  returns table (order_id INTEGER),\n);\n\nSELECT p_order_id AS order_id\n",
            ),
        ),
        described_files=(
            (
                "functions/sql/order_lines.sql",
                'FUNCTION (\n  description "Lines of one order",\n'
                "  arguments (p_order_id INTEGER),\n"
                "  returns table (order_id INTEGER),\n);\n\nSELECT p_order_id AS order_id\n",
            ),
        ),
        expected_diagnostic=("P010", "functions/sql/order_lines.sql", 1),
        expected_message="function 'order_lines' has no description",
        expected_help_fragments=('description "What order_lines returns"',),
    ),
    RequiredDescriptionCase(
        description="Python UDF described by its docstring",
        missing_files=(
            (
                "functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN", runtime_version="3.11")\n'
                "def main(amount: int) -> bool:\n    return amount > 100\n",
            ),
        ),
        described_files=(
            (
                "functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN", runtime_version="3.11")\n'
                "def main(amount: int) -> bool:\n"
                '    """Whether an order amount is large."""\n'
                "    return amount > 100\n",
            ),
        ),
        expected_diagnostic=("P010", "functions/python/is_large_order_py.py", 4),
        expected_message="function 'is_large_order_py' has no description",
        expected_help_fragments=(
            "give its function in functions/python/is_large_order_py.py a docstring or add "
            "this to @udf:\n"
            '            @udf(description="What is_large_order_py returns", ...)',
        ),
    ),
    RequiredDescriptionCase(
        description="Python UDF described by its decorator",
        missing_files=(
            (
                "functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN", runtime_version="3.11")\n'
                "def main(amount: int) -> bool:\n    return amount > 100\n",
            ),
        ),
        described_files=(
            (
                "functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                "@udf(\n"
                '    description="Whether an order amount is large",\n'
                '    arguments={"amount": "INTEGER"},\n'
                '    returns="BOOLEAN",\n'
                '    runtime_version="3.11",\n'
                ")\n"
                "def main(amount: int) -> bool:\n    return amount > 100\n",
            ),
        ),
        expected_diagnostic=("P010", "functions/python/is_large_order_py.py", 4),
        expected_message="function 'is_large_order_py' has no description",
        expected_help_fragments=(),
    ),
    RequiredDescriptionCase(
        description="named SQL hook header",
        missing_files=(
            ("hooks/sql/analyze_orders.sql", "HOOK ();\n\nSELECT 1\n"),
            hooked_model_file(model_name="analyzed_orders", hook='sql("analyze_orders")'),
        ),
        described_files=(
            (
                "hooks/sql/analyze_orders.sql",
                'HOOK (description "What analyze_orders does");\n\nSELECT 1\n',
            ),
            hooked_model_file(model_name="analyzed_orders", hook='sql("analyze_orders")'),
        ),
        expected_diagnostic=("P010", "hooks/sql/analyze_orders.sql", 1),
        expected_message="hook 'analyze_orders' has no description",
        expected_help_fragments=(
            "add this to the HOOK header in hooks/sql/analyze_orders.sql:\n"
            '            HOOK (description "What analyze_orders does");',
        ),
    ),
    RequiredDescriptionCase(
        description="Python hook",
        missing_files=(
            (
                "hooks/python/notify_orders.py",
                python_node_source(module="hooks", decorator="hook", name="notify_orders"),
            ),
            hooked_model_file(model_name="notified_orders", hook='python("notify_orders")'),
        ),
        described_files=(
            (
                "hooks/python/notify_orders.py",
                python_node_source(
                    module="hooks",
                    decorator="hook",
                    name="notify_orders",
                    docstring_line='    """Notify the orders channel."""\n',
                ),
            ),
            hooked_model_file(model_name="notified_orders", hook='python("notify_orders")'),
        ),
        expected_diagnostic=("P010", "hooks/python/notify_orders.py", 4),
        expected_message="hook 'notify_orders' has no description",
        expected_help_fragments=(
            "give its function in hooks/python/notify_orders.py a docstring or pass a "
            "description to @hook:\n"
            '            @hook(description="What notify_orders does", ...)',
        ),
    ),
    RequiredDescriptionCase(
        description="loader without a source declaration",
        missing_files=(
            (
                "python/loaders/raw_returns.py",
                python_node_source(module="loaders", decorator="loader", name="raw_returns"),
            ),
        ),
        described_files=(
            (
                "python/loaders/raw_returns.py",
                python_node_source(
                    module="loaders",
                    decorator="loader",
                    name="raw_returns",
                    docstring_line='    """Returned orders from the support desk."""\n',
                ),
            ),
        ),
        expected_diagnostic=("P010", "python/loaders/raw_returns.py", 4),
        expected_message="loader 'raw_returns' has no description",
        expected_help_fragments=('@loader(description="What raw_returns does", ...)',),
    ),
    RequiredDescriptionCase(
        description="task",
        missing_files=(
            (
                "python/tasks/refresh_exports.py",
                python_node_source(module="tasks", decorator="task", name="refresh_exports"),
            ),
        ),
        described_files=(
            (
                "python/tasks/refresh_exports.py",
                python_node_source(
                    module="tasks",
                    decorator="task",
                    name="refresh_exports",
                    docstring_line='    """Refresh the order exports."""\n',
                ),
            ),
        ),
        expected_diagnostic=("P010", "python/tasks/refresh_exports.py", 4),
        expected_message="task 'refresh_exports' has no description",
        expected_help_fragments=('@task(description="What refresh_exports does", ...)',),
    ),
    RequiredDescriptionCase(
        description="asset",
        missing_files=(
            (
                "python/assets/order_report.py",
                python_node_source(module="assets", decorator="asset", name="order_report"),
            ),
        ),
        described_files=(
            (
                "python/assets/order_report.py",
                python_node_source(
                    module="assets",
                    decorator="asset",
                    name="order_report",
                    docstring_line='    """Weekly order report file."""\n',
                ),
            ),
        ),
        expected_diagnostic=("P010", "python/assets/order_report.py", 4),
        expected_message="asset 'order_report' has no description",
        expected_help_fragments=('@asset(description="What order_report does", ...)',),
    ),
    RequiredDescriptionCase(
        description="check",
        missing_files=(
            (
                "python/checks/orders_present.py",
                python_node_source(
                    module="checks",
                    decorator="check",
                    name="orders_present",
                    extra_import="from sqlbuild.refs import model\n",
                    decorator_arguments='(depends_on=model("orders"))',
                ),
            ),
        ),
        described_files=(
            (
                "python/checks/orders_present.py",
                python_node_source(
                    module="checks",
                    decorator="check",
                    name="orders_present",
                    extra_import="from sqlbuild.refs import model\n",
                    decorator_arguments='(depends_on=model("orders"))',
                    docstring_line='    """Orders exist after the build."""\n',
                ),
            ),
        ),
        expected_diagnostic=("P010", "python/checks/orders_present.py", 5),
        expected_message="check 'orders_present' has no description",
        expected_help_fragments=('@check(description="What orders_present does", ...)',),
    ),
    RequiredDescriptionCase(
        description="provider",
        missing_files=(("providers/orders_api.py", _PROVIDER_UNDESCRIBED),),
        described_files=(("providers/orders_api.py", _PROVIDER_DESCRIBED),),
        expected_diagnostic=("P010", "providers/orders_api.py", 4),
        expected_message="provider 'orders_api' has no description",
        expected_help_fragments=(
            "add a class docstring in providers/orders_api.py:\n"
            "            class OrdersApi(Provider):\n"
            '                """What orders_api provides."""',
        ),
    ),
)


_PATH_DEFAULT_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n\n'
    '[path_defaults.marts]\ndescription = "Mart model"\n'
)
_OWN_DESCRIPTION_MODEL: str = 'MODEL (description "Own description");\n\nSELECT 1 AS id\n'
_INHERITING_MODEL: str = "MODEL ();\n\nSELECT 1 AS id\n"


_TYPED_NULL_FIXTURE_FILES: tuple[tuple[str, str], ...] = (
    ("models/up.sql", 'MODEL (description "Upstream rows");\n\nSELECT 1 AS a\n'),
    (
        "models/marts/a_model.sql",
        'MODEL (description "Own description");\n\nSELECT a\nFROM __ref("up")\n',
    ),
    ("models/marts/b_model.sql", _INHERITING_MODEL),
    (
        "tests/unit/test_a_model.sql",
        'TEST (description "Null values pass through");\n\n'
        "WITH __ref__up AS (\n  SELECT CAST(NULL AS INTEGER) AS a\n),\n\n"
        "__expected__a_model AS (\n  SELECT CAST(NULL AS INTEGER) AS a\n)\n\nSELECT 1\n",
    ),
)


_ALL_UNDESCRIBED_FILES: tuple[tuple[str, str], ...] = (
    *dict(chain.from_iterable(case.missing_files for case in _CASES)).items(),
    ("models/undescribed_orders.sql", "MODEL (materialized view);\n\nSELECT 1 AS id\n"),
)


def _paragraphs(count: int) -> str:
    """A docstring of `count` one-line paragraphs, which wrap to 2 * count - 1 lines."""

    body: str = "\n\n    ".join(f"Orders paragraph {index}." for index in range(count))
    return f'    """{body}"""\n'


# The default max_description_lines is 10: five paragraphs fit in 9 lines, six need 11.
_LENGTH_CASES: tuple[RequiredDescriptionCase, ...] = (
    RequiredDescriptionCase(
        description="task docstring",
        missing_files=(
            (
                "python/tasks/refresh_exports.py",
                python_node_source(
                    module="tasks",
                    decorator="task",
                    name="refresh_exports",
                    docstring_line=_paragraphs(6),
                ),
            ),
        ),
        described_files=(
            (
                "python/tasks/refresh_exports.py",
                python_node_source(
                    module="tasks",
                    decorator="task",
                    name="refresh_exports",
                    docstring_line=_paragraphs(5),
                ),
            ),
        ),
        expected_diagnostic=("P027", "python/tasks/refresh_exports.py", 4),
        expected_message="task 'refresh_exports' description exceeds 10 lines",
        expected_help_fragments=("Shorten the description to 10 lines",),
    ),
    RequiredDescriptionCase(
        description="asset description argument",
        missing_files=(
            (
                "python/assets/order_report.py",
                python_node_source(
                    module="assets",
                    decorator="asset",
                    name="order_report",
                    decorator_arguments=f"(description={'Order report. ' * 90!r})",
                ),
            ),
        ),
        described_files=(
            (
                "python/assets/order_report.py",
                python_node_source(
                    module="assets",
                    decorator="asset",
                    name="order_report",
                    decorator_arguments=f"(description={'Order report. ' * 60!r})",
                ),
            ),
        ),
        expected_diagnostic=("P027", "python/assets/order_report.py", 4),
        expected_message="asset 'order_report' description exceeds 10 lines",
        expected_help_fragments=("Shorten the description to 10 lines",),
    ),
)


@pytest.mark.parametrize("test_case", _LENGTH_CASES, ids=lambda case: case.description)
def test_given_python_description_past_the_line_limit_when_compiling_then_reports_p027(
    test_case: RequiredDescriptionCase, tmp_path: Path
) -> None:
    too_long: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path / "too_long", files=test_case.missing_files
    )
    at_limit: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path / "at_limit", files=test_case.described_files
    )

    diagnostics, text = require_sql_analysis_output(too_long)
    assert too_long.returncode == 1, too_long.stdout + too_long.stderr
    assert diagnostics == (test_case.expected_diagnostic,)
    assert json.loads(too_long.stdout)["diagnostics"][0]["message"] == test_case.expected_message
    assert all(fragment in text for fragment in test_case.expected_help_fragments), text
    assert at_limit.returncode == 0, at_limit.stdout + at_limit.stderr
    assert require_sql_analysis_output(at_limit)[0] == ()


@pytest.mark.parametrize(
    "test_case",
    [RequiredDescriptionCase(**asdict(case)) for case in _CASES],
    ids=lambda case: case.description,
)
def test_given_resource_without_description_when_compiling_then_reports_p010_with_snippet(
    test_case: RequiredDescriptionCase, tmp_path: Path
) -> None:
    result: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path, files=test_case.missing_files
    )

    diagnostics, text = require_sql_analysis_output(result)
    payload: dict[str, Any] = json.loads(result.stdout)
    assert result.returncode == 1, result.stdout + result.stderr
    assert diagnostics == (test_case.expected_diagnostic,)
    assert payload["diagnostics"][0]["message"] == test_case.expected_message
    assert all(fragment in text for fragment in test_case.expected_help_fragments), text


@pytest.mark.parametrize(
    "test_case",
    [RequiredDescriptionCase(**asdict(case)) for case in _CASES],
    ids=lambda case: case.description,
)
def test_given_described_resource_when_compiling_then_compiles_cleanly(
    test_case: RequiredDescriptionCase, tmp_path: Path
) -> None:
    result: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path, files=test_case.described_files
    )

    diagnostics, text = require_sql_analysis_output(result)
    assert result.returncode == 0, result.stdout + result.stderr
    assert diagnostics == ()
    assert test_case.expected_message not in text


@pytest.mark.parametrize(
    "test_case",
    [
        RequiredDescriptionAggregateCase(
            description="every required kind undescribed in one project",
            files=_ALL_UNDESCRIBED_FILES,
            expected_codes=frozenset({"P010"}),
            expected_kinds=frozenset(DESCRIPTION_REQUIRED_INPUT_KINDS.values()),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_every_resource_kind_undescribed_when_compiling_then_one_run_reports_each(
    test_case: RequiredDescriptionAggregateCase, tmp_path: Path
) -> None:
    result: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path, files=test_case.files
    )

    payload: dict[str, Any] = json.loads(result.stdout)
    codes: set[str] = {item["code"] for item in payload["diagnostics"]}
    kinds: set[str] = {item["message"].split(" ", 1)[0] for item in payload["diagnostics"]}
    assert result.returncode == 1, result.stdout + result.stderr
    assert codes == test_case.expected_codes
    assert kinds == test_case.expected_kinds


@pytest.mark.parametrize(
    "test_case",
    [
        RequiredDescriptionExactOutputCase(
            description="seed and source entries located at their outermost declarations",
            files=(
                ("models/order_summary.sql", "MODEL ();\nSELECT 1 AS order_count\n"),
                ("seeds/product_types.csv", _SEED_CSV),
                ("seeds/product_labels.csv", _SEED_CSV),
                ("seeds/products.yml", _TWO_SEEDS_YAML),
                ("sources/raw.yml", _SHADOWED_SOURCES_YAML),
            ),
            expected_diagnostics=(
                (
                    "P010",
                    "model 'order_summary' has no description",
                    "models/order_summary.sql",
                    1,
                    1,
                ),
                ("P010", "seed 'product_types' has no description", "seeds/products.yml", 2, 1),
                ("P010", "seed 'product_labels' has no description", "seeds/products.yml", 8, 1),
                ("P010", "source 'raw_orders' has no description", "sources/raw.yml", 2, 1),
                ("P010", "source 'raw_refunds' has no description", "sources/raw.yml", 8, 1),
                ("P010", "source 'raw_customers' has no description", "sources/raw.yml", 11, 1),
            ),
            expected_help=(
                "to describe model 'order_summary', add this to the MODEL header in "
                "models/order_summary.sql:\n"
                "            MODEL (\n"
                '              description "What one row of order_summary represents",\n'
                "              ...\n"
                "            );",
                (
                    "to describe seed 'product_types', add `description` to its entry in seeds/products.yml:\n"
                    "            - name: product_types\n"
                    "              description: What one row of product_types represents"
                ),
                (
                    "to describe seed 'product_labels', add `description` to its entry in seeds/products.yml:\n"
                    "            - name: product_labels\n"
                    "              description: What one row of product_labels represents"
                ),
                (
                    "to describe source 'raw_orders', add `description` to its entry in sources/raw.yml:\n"
                    "            - name: raw_orders\n"
                    "              description: What one row of raw_orders represents"
                ),
                (
                    "to describe source 'raw_refunds', add `description` to its entry in sources/raw.yml:\n"
                    "            - name: raw_refunds\n"
                    "              description: What one row of raw_refunds represents"
                ),
                (
                    "to describe source 'raw_customers', add `description` to its entry in sources/raw.yml:\n"
                    "            - name: raw_customers\n"
                    "              description: What one row of raw_customers represents"
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_undescribed_yaml_entries_when_compiling_then_reports_exact_ordered_diagnostics(
    test_case: RequiredDescriptionExactOutputCase, tmp_path: Path
) -> None:
    result: subprocess.CompletedProcess[str] = compile_inline_files(
        tmp_path=tmp_path, files=test_case.files
    )

    payload: dict[str, Any] = json.loads(result.stdout)
    diagnostics: list[dict[str, Any]] = payload["diagnostics"]
    assert result.returncode == 1, result.stdout + result.stderr
    assert (
        tuple(
            (item["code"], item["message"], item["path"], item["line"], item["column"])
            for item in diagnostics
        )
        == test_case.expected_diagnostics
    )
    assert tuple(item["help"] for item in diagnostics) == test_case.expected_help


@pytest.mark.parametrize(
    "test_case",
    [
        RequiredDescriptionPlanCase(
            description="undescribed model blocks plan",
            model_sql="MODEL (materialized table);\n\nSELECT 1 AS order_id\n",
            expected_returncode=1,
            expected_stderr_fragment=(
                "[P010] models/orders.sql:1:1: model 'orders' has no description"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_undescribed_model_when_planning_then_plan_is_blocked(
    test_case: RequiredDescriptionPlanCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": REQUIRED_DESCRIPTIONS_PROJECT,
            "models/orders.sql": test_case.model_sql,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "plan")
    )

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        PathDefaultDescriptionCase(
            description="described model sorts before the inheriting model",
            model_files=(
                ("models/marts/a_model.sql", _OWN_DESCRIPTION_MODEL),
                ("models/marts/b_model.sql", _INHERITING_MODEL),
            ),
            expected_returncode=0,
            expected_diagnostics=(),
            expected_descriptions=(("a_model", "Own description"), ("b_model", "Mart model")),
        ),
        PathDefaultDescriptionCase(
            description="inheriting model sorts before the described model",
            model_files=(
                ("models/marts/a_model.sql", _INHERITING_MODEL),
                ("models/marts/b_model.sql", _OWN_DESCRIPTION_MODEL),
            ),
            expected_returncode=0,
            expected_diagnostics=(),
            expected_descriptions=(("a_model", "Mart model"), ("b_model", "Own description")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_path_default_description_when_compiling_then_every_model_keeps_its_description(
    test_case: PathDefaultDescriptionCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={"sqlbuild_project.toml": _PATH_DEFAULT_PROJECT, **dict(test_case.model_files)},
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )
    dag: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "dag", "--json")
    )

    nodes: list[dict[str, Any]] = json.loads(dag.stdout)["nodes"]
    assert compiled.returncode == test_case.expected_returncode, compiled.stdout + compiled.stderr
    assert tuple(json.loads(compiled.stdout)["diagnostics"]) == test_case.expected_diagnostics
    assert dag.returncode == test_case.expected_returncode, dag.stdout + dag.stderr
    assert sorted((node["name"], node["description"]) for node in nodes) == list(
        test_case.expected_descriptions
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FormatDescriptionResolutionE2ECase(
            description="typed-null fixture tests do not hide a path-default description",
            extra_files=(),
            expected_returncode=0,
            expected_faults=(),
        ),
        FormatDescriptionResolutionE2ECase(
            description="a genuinely missing description is still reported",
            extra_files=(("models/bare.sql", "MODEL (materialized table);\n\nSELECT 1 AS id\n"),),
            expected_returncode=1,
            expected_faults=(("models/bare.sql", "description-present"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inherited_descriptions_when_format_checking_then_faults_match_compile(
    test_case: FormatDescriptionResolutionE2ECase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _PATH_DEFAULT_PROJECT,
            **dict(_TYPED_NULL_FIXTURE_FILES),
            **dict(test_case.extra_files),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--check", "--json")
    )

    violations: list[dict[str, Any]] = json.loads(result.stdout)["violations"]
    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert (
        tuple(
            (Path(item["file"]).relative_to(project_dir).as_posix(), item["code"])
            for item in violations
        )
        == test_case.expected_faults
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
