"""Count the statements real CLI commands send, over an orders project with every model kind."""

from __future__ import annotations

import re
import threading
from collections import Counter
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import TypeGuard

import duckdb
import pytest

from sqlbuild._native import run_cli as main
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle

_NAME: str = r'(?P<name>(?:"[^"]*"|[A-Za-z0-9_$]+)(?:\.(?:"[^"]*"|[A-Za-z0-9_$]+)){0,2})'
_KINDS: tuple[tuple[re.Pattern[str], str], ...] = (
    (re.compile(r"^SHOW\s+COLUMNS\s+IN\s+(?:TABLE|VIEW)\s+" + _NAME, re.I), "SHOW COLUMNS {role}"),
    (re.compile(r"^SHOW\s+COLUMNS\s+IN\s+SCHEMA\b", re.I), "SHOW COLUMNS IN SCHEMA"),
    (
        re.compile(r"^SHOW\s+TERSE\s+(?P<kind>TABLES|VIEWS)\s+LIKE\b", re.I),
        "SHOW TERSE {kind} LIKE",
    ),
    (re.compile(r"^SHOW\s+(?P<kind>TABLES|VIEWS)\s+LIKE\b", re.I), "SHOW {kind} LIKE"),
    (
        re.compile(r"^SHOW\s+(?:TERSE\s+)?(?P<kind>TABLES|VIEWS)\s+IN\b", re.I),
        "SHOW {kind} IN SCHEMA",
    ),
    (re.compile(r"^SHOW\s+(?P<kind>\w+)", re.I), "SHOW {kind}"),
    (re.compile(r"\binformation_schema\.(?P<kind>\w+)", re.I), "INFORMATION_SCHEMA {kind}"),
    (
        re.compile(r"^DROP\s+(?P<kind>TABLE|VIEW)\s+(?:IF\s+EXISTS\s+)?" + _NAME, re.I),
        "DROP {kind} {role}",
    ),
    (
        re.compile(
            r"^CREATE\s+(?:OR\s+REPLACE\s+)?(?:TRANSIENT\s+|TEMPORARY\s+|TEMP\s+)?"
            r"(?P<kind>TABLE|VIEW)\s+(?P<exists>IF\s+NOT\s+EXISTS\s+)?" + _NAME,
            re.I,
        ),
        "CREATE {kind} {role}",
    ),
    (
        re.compile(r"^ALTER\s+\w+\s+" + _NAME + r"\s+(?P<kind>SWAP|RENAME|SET|ADD)\b", re.I),
        "ALTER {kind} {role}",
    ),
    (re.compile(r"^INSERT\s+INTO\s+" + _NAME, re.I), "INSERT {role}"),
    (re.compile(r"^DELETE\s+FROM\s+" + _NAME, re.I), "DELETE {role}"),
    (re.compile(r"^MERGE\s+INTO\s+" + _NAME, re.I), "MERGE {role}"),
    (re.compile(r"^UPDATE\s+" + _NAME, re.I), "UPDATE {role}"),
    (re.compile(r"^SELECT\s+COUNT\(\*\)\s+FROM\s+" + _NAME, re.I), "SELECT COUNT {role}"),
    (re.compile(r"^(?P<kind>BEGIN|COMMIT|ROLLBACK)\b", re.I), "{kind}"),
    (re.compile(r"^(?:SELECT|WITH)\b", re.I), "SELECT"),
    (re.compile(r"^(?P<kind>\w+)", re.I), "{kind}"),
    (re.compile(r"^"), "OTHER"),
)
_ROLE_RULES: tuple[tuple[Callable[[str], bool], Callable[[str], str]], ...] = (
    (lambda last: last.startswith("_sqlbuild_"), lambda last: last),
    (lambda last: "__sqb_rebuild__" in last, lambda _last: "rebuild"),
    (lambda last: last.endswith("__delta"), lambda _last: "delta"),
    (lambda last: last.endswith("__staging"), lambda _last: "staging"),
    (lambda _last: True, lambda _last: "relation"),
)
_LEADING_NOISE: re.Pattern[str] = re.compile(r"^(?:\s+|--[^\n]*\n|/\*.*?\*/)+", re.S)


def relation_role(name: str) -> str:
    """The part a relation plays: a SQLBuild state table, a build intermediate, or a relation."""

    last: str = name.split(".")[-1].strip('"').lower()
    return next(rule[1](last) for rule in filter(lambda rule: rule[0](last), _ROLE_RULES))


def _has_match(pair: tuple[re.Match[str] | None, str]) -> TypeGuard[tuple[re.Match[str], str]]:
    return pair[0] is not None


def statement_kind(sql: str) -> str:
    """Classify one statement by its leading keywords and the role of the relation it names."""

    text: str = _LEADING_NOISE.sub("", sql)
    matched: tuple[re.Match[str], str] = next(
        filter(_has_match, ((pattern.search(text), template) for pattern, template in _KINDS))
    )
    template: str = matched[1]
    groups: dict[str, str | None] = matched[0].groupdict()
    kind: str = (groups.get("kind") or "").upper() + ("", " IF NOT EXISTS")[
        bool(groups.get("exists"))
    ]
    return template.format(kind=kind, role=relation_role(groups.get("name") or "")).strip()


def kind_counts(statements: tuple[str, ...] | list[str]) -> dict[str, int]:
    """Count statements by kind."""

    return dict(sorted(Counter(map(statement_kind, statements)).items()))


@contextmanager
def recorded_statements() -> Iterator[list[str]]:
    """Collect every finished outermost statement any thread of the command sends."""

    statements: list[str] = []
    lock: threading.Lock = threading.Lock()

    def record(*, sql: str) -> None:
        with lock:
            statements.append(sql)

    with StatementLifecycle.listener_scope(record):
        yield statements


def run_counted(
    *, project_dir: Path, args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> tuple[int, str, dict[str, int], tuple[str, ...]]:
    """Run one real CLI command and return its exit code, output, statement kinds and statements."""

    with recorded_statements() as statements:
        exit_code: int = main(["--project-dir", str(project_dir), "--no-color", *args])
    recorded: tuple[str, ...] = tuple(statements)
    return exit_code, "".join(capsys.readouterr()), kind_counts(recorded), recorded


def write_files(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write project files relative to ``project_dir``."""

    relative: str
    content: str
    for relative, content in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")


def matching(statements: tuple[str, ...], predicate: Callable[[str], bool]) -> tuple[str, ...]:
    """The statements one predicate selects, in issue order."""

    return tuple(filter(predicate, statements))


ORDERS_DATABASE: str = "orders.duckdb"
_RAW_ORDERS_SQL: str = (
    "CREATE OR REPLACE TABLE main.raw_orders AS SELECT * FROM (VALUES "
    "(1, TIMESTAMP '2026-01-01 08:00:00', 10.00), "
    "(2, TIMESTAMP '2026-01-02 09:00:00', 20.00), "
    "(3, TIMESTAMP '2026-01-03 10:00:00', 30.00)"
    ") AS raw(order_id, ordered_at, amount)"
)
_MODEL_HEADER: str = "MODEL (description 'Test model {name}.',\n{config});\n\n"
_ORDERS_SELECT: str = 'SELECT order_id, ordered_at, amount FROM __source("raw_orders")\n'
ORDERS_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_shop"\n'
        'adapter = "duckdb"\n\n'
        "[connection]\n"
        f'database = "{ORDERS_DATABASE}"\n'
    ),
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
    ),
    "sql/models/orders_table.sql": _MODEL_HEADER.format(
        name="orders_table",
        config=("  materialized table,\n  columns (order_id (audits [not_null])),\n"),
    )
    + _ORDERS_SELECT,
    "sql/models/orders_view.sql": _MODEL_HEADER.format(
        name="orders_view", config="  materialized view,\n"
    )
    + 'SELECT order_id, amount FROM __ref("orders_table")\n',
    "sql/models/orders_merge.sql": _MODEL_HEADER.format(
        name="orders_merge",
        config=(
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [order_id],\n"
            "  cursor ordered_at,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    raw_orders ordered_at,\n  ),\n"
        ),
    )
    + _ORDERS_SELECT,
    "sql/models/orders_daily.sql": _MODEL_HEADER.format(
        name="orders_daily",
        config=(
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  incremental_mode microbatch,\n"
            "  microbatch_strategy watermark,\n"
            "  cursor ordered_at,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_start '2026-01-01',\n"
            "  cursor_watermark_mode all,\n"
            "  cursor_inputs (\n"
            "    raw_orders (column ordered_at, roles [filter, watermark]),\n"
            "  ),\n"
            "  batch_size 1d,\n"
        ),
    )
    + _ORDERS_SELECT,
    "tests/unit/test_orders_table.sql": (
        "TEST ();\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 1 AS order_id, TIMESTAMP '2026-01-01 08:00:00' AS ordered_at, "
        "10.00 AS amount\n"
        "),\n"
        "__expected__orders_table AS (\n"
        "  SELECT 1 AS order_id, TIMESTAMP '2026-01-01 08:00:00' AS ordered_at, "
        "10.00 AS amount\n"
        ")\n"
    ),
    "tests/unit/test_orders_view_chain.sql": (
        "TEST ();\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 2 AS order_id, TIMESTAMP '2026-01-02 09:00:00' AS ordered_at, "
        "20.00 AS amount\n"
        "),\n"
        "__expected__orders_view AS (\n"
        "  SELECT 2 AS order_id, 20.00 AS amount\n"
        ")\n"
    ),
    "tests/scenarios/orders_flow.sql": (
        "SCENARIO (description 'Orders flow through the table and view.');\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 1 AS order_id, TIMESTAMP '2026-01-01 08:00:00' AS ordered_at, "
        "10.00 AS amount\n"
        "),\n"
        "__expected__orders_view AS (\n"
        "  SELECT 1 AS order_id, 10.00 AS amount\n"
        ")\n"
        "SELECT 1\n"
    ),
}


NEW_ORDER_SQL: str = (
    "INSERT INTO main.raw_orders VALUES (4, TIMESTAMP '2026-01-04 11:00:00', 40.00)"
)


def write_orders_project(*, project_dir: Path) -> Path:
    """Write the orders project and its raw source table."""

    write_files(project_dir=project_dir, files=ORDERS_FILES)
    with duckdb.connect(str(project_dir / ORDERS_DATABASE)) as connection:
        _ = connection.execute(_RAW_ORDERS_SQL)
    return project_dir


def query_rows(project_dir: Path, sql: str) -> list[tuple[object, ...]]:
    """Rows one read-only query returns from the orders database."""

    with duckdb.connect(str(project_dir / ORDERS_DATABASE), read_only=True) as connection:
        return connection.execute(sql).fetchall()


def execute_sql(project_dir: Path, sql: str) -> None:
    """Run statements against the orders database, as another session would."""

    with duckdb.connect(str(project_dir / ORDERS_DATABASE)) as connection:
        _ = connection.execute(sql)


def fingerprint_rows(project_dir: Path) -> dict[str, int]:
    """Recorded fingerprint rows per model."""

    return {
        str(name): int(str(count))
        for name, count in query_rows(
            project_dir,
            "SELECT node_name, COUNT(*) FROM main._sqlbuild_fingerprints "
            "WHERE node_type = 'model' GROUP BY node_name ORDER BY node_name",
        )
    }


def row_counts(project_dir: Path, names: tuple[str, ...]) -> dict[str, int]:
    """The row count of each named relation in the main schema."""

    return {
        name: int(str(query_rows(project_dir, f"SELECT COUNT(*) FROM main.{name}")[0][0]))
        for name in names
    }


def intermediate_relations(project_dir: Path) -> tuple[str, ...]:
    """Delta and staging relations left in the main schema."""

    return tuple(
        str(row[0])
        for row in query_rows(
            project_dir,
            "SELECT table_name FROM information_schema.tables "
            "WHERE table_name LIKE '%\\_\\_delta' ESCAPE '\\' "
            "OR table_name LIKE '%\\_\\_staging' ESCAPE '\\' ORDER BY table_name",
        )
    )


def scenario_relation_kinds(project_dir: Path) -> tuple[tuple[str, str], ...]:
    """Each scenario-owned relation's name suffix after its namespace, with its table type."""

    return tuple(
        (str(name).split("__", 3)[-1], str(kind))
        for name, kind in query_rows(
            project_dir,
            "SELECT table_name, table_type FROM information_schema.tables "
            "WHERE table_name LIKE '\\_\\_sqb\\_%' ESCAPE '\\' ORDER BY table_name",
        )
    )


def scenario_relation_name(project_dir: Path, suffix: str) -> str:
    """The qualified name of the scenario-owned relation ending in ``suffix``."""

    return "main." + next(
        str(row[0])
        for row in query_rows(
            project_dir,
            "SELECT table_name FROM information_schema.tables "
            f"WHERE table_name LIKE '\\_\\_sqb\\_%{suffix}' ESCAPE '\\'",
        )
    )


def revise_model(project_dir: Path, name: str) -> None:
    """Change one model's definition by adding a constant column."""

    model: Path = project_dir / "sql" / "models" / f"{name}.sql"
    _ = model.write_text(
        model.read_text(encoding="utf-8").replace("SELECT ", "SELECT 1 AS revision, ", 1),
        encoding="utf-8",
    )


def add_post_hook(project_dir: Path, name: str, sql: str) -> str:
    """Give one model an inline SQL post hook, returning its original definition."""

    model: Path = project_dir / "sql" / "models" / f"{name}.sql"
    original: str = model.read_text(encoding="utf-8")
    _ = model.write_text(
        re.sub(
            r"(materialized \w+,\n)",
            lambda match: f'{match.group(1)}  post_hooks [\n    inline_sql("{sql}"),\n  ],\n',
            original,
        ),
        encoding="utf-8",
    )
    return original


def add_upstream_reads(project_dir: Path, name: str, upstream: tuple[str, ...]) -> None:
    """Make one model read ``upstream`` without changing its rows, so it builds after them."""

    model: Path = project_dir / "sql" / "models" / f"{name}.sql"
    filters: str = " AND ".join(
        f'NOT EXISTS (SELECT 1 FROM __ref("{reference}") WHERE FALSE)' for reference in upstream
    )
    _ = model.write_text(
        model.read_text(encoding="utf-8").rstrip("\n") + f" WHERE {filters}\n", encoding="utf-8"
    )


def run_step(
    *,
    project_dir: Path,
    args: tuple[str, ...],
    new_orders: int,
    capsys: pytest.CaptureFixture[str],
) -> tuple[int, str, dict[str, int], tuple[str, ...]]:
    """Add ``new_orders`` raw orders, then run one real CLI command and count its statements."""

    sql: str
    for sql in (NEW_ORDER_SQL,) * new_orders:
        execute_sql(project_dir, sql)
    return run_counted(project_dir=project_dir, args=args, capsys=capsys)


def budgeted(counts: dict[str, int], prefixes: tuple[str, ...]) -> dict[str, int]:
    """The statement counts whose kind starts with one of ``prefixes``."""

    return dict(filter(lambda item: item[0].startswith(prefixes), counts.items()))


def recreated_after_drop(statements: tuple[str, ...]) -> tuple[str, ...]:
    """Drops immediately followed by a CREATE of the same table, which replacing makes redundant."""

    return tuple(
        pair[0]
        for pair in filter(
            lambda pair: (
                statement_kind(pair[0]).startswith("DROP TABLE")
                and statement_kind(pair[1]).startswith("CREATE TABLE")
                and pair[0].split()[-1] in pair[1]
            ),
            zip(statements, statements[1:], strict=False),
        )
    )
