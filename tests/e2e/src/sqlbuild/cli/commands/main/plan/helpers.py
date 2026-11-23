"""Project builders for plan command e2e tests."""

from __future__ import annotations

import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from bisect import bisect_right
from collections.abc import Callable, Iterator
from contextlib import closing, contextmanager
from dataclasses import dataclass
from itertools import accumulate
from pathlib import Path
from typing import Any, cast
from urllib.request import Request, urlopen
from uuid import uuid4

import sqlbuild._native as native_module
from sqlbuild._native import (
    CompilePipelineOptions,
    CompilePipelineResult,
    discover_project_inputs,
    run_compile_pipeline,
)
from sqlbuild.runtime.contracts.models import ConnectionHooks
from tests.e2e.src.sqlbuild.cli.commands.main.lineage.helpers import DIAMOND_LAYERS
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SnapshotApi,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    request_plan,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    snapshot_api,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    offline_cursor_bound_relations,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeColumn,
    FakeRelation,
    OfflineSnowflakeAdapter,
    RecordingSnowflakeWarehouse,
)

DIAMOND_DATABASE_FILE: str = "orders.duckdb"
_DIAMOND_PROJECT_TOML: str = (
    'name = "incremental_diamond"\nadapter = "duckdb"\n\n'
    f'[connection]\ndatabase = "{DIAMOND_DATABASE_FILE}"\n'
)
_DIAMOND_SOURCES_YML: str = (
    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
    "    schema: main\n    table: raw_orders\n"
)
_DIAMOND_ROOT_PATH: str = "sql/models/orders_0.sql"


def _incremental_header(*, extra_config: str) -> str:
    return (
        "MODEL (description 'Test model orders_0.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        f"{extra_config}"
        ");\n\n"
    )


def _diamond_root_sql(*, amount_offset: int) -> str:
    return (
        _incremental_header(extra_config="  replay_on_change full,\n")
        + "SELECT order_id, order_date, amount_cents + "
        f'{amount_offset} AS amount_cents FROM __source("raw_orders")\n'
    )


def prepare_incremental_diamond_project(*, tmp_path: Path) -> Path:
    """Write incremental layered diamonds whose root replays fully on change."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _DIAMOND_PROJECT_TOML,
        "sql/sources/raw.yml": _DIAMOND_SOURCES_YML,
        _DIAMOND_ROOT_PATH: _diamond_root_sql(amount_offset=0),
    }
    for layer in range(1, DIAMOND_LAYERS + 1):
        for side in ("left", "right"):
            files[f"sql/models/orders_{side}_{layer}.sql"] = (
                _incremental_header(extra_config="") + "SELECT order_id, order_date, amount_cents "
                f'FROM __ref("orders_{layer - 1}")\n'
            )
        files[f"sql/models/orders_{layer}.sql"] = (
            _incremental_header(
                extra_config=(
                    f"  cursor_inputs (orders_left_{layer} order_date, "
                    f"orders_right_{layer} order_date,),\n"
                )
            )
            + "SELECT l.order_id, l.order_date, l.amount_cents + r.amount_cents AS amount_cents\n"
            f'FROM __ref("orders_left_{layer}") AS l\n'
            f'JOIN __ref("orders_right_{layer}") AS r ON l.order_id = r.order_id\n'
        )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="incremental_diamond", repo_files=files
    )
    execute_duckdb(
        db_path=project_dir / DIAMOND_DATABASE_FILE,
        sql=(
            "CREATE TABLE main.raw_orders AS SELECT i AS order_id, "
            "TIMESTAMP '2026-01-01' + to_days(CAST(i - 1 AS INTEGER)) AS order_date, "
            "100 + i AS amount_cents FROM range(1, 4) AS t(i)"
        ),
    )
    return project_dir


def change_incremental_diamond_root(*, project_dir: Path) -> None:
    """Change the root query, which replays fully under its own replay_on_change."""

    (project_dir / _DIAMOND_ROOT_PATH).write_text(
        _diamond_root_sql(amount_offset=1), encoding="utf-8"
    )


def prepare_changed_incremental_diamond_project(*, tmp_path: Path) -> Path:
    """Build the incremental diamonds, then change the root query."""

    project_dir: Path = prepare_incremental_diamond_project(tmp_path=tmp_path)
    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build.returncode == 0, build.stdout + build.stderr
    change_incremental_diamond_root(project_dir=project_dir)
    return project_dir


_MISSING_SOURCE_TABLES_SOURCES_YML: str = """sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: raw
    table: orders
    freshness:
      strategy: column
      column: updated_at
      type: timestamp
  - name: raw_customers
    description: Test source raw_customers.
    schema: raw
    table: customers
    freshness:
      strategy: column
      column: updated_at
      type: timestamp
  - name: raw_payments
    description: Test source raw_payments.
    schema: raw
    table: payments
    freshness:
      strategy: column
      column: updated_at
      type: timestamp
"""


def prepare_missing_source_tables_project(*, tmp_path: Path) -> Path:
    """Write three source-reading models where only the payments source table is missing."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="missing_source_tables",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "missing_source_tables"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/sources/raw.yml": _MISSING_SOURCE_TABLES_SOURCES_YML,
            "sql/models/orders.sql": (
                'MODEL (description "Test model orders.", '
                'materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "sql/models/customers.sql": (
                'MODEL (description "Test model customers.", '
                'materialized table);\n\nSELECT * FROM __source("raw_customers")\n'
            ),
            "sql/models/payments.sql": (
                'MODEL (description "Test model payments.", '
                'materialized table);\n\nSELECT * FROM __source("raw_payments")\n'
            ),
        },
    )
    db_path: Path = project_dir / "warehouse.duckdb"
    statement: str
    for statement in (
        "CREATE SCHEMA raw",
        "CREATE TABLE raw.orders AS SELECT 1 AS order_id, "
        "TIMESTAMP '2026-01-01 00:00:00' AS updated_at",
        "CREATE TABLE raw.customers AS SELECT 1 AS customer_id, "
        "TIMESTAMP '2026-01-02 00:00:00' AS updated_at",
    ):
        execute_duckdb(db_path=db_path, sql=statement)
    return project_dir


_EXIT_CODE_PROJECT_TOML: str = (
    'name = "plan_exit_codes"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connection]\n"
    'database = "plan.duckdb"\n\n'
    "[targets.dev]\n"
    'schema = "analytics"\n'
)
_REVENUE_MODEL: str = "MODEL (description 'Revenue.', materialized table);\n\nSELECT 1 AS amount\n"
_DAILY_REVENUE_MODEL: str = (
    "MODEL (description 'Daily revenue.', materialized table, migrate_from revenue);\n\n"
    "SELECT 1 AS amount\n"
)
RECLAIMED_NAME: dict[str, str] = {"sql/models/revenue.sql": _REVENUE_MODEL}
REMOVABLE_MIGRATION_WARNING: str = "migrate_from can be removed from 'daily_revenue'"
NAME_CLAIM_ERROR: str = "model 'revenue' would replace analytics.revenue"


def migrated_project(tmp_path: Path) -> Path:
    """A project whose `revenue` table was migrated to `daily_revenue` and built."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="plan_exit_codes",
        repo_files={
            "sqlbuild_project.toml": _EXIT_CODE_PROJECT_TOML,
            "sql/models/revenue.sql": _REVENUE_MODEL,
        },
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    (project_dir / "sql/models/revenue.sql").unlink()
    (project_dir / "sql/models/daily_revenue.sql").write_text(
        _DAILY_REVENUE_MODEL, encoding="utf-8"
    )
    migrated: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert migrated.returncode == 0, migrated.stdout + migrated.stderr
    return project_dir


PLAN_TIMINGS_HEADER: str = "Plan timings"
_TIMING_STEP_ROW: re.Pattern[str] = re.compile(r"^  (\S.*?)\s+(\d+\.\d)\s+\d+%$", re.MULTILINE)
_TIMING_TOTAL_ROW: re.Pattern[str] = re.compile(r"^  Total\s+(\d+\.\d)$", re.MULTILINE)


@dataclass(frozen=True)
class PlanTimingTable:
    """A `--timings` table: its header words, every row line, the parsed steps and the total."""

    header: list[str]
    row_count: int
    steps: list[tuple[str, float]]
    total_seconds: float


def migrated_plan_output(tmp_path: Path, plan_args: tuple[str, ...]) -> str:
    """`sqb plan` output over the migrated project; the plan must succeed."""

    project_dir: Path = migrated_project(tmp_path)
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", *plan_args),
        project_dir=project_dir,
    )
    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    return output


def plan_timing_table(output: str) -> PlanTimingTable:
    """The `--timings` table at the end of `output`."""

    table: str = output[output.index(PLAN_TIMINGS_HEADER) :].rstrip("\n")
    lines: list[str] = table.splitlines()
    total: re.Match[str] | None = _TIMING_TOTAL_ROW.search(table)
    assert total is not None, output
    return PlanTimingTable(
        header=lines[0].split(),
        row_count=len(lines) - 2,
        steps=[(name, float(seconds)) for name, seconds in _TIMING_STEP_ROW.findall(table)],
        total_seconds=float(total.group(1)),
    )


# ---- the DuckShop 10K plan preview guard ----

DUCKSHOP_10K_TARGET: str = "dev"
DUCKSHOP_10K_GENERATOR: Path = REPO_ROOT / "benchmarks" / "duckshop-10k"
DUCKSHOP_10K_SETUP_SECONDS: int = 600
PREVIEW_PUBLISHED: str = "Location snapshot published."
PREVIEW_DONE: str = "Location work completed."
_PAGE_RECORDS: int = 2000
_PAGE_BYTES: int = 6 * 1024 * 1024
_REQUIRED_SYNCS: tuple[str, ...] = ("catalog", "fingerprints", "consumption")


@dataclass(frozen=True)
class ServedLocation:
    api: SnapshotApi
    control: Path
    log: Path
    process: subprocess.Popen[bytes]
    #: Seconds from starting the location until it published its compiled project.
    published_seconds: float


@contextmanager
def served_duckshop_location(
    directory: Path, project: Path, *, env: dict[str, str] | None = None
) -> Iterator[ServedLocation]:
    """A control server and `sqb location serve` of the project, once the location has published."""
    directory.mkdir(parents=True)
    with snapshot_api(
        directory / "control",
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as api:
        log: Path = directory / "location.log"
        started: float = time.monotonic()
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                DUCKSHOP_10K_TARGET,
            ],
            log,
            directory / "location.stdout",
            env=os.environ | (env or {}) | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as process:
            control: Path = directory / "control/.sqlbuild/control.sqlite3"
            publishing: ServedLocation = ServedLocation(api, control, log, process, 0.0)
            published: float = wait_for_log_count(publishing, PREVIEW_PUBLISHED, 1, 900.0)
            yield ServedLocation(api, control, log, process, published - started)


def wait_for_log_count(served: ServedLocation, text: str, count: int, timeout: float) -> float:
    """When the location log first holds `count` lines of `text`."""
    deadline: float = time.monotonic() + timeout
    while (content := served.log.read_text(errors="replace")).count(text) < count:
        assert served.process.poll() is None, content[-4000:]
        assert time.monotonic() < deadline, content[-4000:]
        time.sleep(0.02)
    return time.monotonic()


def published_keys(control: Path) -> list[str]:
    """Every node a `sqb build` of the published project runs, as the server freezes it."""
    with closing(sqlite3.connect(control)) as database:
        return [
            row[0]
            for row in database.execute(
                "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c "
                "ON c.snapshot_id=n.snapshot_id WHERE n.node_kind IN "
                "('model','source','seed','udf','table_fn','asset','loader')"
            )
        ]


def location_session(control: Path) -> tuple[str, int]:
    """The serving location's session and its published generation, which metadata pages name."""
    with closing(sqlite3.connect(control)) as database:
        session: str = database.execute(
            "SELECT instance_id FROM sqb_location_instances WHERE target=?", (DUCKSHOP_10K_TARGET,)
        ).fetchone()[0]
        generation: int = database.execute(
            "SELECT s.generation FROM sqb_current_snapshots c JOIN sqb_snapshots s "
            "ON s.snapshot_id=c.snapshot_id WHERE c.target=?",
            (DUCKSHOP_10K_TARGET,),
        ).fetchone()[0]
    return str(session), int(generation)


def _location_post(api: SnapshotApi, path: str, body: dict[str, Any]) -> None:
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}{path}",
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {api.bearer}", "Content-Type": "application/json"},
    )
    with urlopen(request, timeout=120) as response:
        _ = response.read()


def _pages(records: list[dict[str, Any]]) -> Iterator[list[dict[str, Any]]]:
    """Pages of at most `_PAGE_RECORDS` records and `_PAGE_BYTES`, each holding at least one."""
    sizes: list[int] = [len(json.dumps(record)) for record in records]
    start: int = 0
    while start < len(records):
        totals: list[int] = list(accumulate(sizes[start : start + _PAGE_RECORDS]))
        end: int = start + max(1, bisect_right(totals, _PAGE_BYTES))
        yield records[start:end]
        start = end


def _namespace_order(item: Any) -> str:
    """A stable order over state namespaces, whose database may be unset."""
    return json.dumps(item, default=str)


def mirror_synced_state(served: ServedLocation, batches: dict[str, list[dict[str, Any]]]) -> None:
    """Deliver the synced state as a location's sync does: pages, complete scans, clean syncs."""
    session, generation = location_session(served.control)
    path: str = f"/targets/{DUCKSHOP_10K_TARGET}/metadata"

    def page_of(kind: str, records: list[dict[str, Any]]) -> dict[str, Any]:
        return {"kind": kind, "session_id": session, "generation": generation, "records": records}

    namespaces: set[tuple[str | None, str]] = set()
    for kind in ("fingerprints", "consumption", "observations"):
        for page in _pages(batches[kind]):
            _location_post(served.api, path, page_of(kind, page))
            namespaces.update((record["state_database"], record["state_schema"]) for record in page)
    by_namespace: dict[tuple[str | None, str], list[dict[str, Any]]] = {}
    for record in batches["catalog"]:
        by_namespace.setdefault((record["state_database"], record["state_schema"]), []).append(
            record
        )
    for (database, schema), records in sorted(by_namespace.items(), key=_namespace_order):
        scan: str = str(uuid4())
        pages: list[list[dict[str, Any]]] = list(_pages(records))
        for index, page in enumerate(pages):
            body: dict[str, Any] = page_of("catalog", page)
            body["scan"] = {
                "scan_id": scan,
                "location": {"database": database, "schema": schema},
                "complete": index == len(pages) - 1,
            }
            _location_post(served.api, path, body)
        namespaces.add((database, schema))
    for database, schema in sorted(namespaces, key=_namespace_order):
        for table in _REQUIRED_SYNCS:
            _location_post(
                served.api,
                "/health",
                {
                    "event_id": str(uuid4()),
                    "session_id": session,
                    "target": DUCKSHOP_10K_TARGET,
                    "health": {
                        "kind": "warehouse_sync",
                        "generation": generation,
                        "location": {"database": database, "schema": schema},
                        "table": table,
                        "failure": None,
                        "records_sent": 1,
                        "elapsed_millis": 1,
                    },
                },
            )


def timed_plan(
    served: ServedLocation,
    selection: list[str],
    settings: dict[str, Any],
    *,
    fresh: bool = False,
) -> tuple[float, dict[str, Any]]:
    """A person's plan from the moment the server queues it until the location stored it.

    `fresh` is Plan again, so the location plans anew instead of returning a plan it kept.
    """
    started: float = time.monotonic()
    request_id: str = request_plan(served.control, selection, settings, fresh=fresh)
    deadline: float = started + 600.0
    result: str | None = None
    while result is None:
        assert served.process.poll() is None, served.log.read_text(errors="replace")[-4000:]
        assert time.monotonic() < deadline, served.log.read_text(errors="replace")[-4000:]
        time.sleep(0.02)
        with closing(sqlite3.connect(served.control)) as database:
            row: tuple[str | None] | None = database.execute(
                "SELECT result_json FROM sqb_location_work WHERE request_id=?", (request_id,)
            ).fetchone()
        result = (row or (None,))[0]
    elapsed: float = time.monotonic() - started
    document: dict[str, Any] = json.loads(result)["outcome"]["document"]
    return elapsed, document


# ---- a live plan of the DuckShop 10K project against its synced warehouse ----

#: The database and schema the synced state places built models, seeds and state tables in.
_LIVE_TARGET: tuple[str, str] = ("DUCKSHOP_10K", "DEV")
#: One source table in this many carries a column its declaration lacks, as warehouses drift.
DRIFT_EVERY: int = 50
_DRIFT_COLUMN: FakeColumn = FakeColumn(name="_LOADED_AT", data_type="TIMESTAMP_NTZ")
_NUMBER_TYPE: re.Pattern[str] = re.compile(r"NUMBER\((\d+),(\d+)\)")
_RELATION_TYPES: dict[str, str] = {"TABLE": "BASE TABLE", "VIEW": "VIEW"}
#: Declared string types Snowflake stores, and lists, as TEXT of its default length.
_LISTED_TYPES: dict[str, str] = {"VARCHAR": "TEXT", "STRING": "TEXT", "TEXT": "TEXT"}
_TEXT_LENGTHS: dict[str, int] = dict.fromkeys(_LISTED_TYPES, 16_777_216)


@dataclass(frozen=True)
class LivePlan:
    """One live plan's wall time and the work it did against the offline warehouse."""

    seconds: float
    model_entries: int
    source_rebinding_analyses: int
    cursor_relations: int
    cursor_statements: int
    warehouse_statements: int


def _number_column(name: str, declared: re.Match[str]) -> FakeColumn:
    precision, scale = declared.groups()
    return FakeColumn(
        name=name,
        data_type="NUMBER",
        numeric_precision=int(precision),
        numeric_scale=int(scale),
        character_maximum_length=None,
    )


def _column(column: dict[str, str]) -> FakeColumn:
    """The warehouse column a declared column lists as, its type spelled as Snowflake lists it."""

    declared: str = column["data_type"].upper()
    number: re.Match[str] | None = _NUMBER_TYPE.fullmatch(declared)
    builders: dict[bool, Callable[[], FakeColumn]] = {
        True: lambda: _number_column(column["name"], cast(re.Match[str], number)),
        False: lambda: FakeColumn(
            name=column["name"],
            data_type=_LISTED_TYPES.get(declared, declared),
            character_maximum_length=_TEXT_LENGTHS.get(declared),
        ),
    }
    return builders[number is not None]()


def _relation(record: dict[str, Any], drifted: frozenset[str]) -> FakeRelation:
    location: str = f"{record['state_schema']}.{record['name']}"
    columns: tuple[FakeColumn, ...] = tuple(map(_column, record["columns"]))
    return FakeRelation(
        database=record["state_database"],
        schema=record["state_schema"],
        name=record["name"],
        table_type=_RELATION_TYPES[record["relation_type"]],
        columns=columns + {True: (_DRIFT_COLUMN,), False: ()}[location in drifted],
    )


def offline_relations(catalog: list[dict[str, Any]]) -> tuple[FakeRelation, ...]:
    """The synced catalog's relations, every `DRIFT_EVERY`th source with an undeclared column."""

    sources: list[str] = sorted(
        f"{record['state_schema']}.{record['name']}"
        for record in filter(
            lambda record: (record["state_database"], record["state_schema"]) != _LIVE_TARGET,
            catalog,
        )
    )
    drifted: frozenset[str] = frozenset(sources[::DRIFT_EVERY])
    return tuple(_relation(record, drifted) for record in catalog)


def live_plan(project: Path, catalog: list[dict[str, Any]]) -> LivePlan:
    """Compile and plan `project` in this process, as `sqb plan` does, against `catalog`."""

    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=offline_relations(catalog), current_database=_LIVE_TARGET[0]
    )
    _ = native_module.compile_stage_work_info(reset=True)
    started: float = time.monotonic()
    inputs: Any = discover_project_inputs(project_dir=project)
    result: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=inputs,
        adapter=OfflineSnowflakeAdapter(warehouse=warehouse),
        options=CompilePipelineOptions(
            no_cache=True, connection_config=dict(inputs.project_config.connection)
        ),
        hooks=ConnectionHooks(),
    )
    seconds: float = time.monotonic() - started
    stages: dict[str, int] = native_module.compile_stage_work_info()
    return LivePlan(
        seconds=seconds,
        model_entries=len(result.plan_output.model_entries),
        source_rebinding_analyses=stages.get("source_rebinding_analyses", 0),
        cursor_relations=len(offline_cursor_bound_relations(warehouse)),
        cursor_statements=len(warehouse.queries_of_kind("cursor_bounds")),
        warehouse_statements=len(warehouse.queries),
    )
