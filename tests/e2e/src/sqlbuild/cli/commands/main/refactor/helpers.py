"""Project builders and warehouse readers for `sqb rename` and `sqb mv` e2e tests."""

from __future__ import annotations

import base64
import json
import re
import shlex
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

DATABASE_FILE: str = "orders.duckdb"
_LAYER_MOVE_HINT: re.Pattern[str] = re.compile(
    r"^.*\n  Rename and move in one step:\n.*\n.*$", re.MULTILINE
)
SCHEMA: str = "analytics"


def project_toml(*, target_settings: str = "") -> str:
    """Return the project config, with extra settings for the dev target."""

    return (
        'name = "orders_project"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{DATABASE_FILE}"\n\n'
        f'[targets.dev]\nschema = "{SCHEMA}"\n{target_settings}\n'
        "[janitor]\nenabled = true\n"
    )


_PROJECT_TOML: str = project_toml()
_SOURCES_YML: str = (
    "sources:\n"
    "  - name: raw_orders\n    description: Test source raw_orders.\n"
    "    schema: main\n"
    "    table: raw_orders\n"
    "    columns:\n"
    "      - name: order_id\n"
    "      - name: customer_id\n"
    "      - name: amount\n"
    "      - name: order_date\n"
)
STG_ORDERS: str = (
    "MODEL (description 'Test model.',\n  materialized view,\n);\n\n"
    "SELECT\n  order_id,\n  customer_id,\n  amount,\n  order_date\n"
    'FROM __source("raw_orders")\n'
)
FACT_ORDERS: str = (
    "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
    "SELECT\n  o.order_id,\n  o.customer_id,\n  o.amount,\n  o.order_date\n"
    'FROM __ref("stg_orders") AS o\n'
)
CUSTOMER_TOTALS: str = (
    "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
    "SELECT\n  customer_id,\n  SUM(amount) AS total_amount\n"
    'FROM __ref("fact_orders")\nGROUP BY customer_id\n'
)
FACT_ORDERS_TEST: str = (
    "TEST();\n\n"
    "WITH\n"
    "__ref__stg_orders AS (\n"
    "  SELECT 1 AS order_id, 10 AS customer_id, 25 AS amount, "
    "CAST('2026-01-01' AS TIMESTAMP) AS order_date\n"
    "),\n"
    "__expected__fact_orders AS (\n"
    "  SELECT 1 AS order_id, 10 AS customer_id, 25 AS amount, "
    "CAST('2026-01-01' AS TIMESTAMP) AS order_date\n"
    ")\n"
    "SELECT 1\n"
)
BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_TOML,
    "sources/raw.yml": _SOURCES_YML,
    "models/staging/stg_orders.sql": STG_ORDERS,
    "models/marts/fact_orders.sql": FACT_ORDERS,
    "models/marts/customer_totals.sql": CUSTOMER_TOTALS,
    "tests/unit/test_fact_orders.sql": FACT_ORDERS_TEST,
}
UDF_FILE: tuple[str, str] = (
    "functions/sql/udf__is_large_order.sql",
    "FUNCTION (description 'Test function udf__is_large_order.',"
    "\n  arguments (amount INTEGER),\n  returns BOOLEAN,\n);\n\namount >= 100\n",
)
CENTS_MACRO: dict[str, str] = {
    "models/staging/_sqlbuild/_macros/python/cents.py": (
        'def to_cents(expression: str) -> str:\n    return f"({expression}) * 100"\n'
    ),
    "models/staging/stg_order_cents.sql": (
        "MODEL (description 'Test model stg_order_cents.',\n  materialized view,\n);\n\n"
        'SELECT\n  order_id,\n  @py.to_cents("amount") AS amount_cents\nFROM __ref("stg_orders")\n'
    ),
}
ORDER_REFUNDS: dict[str, str] = {
    "models/staging/stg_order_refunds.sql": (
        "MODEL (description 'Test model stg_order_refunds.',\n  materialized view,\n);\n\n"
        'SELECT\n  order_id,\n  @py.to_cents("amount") AS refund_cents\nFROM __ref("stg_orders")\n'
    ),
}
SPLIT_CENTS_MACROS: dict[str, str] = {
    **CENTS_MACRO,
    "models/staging/_sqlbuild/_macros/python/cents.py": (
        CENTS_MACRO["models/staging/_sqlbuild/_macros/python/cents.py"]
        + '\n\ndef to_dollars(expression: str) -> str:\n    return f"({expression}) / 100"\n'
    ),
    "models/staging/stg_order_refunds.sql": ORDER_REFUNDS[
        "models/staging/stg_order_refunds.sql"
    ].replace("to_cents", "to_dollars"),
}


_LAYOUT_RULES: str = '\n[rules]\nselect = ["SQBRDECLARATION302"]\n'
_LABEL_MACRO: str = 'def upper_label(expression: str) -> str:\n    return f"UPPER({expression})"\n'


def grouped_role_files(*, cents_path: str, labels_path: str) -> dict[str, str]:
    """Return the cents users with ``cents_path`` and a shared label macro at ``labels_path``.

    Both cents users sit in ``models/staging``; the label macro is read there and from
    ``models/staging/eu``, so it is descendant-public at ``models/staging``.
    """

    return {
        "sqlbuild_project.toml": project_toml() + _LAYOUT_RULES,
        cents_path: CENTS_MACRO["models/staging/_sqlbuild/_macros/python/cents.py"],
        "models/staging/stg_order_cents.sql": CENTS_MACRO["models/staging/stg_order_cents.sql"],
        "models/staging/stg_order_refunds.sql": ORDER_REFUNDS[
            "models/staging/stg_order_refunds.sql"
        ].replace(
            "AS refund_cents\n", "AS refund_cents,\n  @py.upper_label(\"'refund'\") AS label\n"
        ),
        labels_path: _LABEL_MACRO,
        "models/staging/eu/stg_order_labels.sql": _label_model("stg_order_labels"),
    }


def shared_cents_with_grouped_global_role() -> dict[str, str]:
    """Return two staging users of a direct local cents macro beside a grouped ``macros/`` role.

    Moving only one user to ``models/finance`` would carry cents to the grouped top-level role,
    which a direct file cannot join; moving both leaves it local to ``models/finance``.
    """

    return {
        "sqlbuild_project.toml": project_toml() + _LAYOUT_RULES,
        **CENTS_MACRO,
        **ORDER_REFUNDS,
        "macros/python/text/labels.py": _LABEL_MACRO,
        "models/staging/stg_order_labels.sql": _label_model("stg_order_labels"),
        "models/marts/order_labels.sql": _label_model("order_labels"),
    }


def same_named_local_macros() -> dict[str, str]:
    """Return two models in different folders, each using its own local ``helpers.py`` macro."""

    return {
        "models/staging/_sqlbuild/_macros/python/helpers.py": CENTS_MACRO[
            "models/staging/_sqlbuild/_macros/python/cents.py"
        ],
        "models/staging/stg_order_cents.sql": CENTS_MACRO["models/staging/stg_order_cents.sql"],
        "models/marts/_sqlbuild/_macros/python/helpers.py": _LABEL_MACRO,
        "models/marts/order_labels.sql": _label_model("order_labels"),
    }


def _label_model(name: str) -> str:
    return (
        f"MODEL (description 'Test model {name}.',\n  materialized view,\n);\n\n"
        'SELECT\n  order_id,\n  @py.upper_label("\'order\'") AS label\nFROM __ref("stg_orders")\n'
    )


MOVE_FILE_PLACEHOLDER: str = "{move_file}"


def with_move_file(arguments: tuple[str, ...], move_file: Path) -> tuple[str, ...]:
    """Return ``arguments`` with the move-file placeholder replaced by ``move_file``."""

    return tuple(argument.replace(MOVE_FILE_PLACEHOLDER, str(move_file)) for argument in arguments)


def order_amount_union(*, first: str, second: str) -> dict[str, str]:
    """Return a view unioning amount from two models, and a view reading its output."""

    return {
        "models/marts/order_amounts.sql": (
            "MODEL (description 'Test model order_amounts.',\n  materialized view,\n);\n\n"
            f'SELECT amount FROM __ref("{first}")\nUNION ALL\n'
            f'SELECT amount FROM __ref("{second}")\n'
        ),
        "models/marts/order_amount_reads.sql": (
            'MODEL (description "Test model order_amount_reads.",'
            '\n  materialized view,\n);\n\nSELECT amount FROM __ref("order_amounts")\n'
        ),
    }


ORDER_EXPORT: dict[str, str] = {
    "models/marts/order_export.sql": (
        'MODEL (description "Test model order_export.",'
        '\n  materialized view,\n);\n\nSELECT *\nFROM __ref("fact_orders")\n'
    ),
}
ORDER_AMOUNT_CHECK: dict[str, str] = {
    "python/checks/order_amounts.py": (
        "from sqlbuild.checks import check\n"
        "from sqlbuild.refs import model\n\n\n"
        '@check(depends_on=model("stg_orders"))\n'
        "def order_amounts_present(ctx):\n"
        '    """Compare with: select amount from stg_orders."""\n'
        "    return ctx.pass_()\n"
    ),
}
CUSTOMER_DECLARATIONS: dict[str, str] = {
    "seeds/customers.csv": "customer_id,customer_name\n10,Ada\n11,Bo\n",
    "seeds/customers.yml": (
        "seeds:\n"
        "  - name: customers\n    description: Test seed customers.\n"
        "    columns:\n"
        "      - name: customer_id\n"
        "        type: INTEGER\n"
        "        audits:\n"
        "          - relationships:\n"
        '              to: __ref("stg_orders")\n'
        "              field: customer_id\n"
        "      - name: customer_name\n"
        "        type: VARCHAR\n"
    ),
    "sources/customers.yml": (
        "sources:\n"
        "  - name: raw_customers\n    description: Test source raw_customers.\n"
        '    expression: "SELECT 10 AS customer_id UNION ALL SELECT 11"\n'
        "    columns:\n"
        "      - name: customer_id\n"
        "        audits:\n"
        "          - relationships: {to: '__ref(\"stg_orders\")', field: customer_id}\n"
    ),
}
ORDER_SHAPE: dict[str, str] = {
    "models/marts/_sqlbuild/_schemas/order_shape.sql": (
        "SCHEMA (description 'Test schema.',"
        "\n  name order_shape,\n  columns (\n    customer_id (audits [relationships "
        '(to __ref("stg_orders"), field customer_id)]),\n  ),\n);\n'
    ),
    "models/marts/fact_orders.sql": FACT_ORDERS.replace(
        "materialized table,", "materialized table,\n  model_schema order_shape,"
    ),
}
FLAGGED_ORDERS: str = (
    "MODEL (\n"
    '  description "Orders with a flag",\n'
    "  materialized table,\n"
    "  columns (\n"
    "    order_id (type VARCHAR)\n"
    "    is_flagged (type BOOLEAN)\n"
    "  ),\n"
    ");\n\n"
    "SELECT\n"
    "  CAST('o1' AS VARCHAR) AS order_id,\n"
    "  CAST(TRUE AS BOOLEAN) AS is_flagged\n"
)
FLAGGED_ORDER_SUMMARY: str = (
    "MODEL (\n"
    '  description "Orders summary",\n'
    "  materialized table,\n"
    ");\n\n"
    "WITH orders AS (\n"
    "  SELECT *\n"
    '  FROM __ref("flagged_orders")\n'
    ")\n\n"
    "SELECT\n"
    "  order_id,\n"
    "  is_flagged\n"
    "FROM orders\n"
)
FLAGGED_CONTRACT: dict[str, str] = {
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS,
    "models/marts/flagged_order_summary.sql": FLAGGED_ORDER_SUMMARY,
}
FLAGGED_AUDITS: dict[str, str] = {
    "models/marts/_sqlbuild/_audits/generic/flag_check.sql": (
        "AUDIT ();\n\nSELECT *\nFROM @relation\nWHERE @where_condition AND order_id IS NULL\n"
    ),
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS.replace(
        "is_flagged (type BOOLEAN)",
        "is_flagged (type BOOLEAN, audits [flag_check "
        "(where_condition \"is_flagged = TRUE AND 'is_flagged' <> ''\")])",
    ).replace(
        "  ),\n);",
        '  ),\n  audits [flag_check (name "orders_flag", where_condition "NOT is_flagged")],\n);',
    ),
    "models/marts/flagged_order_summary.sql": FLAGGED_ORDER_SUMMARY.replace(
        "  materialized table,\n",
        '  materialized table,\n  audits [flag_check (where_condition "is_flagged IS NULL")],\n',
    ),
}
_FLAG_CHECK: str = (
    "AUDIT ();\n\nSELECT *\nFROM @relation\nWHERE @where_condition AND order_id IS NULL\n"
)
FLAGGED_SUBQUERY_AUDITS: dict[str, str] = {
    "models/marts/_sqlbuild/_audits/generic/flag_check.sql": _FLAG_CHECK,
    "models/marts/vip_flags.sql": (
        'MODEL (description "VIP flags", materialized table);\n'
        "SELECT CAST(TRUE AS BOOLEAN) AS is_flagged\n"
    ),
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS.replace(
        "  ),\n);",
        "  ),\n  audits [\n"
        '    flag_check (name "vip_flag", where_condition '
        "'is_flagged IN (SELECT is_flagged FROM __ref(\"vip_flags\"))'),\n"
        '    flag_check (name "own_flag", where_condition '
        "'is_flagged IN (SELECT is_flagged FROM __ref(\"flagged_orders\"))'),\n"
        "  ],\n);",
    ),
}
FLAGGED_USING_AUDIT: dict[str, str] = {
    **FLAGGED_SUBQUERY_AUDITS,
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS.replace(
        "  ),\n);",
        "  ),\n  audits [\n"
        '    flag_check (name "joined_flag", where_condition '
        '\'order_id IN (SELECT o.order_id FROM __ref("flagged_orders") AS o '
        'JOIN __ref("vip_flags") AS v USING (is_flagged))\'),\n'
        "  ],\n);",
    ),
}
FLAGGED_LAMBDA_AUDIT: dict[str, str] = {
    "models/marts/_sqlbuild/_audits/generic/flag_check.sql": _FLAG_CHECK,
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS.replace(
        "  ),\n);",
        "  ),\n  audits [\n"
        '    flag_check (name "lambda_flag", where_condition '
        "'len(list_filter([is_flagged], lambda is_flagged: is_flagged)) > 0'),\n"
        "  ],\n);",
    ),
}
FLAGGED_LITERAL_AUDIT: dict[str, str] = {
    "models/marts/_sqlbuild/_audits/generic/prefix_check.sql": (
        "AUDIT ();\n\nSELECT *\nFROM @relation\nWHERE @'prefix' = ''\n"
    ),
    "models/marts/flagged_orders.sql": FLAGGED_ORDERS.replace(
        "  ),\n);", '  ),\n  audits [prefix_check (prefix "is_flagged")],\n);'
    ),
}
ORDERS_MACRO: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_TOML + "\n[references]\nenforce_explicit = false\n",
    "models/staging/_sqlbuild/_macros/python/orders.py": (
        "def staged_orders() -> str:\n    return '__ref(\"stg_orders\")'\n"
    ),
    "models/staging/stg_order_count.sql": (
        "MODEL (description 'Test model stg_order_count.',\n  materialized view,\n);\n\n"
        "SELECT COUNT(*) AS order_count\nFROM @py.staged_orders()\n"
    ),
}
_INCREMENTAL_HEADER: str = (
    "  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n"
    "  unique_key order_id,\n"
    "  cursor order_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    '  cursor_start "2026-01-01",\n'
    "  cursor_inputs (\n    stg_orders order_date,\n  ),\n"
)


_HISTORY_HEADERS: dict[str, str] = {
    "table": "  materialized table,\n",
    "view": "  materialized view,\n",
    "incremental": _INCREMENTAL_HEADER,
}
_UDF_COLUMN: dict[bool, str] = {
    True: ',\n  __udf("udf__is_large_order")(amount) AS is_large',
    False: "",
}
_UDF_FILES: dict[bool, dict[str, str]] = {True: {UDF_FILE[0]: UDF_FILE[1]}, False: {}}


def order_history_files(*, materialized: str, udf: bool) -> dict[str, str]:
    """Return an order_history model, optionally calling a UDF discovery cannot fingerprint."""

    return {
        "models/marts/order_history.sql": (
            "MODEL (description 'Test model order_history.',"
            f"\n{_HISTORY_HEADERS[materialized]});\n\n"
            f"SELECT\n  order_id,\n  amount,\n  order_date{_UDF_COLUMN[udf]}\n"
            'FROM __ref("stg_orders")\n'
        ),
        **_UDF_FILES[udf],
    }


def write_orders_project(
    *, tmp_path: Path, files: Mapping[str, str] | None = None, raw_order_count: int = 3
) -> Path:
    """Write the base orders project plus overrides and load raw orders."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders_project", repo_files={**BASE_FILES, **(files or {})}
    )
    load_raw_orders(project_dir=project_dir, order_ids=tuple(range(1, raw_order_count + 1)))
    return project_dir


def order_history_copy() -> dict[str, str]:
    """Return a second model with exactly the order_history definition."""

    return {
        "models/marts/order_history_copy.sql": order_history_files(
            materialized="incremental", udf=False
        )["models/marts/order_history.sql"]
    }


def strip_migration_fingerprints(*, project_dir: Path, models: tuple[str, ...]) -> None:
    """Make built models look like ones built before rename matching recorded fingerprints."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        rows: list[tuple[Any, ...]] = connection.execute(
            f"SELECT node_name, metadata_json_b64 FROM {SCHEMA}._sqlbuild_fingerprints "
            "WHERE list_contains(?, node_name)",
            [list(models)],
        ).fetchall()
        model: str
        encoded: str
        for model, encoded in rows:
            metadata: dict[str, Any] = json.loads(base64.b64decode(encoded))
            _ = metadata.pop("migration_fingerprint", None)
            _ = connection.execute(
                f"UPDATE {SCHEMA}._sqlbuild_fingerprints SET metadata_json_b64 = ? "
                "WHERE node_name = ? AND metadata_json_b64 = ?",
                [base64.b64encode(json.dumps(metadata).encode()).decode(), model, encoded],
            )
    finally:
        connection.close()


def drop_view_outside_sqlbuild(*, project_dir: Path, name: str) -> None:
    """Drop a built view directly in the warehouse, leaving SQLBuild's history in place."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        _ = connection.execute(f"DROP VIEW {SCHEMA}.{name}")
    finally:
        connection.close()


def declare_missing_column_origin(*, project_dir: Path) -> None:
    """Add a revenue column that declares migrate_from a column the table never had."""

    path: Path = project_dir / "models/marts/order_history.sql"
    contents: str = path.read_text(encoding="utf-8")
    _ = path.write_text(
        contents.replace(
            "MODEL (", "MODEL (\n  columns (revenue (migrate_from gross_amount)),\n  ", 1
        ).replace("  order_date\n", "  order_date,\n  amount AS revenue\n", 1),
        encoding="utf-8",
    )


def existing_files(*, project_dir: Path, paths: tuple[str, ...]) -> tuple[str, ...]:
    """Return the given project-relative paths that exist, sorted."""

    present: frozenset[str] = frozenset(
        path.relative_to(project_dir).as_posix() for path in project_dir.rglob("*.*")
    )
    return tuple(sorted(frozenset(paths) & present))


def remove_files(*, project_dir: Path, paths: tuple[str, ...]) -> None:
    """Delete project files by hand, as an author removing a model would."""

    path: str
    for path in paths:
        (project_dir / path).unlink()


def load_raw_orders(*, project_dir: Path, order_ids: tuple[int, ...]) -> None:
    """Replace the raw orders with one order per id, dated January of that day."""

    values: str = ", ".join(
        f"({order_id}, {10 + order_id % 2}, {order_id * 50}, TIMESTAMP '2026-01-{order_id:02d}')"
        for order_id in order_ids
    )
    execute_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "CREATE OR REPLACE TABLE main.raw_orders AS SELECT * FROM (VALUES "
            f"{values}) AS t(order_id, customer_id, amount, order_date)"
        ),
    )


def sqb(project_dir: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run one sqb command without colour."""

    return run_sqb(command=("--no-color", *args), project_dir=project_dir)


def sqb_json(project_dir: Path, *args: str) -> tuple[int, dict[str, Any]]:
    """Run one sqb command with --json and parse its stdout."""

    result: subprocess.CompletedProcess[str] = sqb(project_dir, *args, "--json")
    return result.returncode, json.loads(result.stdout)


def project_text(project_dir: Path) -> dict[str, str]:
    """Return every authored project file keyed by its relative path."""

    paths: tuple[Path, ...] = (
        project_dir / "sqlbuild_project.toml",
        *project_dir.glob("sources/*.yml"),
        *project_dir.glob("seeds/*.yml"),
        *project_dir.glob("models/**/*.sql"),
        *project_dir.glob("models/**/*.py"),
        *project_dir.glob("functions/**/*.sql"),
        *project_dir.glob("tests/**/*.sql"),
    )
    return {
        path.relative_to(project_dir).as_posix(): path.read_text(encoding="utf-8")
        for path in sorted(paths)
    }


def fragments_present(*, files: dict[str, str], expected: dict[str, tuple[str, ...]]) -> bool:
    """Return whether every expected fragment appears in its file."""

    found: list[bool] = []
    path: str
    fragments: tuple[str, ...]
    for path, fragments in expected.items():
        found.extend(fragment in files[path] for fragment in fragments)
    return all(found)


def relation_type(*, project_dir: Path, name: str) -> str | None:
    """Return the information_schema table type of one relation, or None."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "SELECT table_type FROM information_schema.tables "
            f"WHERE table_schema = '{SCHEMA}' AND table_name = '{name}'"
        ),
    )
    return next((str(row[0]) for row in rows), None)


def relation_columns(*, project_dir: Path, name: str) -> tuple[str, ...]:
    """Return the column names of one relation in ordinal order."""

    return tuple(
        str(row[0])
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                "SELECT column_name FROM information_schema.columns "
                f"WHERE table_schema = '{SCHEMA}' AND table_name = '{name}' "
                "ORDER BY ordinal_position"
            ),
        )
    )


def order_ids(*, project_dir: Path, name: str) -> tuple[int, ...]:
    """Return the order ids stored in one relation."""

    return tuple(
        int(row[0])
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=f"SELECT order_id FROM {SCHEMA}.{name} ORDER BY order_id",
        )
    )


def layer_move_hint(stdout: str) -> tuple[str, ...]:
    """Return the layer-folder hint lines around the one-step command; empty when absent."""

    return tuple("\n".join(_LAYER_MOVE_HINT.findall(stdout)).splitlines())


def run_printed_command(command: str) -> subprocess.CompletedProcess[str]:
    """Run a command line exactly as the CLI printed it."""

    return subprocess.run(
        ["uv", "run", *shlex.split(command)],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


CENTS_CHECK: str = "python/checks/order_cents.py"


def cents_check(imports: str, body: str = '    assert to_cents("1")\n') -> dict[str, str]:
    """Return the cents macro and its model plus a Python check importing the macro module."""

    return {
        **CENTS_MACRO,
        CENTS_CHECK: (
            "from sqlbuild.checks import check\nfrom sqlbuild.refs import model\n\n"
            f"{imports}\n\n\n"
            '@check(depends_on=model("stg_orders"))\n'
            "def order_cents_present(ctx):\n"
            '    """Cents come from the shared macro."""\n'
            f"{body}"
            "    return ctx.pass_()\n"
        ),
    }


_FOLDER_LIMIT_RULE: str = """from collections import Counter

from sqlbuild.rules import Finding, Project, RuleContext, rule

_MAXIMUM: int = 3


@rule(code="XSQBRLAYOUT001", message="Folders hold few models", remediation="Split the folder.")
def folder_size(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    counts: Counter[str] = Counter(model.path.parent.as_posix() for model in ctx.project.models)
    return [
        ctx.finding(
            subject=folder, message=f"{folder} holds {count} models; the maximum is {_MAXIMUM}"
        )
        for folder, count in sorted(counts.items())
        if count > _MAXIMUM
    ]
"""


def layout_view(name: str, source: str = '__ref("stg_orders")') -> str:
    """Return a view model selecting order_id from `source`."""

    return (
        f"MODEL (description 'Test model {name}.',\n  materialized view,\n);\n\n"
        f"SELECT order_id FROM {source}\n"
    )


OVER_FULL_STAGING: dict[str, str] = {
    "sqlbuild_project.toml": project_toml()
    + '\n[rules]\nselect = ["XSQBRLAYOUT001"]\n\n'
    + "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
    "rules/layout.py": _FOLDER_LIMIT_RULE,
    **{
        f"models/staging/stg_order_{name}.sql": layout_view(f"stg_order_{name}")
        for name in ("lines", "refunds", "returns", "notes")
    },
}
_DOMAIN_RULES: str = '\n[rules]\nselect = ["SQBRPROJECT202"]\ndomains = ["orders"]\n'
MIXED_OWNER: dict[str, str] = {
    "sqlbuild_project.toml": project_toml() + _DOMAIN_RULES,
    "models/orders/staging/orders__stg__lines.sql": layout_view(
        "orders__stg__lines", '__source("raw_orders")'
    ),
    "models/orders/staging/orders__stg__refunds.sql": layout_view(
        "orders__stg__refunds", '__source("raw_orders")'
    ),
    "models/orders/staging/web/orders__stg__web_lines.sql": layout_view(
        "orders__stg__web_lines", '__source("raw_orders")'
    ),
}
MIXED_OWNER_CHILD: str = "models/orders/staging/core/"

RESERVED_CTE_FACT_ORDERS: str = (
    "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
    'WITH __helper__tmp AS (SELECT * FROM __ref("stg_orders"))\n'
    "SELECT\n  o.order_id,\n  o.customer_id,\n  o.amount,\n  o.order_date\n"
    "FROM __helper__tmp AS o\n"
)
SIBLING_OWNERS: dict[str, str] = {
    "sqlbuild_project.toml": project_toml()
    + '\n[rules]\nselect = ["SQBRPROJECT204"]\ndomains = ["orders"]\n\n'
    + "[rules.thresholds]\nmin_shared_owner_prefix_directories = 2\n",
    **{
        f"models/orders/staging/{folder}/orders__stg__{name}.sql": layout_view(
            f"orders__stg__{name}", '__source("raw_orders")'
        )
        for folder, name in (("fin_a", "a"), ("fin_b", "b"), ("core", "c"))
    },
}


def refresh_job(select: str) -> str:
    """Return a job file whose `select` is the TOML value `select`."""
    return f'name = "orders_refresh_job"\ndescription = "Refresh orders."\nselect = {select}\n'
