"""Helpers for source loader e2e tests."""

from __future__ import annotations

import json
import sqlite3
import subprocess
import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import ClassVar, cast

from tests.e2e.src.sqlbuild.cli.commands.main.load._test_types import (
    SourceOnlyIngressDependencyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


def assert_source_only_ingress_dependency_case(
    *, tmp_path: Path, test_case: SourceOnlyIngressDependencyE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files={
            **build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n  - name: raw_events\n    description: Test source raw_events.\n"
                    "    managed: true\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n"
                    "from python.nodes.tasks.prepare.asset import prepare_events\n\n"
                    "@loader(depends_on=[prepare_events], write_strategy='table', columns=[\n"
                    "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                    "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                    "])\n"
                    "def fetch_events(ctx):\n"
                    "    '''Test loader fetch_events.'''\n    return [\n"
                    "        {'event_id': 1, 'load_seq': 1},\n"
                    "        {'event_id': 2, 'load_seq': 1},\n"
                    "    ]\n\n"
                    "@loader(depends_on=[fetch_events])\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                    "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                    "SELECT event_id FROM {events.destination}')\n"
                ),
            ),
            "python/nodes/tasks/prepare/asset.py": (
                "from pathlib import Path\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def prepare_events(ctx):\n"
                "    '''Test task prepare_events.'''\n"
                "    Path(__file__).parents[2].joinpath('prepared.txt').write_text('prepared')\n"
                "    return ctx.result()\n"
            ),
        },
    )
    setup_strategy: Callable[..., None] = {
        False: _skip_source_only_setup,
        True: _run_source_only_setup,
    }[test_case.setup_command is not None]
    setup_strategy(
        project_dir=project_dir,
        setup_command=test_case.setup_command,
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    combined_output: str = result.stdout + result.stderr
    error_strategy: Callable[..., None] = {
        False: _skip_expected_error_assertion,
        True: _assert_expected_error,
    }[test_case.expected_error_fragment is not None]
    error_strategy(
        combined_output=combined_output,
        expected_error_fragment=test_case.expected_error_fragment,
    )
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    result_strategy: Callable[..., None] = {
        False: _assert_successful_source_only_result,
        True: _skip_successful_source_only_assertions,
    }[test_case.expected_return_code != 0]
    result_strategy(
        project_dir=project_dir,
        db_path=db_path,
        result=result,
        test_case=test_case,
    )


def _skip_source_only_setup(**_kwargs: object) -> None:
    return


def _run_source_only_setup(*, project_dir: Path, setup_command: tuple[str, ...] | None) -> None:
    setup_result: subprocess.CompletedProcess[str] = run_sqb(
        command=cast(tuple[str, ...], setup_command),
        project_dir=project_dir,
    )
    assert setup_result.returncode == 0, setup_result.stdout + setup_result.stderr


def _skip_expected_error_assertion(**_kwargs: object) -> None:
    return


def _assert_expected_error(*, combined_output: str, expected_error_fragment: str | None) -> None:
    assert cast(str, expected_error_fragment) in combined_output


def _skip_successful_source_only_assertions(**_kwargs: object) -> None:
    return


def _assert_successful_source_only_result(
    *,
    project_dir: Path,
    db_path: Path,
    result: subprocess.CompletedProcess[str],
    test_case: SourceOnlyIngressDependencyE2ETestCase,
) -> None:
    assert ("loader    fetch_events" in result.stdout) is (
        "loader    fetch_events" in test_case.expected_stdout_fragments
    )
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, load_seq FROM __loader__fetch_events ORDER BY event_id",
    )
    terminal_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM raw_events ORDER BY event_id",
    )
    assert tuple(intermediate_rows) == test_case.expected_intermediate_rows
    assert tuple(terminal_rows) == test_case.expected_terminal_rows
    assert (project_dir / "prepared.txt").exists() is test_case.expected_marker_exists


def build_schema_behavior_project_files(*, source_yaml: str, loader_py: str) -> dict[str, str]:
    return {
        "sqlbuild_project.toml": (
            'name = "source_loader_schema_behavior"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "source_loader_schema_behavior.duckdb"\n'
        ),
        "sql/sources/raw.yml": source_yaml,
        "python/nodes/loaders/source_rows.py": loader_py,
    }


def write_sqlite_orders_source_database(db_path: Path) -> None:
    connection: sqlite3.Connection = sqlite3.connect(str(db_path))
    try:
        connection.execute("CREATE TABLE orders (order_id INTEGER, amount INTEGER)")
        connection.execute("INSERT INTO orders VALUES (1, 10), (2, 20), (3, 30)")
        connection.commit()
    finally:
        connection.close()


_ORDERS_PAYLOAD: bytes = json.dumps(
    [
        {"order_id": 1, "amount": 10},
        {"order_id": 2, "amount": 20},
        {"order_id": 3, "amount": 30},
    ]
).encode("utf-8")


class _OrdersHandler(BaseHTTPRequestHandler):
    responses: ClassVar[dict[str, tuple[int, tuple[tuple[str, str], ...], bytes]]] = {
        "/orders": (
            200,
            (
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(_ORDERS_PAYLOAD))),
            ),
            _ORDERS_PAYLOAD,
        )
    }
    not_found_response: ClassVar[tuple[int, tuple[tuple[str, str], ...], bytes]] = (404, (), b"")

    def do_GET(self) -> None:
        status, headers, payload = self.responses.get(self.path, self.not_found_response)
        self.send_response(status)
        for name, value in headers:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format: str, *args: object) -> None:
        return None


@contextmanager
def serve_orders_api() -> Iterator[str]:

    server: ThreadingHTTPServer = ThreadingHTTPServer(("127.0.0.1", 0), _OrdersHandler)
    thread: threading.Thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = cast(tuple[str, int], server.server_address)
        yield f"http://{host}:{port}"
    finally:
        server.shutdown()
        server.server_close()


def build_loader_waffle_shop_project_files(*, project_toml: str | None = None) -> dict[str, str]:
    """Build a compact project that stresses chained source loaders and models."""

    return {
        "sqlbuild_project.toml": project_toml
        or (
            'name = "loader_waffle_shop"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "loader_waffle_shop.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sql/sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    managed: true\n"
            "    write_strategy: table\n"
            "    columns:\n"
            "      - name: order_id\n"
            "        type: INTEGER\n"
            "      - name: customer_id\n"
            "        type: INTEGER\n"
            "      - name: waffle_type\n"
            "        type: VARCHAR\n"
            "      - name: quantity\n"
            "        type: INTEGER\n"
            "      - name: price_cents\n"
            "        type: INTEGER\n"
            "      - name: load_seq\n"
            "        type: INTEGER\n"
            "  - name: raw_customers\n    description: Test source raw_customers.\n"
            "    managed: true\n"
            "    write_strategy: table\n"
            "    columns:\n"
            "      - name: customer_id\n"
            "        type: INTEGER\n"
            "      - name: plan_name\n"
            "        type: VARCHAR\n"
            "      - name: load_seq\n"
            "        type: INTEGER\n"
        ),
        "python/nodes/loaders/waffle_loaders/fetch_order_events/loader.py": (
            'from sqlbuild.loaders import loader\n'
            '\n'
            '\n'
            "@loader(write_strategy='append', cursor_column='load_seq', columns=[\n"
            "    {'name': 'order_id', 'type': 'INTEGER'},\n"
            "    {'name': 'customer_id', 'type': 'INTEGER'},\n"
            "    {'name': 'waffle_type', 'type': 'VARCHAR'},\n"
            "    {'name': 'quantity', 'type': 'INTEGER'},\n"
            "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
            '])\n'
            'def fetch_order_events(ctx):\n'
            "    '''Test loader fetch_order_events.'''\n"
            '    if ctx.current_cursor_value is None:\n'
            '        next_seq = 1\n'
            '    else:\n'
            '        next_seq = ctx.current_cursor_value + 1\n'
            '    first_order = (next_seq - 1) * 2 + 1\n'
            '    return [\n'
            '        {\n'
            "            'order_id': first_order,\n"
            "            'customer_id': 1 if next_seq == 1 else 3,\n"
            "            'waffle_type': 'classic',\n"
            "            'quantity': next_seq,\n"
            "            'load_seq': next_seq,\n"
            '        },\n'
            '        {\n'
            "            'order_id': first_order + 1,\n"
            "            'customer_id': 2,\n"
            "            'waffle_type': 'blueberry',\n"
            "            'quantity': next_seq + 1,\n"
            "            'load_seq': next_seq,\n"
            '        },\n'
            '    ]\n'
        ),
        "python/nodes/loaders/waffle_loaders/fetch_customers/loader.py": (
            'from sqlbuild.loaders import loader\n'
            '\n'
            '\n'
            '@loader(\n'
            "    write_strategy='merge',\n"
            "    unique_key='customer_id',\n"
            "    cursor_column='load_seq',\n"
            '    columns=[\n'
            "    {'name': 'customer_id', 'type': 'INTEGER'},\n"
            "    {'name': 'plan_name', 'type': 'VARCHAR'},\n"
            "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
            '])\n'
            'def fetch_customers(ctx):\n'
            "    '''Test loader fetch_customers.'''\n"
            '    if ctx.current_cursor_value is None:\n'
            '        return [\n'
            "            {'customer_id': 1, 'plan_name': 'basic', 'load_seq': 1},\n"
            "            {'customer_id': 2, 'plan_name': 'plus', 'load_seq': 1},\n"
            '        ]\n'
            '    return [\n'
            "        {'customer_id': 1, 'plan_name': 'pro', 'load_seq': 2},\n"
            "        {'customer_id': 3, 'plan_name': 'enterprise', 'load_seq': 2},\n"
            '    ]\n'
        ),
        "python/nodes/loaders/waffle_loaders/fetch_prices/loader.py": (
            'from sqlbuild.loaders import loader\n'
            '\n'
            '\n'
            "@loader(write_strategy='delete_insert', cursor_column='load_seq', columns=[\n"
            "    {'name': 'waffle_type', 'type': 'VARCHAR'},\n"
            "    {'name': 'price_cents', 'type': 'INTEGER'},\n"
            "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
            '])\n'
            'def fetch_prices(ctx):\n'
            "    '''Test loader fetch_prices.'''\n"
            '    classic_price = 600 if ctx.current_cursor_value is None else 650\n'
            '    return [\n'
            "        {'waffle_type': 'classic', 'price_cents': classic_price, 'load_seq': 1},\n"
            "        {'waffle_type': 'blueberry', 'price_cents': 750, 'load_seq': 1},\n"
            '    ]\n'
        ),
        "python/nodes/loaders/waffle_loaders/raw_orders/loader.py": (
            'from sqlbuild.loaders import loader\n'
            'from python.nodes.loaders.waffle_loaders.fetch_order_events.loader import fetch_order_events\n'
            'from python.nodes.loaders.waffle_loaders.fetch_prices.loader import fetch_prices\n'
            '\n'
            '\n'
            '@loader(depends_on=[fetch_order_events, fetch_prices])\n'
            'def raw_orders(ctx):\n'
            "    '''Test loader raw_orders.'''\n"
            '    events = ctx.loader(fetch_order_events)\n'
            '    prices = ctx.loader(fetch_prices)\n'
            '    cursor = ctx.query(\n'
            "        f'SELECT e.order_id, e.customer_id, e.waffle_type, e.quantity, '\n"
            "        f'p.price_cents, e.load_seq FROM {events.destination} e '\n"
            "        f'JOIN {prices.destination} p ON e.waffle_type = p.waffle_type '\n"
            "        f'ORDER BY e.order_id'\n"
            '    )\n'
            '    return [\n'
            '        {\n'
            "            'order_id': row[0],\n"
            "            'customer_id': row[1],\n"
            "            'waffle_type': row[2],\n"
            "            'quantity': row[3],\n"
            "            'price_cents': row[4],\n"
            "            'load_seq': row[5],\n"
            '        }\n'
            '        for row in cursor.fetchall()\n'
            '    ]\n'
        ),
        "python/nodes/loaders/waffle_loaders/raw_customers/loader.py": (
            'from sqlbuild.loaders import loader\n'
            'from python.nodes.loaders.waffle_loaders.fetch_customers.loader import fetch_customers\n'
            '\n'
            '\n'
            '@loader(depends_on=[fetch_customers])\n'
            'def raw_customers(ctx):\n'
            "    '''Test loader raw_customers.'''\n"
            '    customers = ctx.loader(fetch_customers)\n'
            '    cursor = ctx.query(\n'
            "        f'SELECT customer_id, plan_name, load_seq FROM {customers.destination} '\n"
            "        f'ORDER BY customer_id'\n"
            '    )\n'
            '    return [\n'
            "        {'customer_id': row[0], 'plan_name': row[1], 'load_seq': row[2]}\n"
            '        for row in cursor.fetchall()\n'
            '    ]\n'
        ),
        "python/nodes/loaders/waffle_loaders/__init__.py": (
            'from .fetch_order_events.loader import fetch_order_events\n'
            'from .fetch_customers.loader import fetch_customers\n'
            'from .fetch_prices.loader import fetch_prices\n'
            'from .raw_orders.loader import raw_orders\n'
            'from .raw_customers.loader import raw_customers\n'
        )
        ),
        "sql/models/fact_waffle_orders.sql": (
            "MODEL (description 'Test model fact_waffle_orders.', materialized table);\n\n"
            "SELECT\n"
            "  o.order_id,\n"
            "  o.customer_id,\n"
            "  c.plan_name,\n"
            "  o.waffle_type,\n"
            "  o.quantity,\n"
            "  o.price_cents,\n"
            "  o.quantity * o.price_cents AS revenue_cents,\n"
            "  o.load_seq\n"
            'FROM __source("raw_orders") o\n'
            'LEFT JOIN __source("raw_customers") c ON o.customer_id = c.customer_id\n'
        ),
        "sql/models/customer_revenue.sql": (
            "MODEL (description 'Test model customer_revenue.', materialized table);\n\n"
            "SELECT\n"
            "  customer_id,\n"
            "  COALESCE(plan_name, 'unknown') AS plan_name,\n"
            "  SUM(revenue_cents) AS revenue_cents,\n"
            "  COUNT(*) AS order_count\n"
            'FROM __ref("fact_waffle_orders")\n'
            "GROUP BY customer_id, COALESCE(plan_name, 'unknown')\n"
        ),
    }
