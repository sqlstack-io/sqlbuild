"""Offline Snowflake build harness whose catalog follows the DDL the build issues.

Data statements are accepted and ignored; only relation existence and columns are simulated, so
metadata round trips during a real compile, plan, and build can be counted without a warehouse.
"""

from __future__ import annotations

import re
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    discover_project_inputs,
    run_cli,
    run_compile_pipeline,
)
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_connection import _SnowflakeConnection
from sqlbuild.executor.build.models import BuildExecutionResult, BuildRuntimeParams
from sqlbuild.executor.pipeline.main.run import run_build_pipeline
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.models import ConnectionHooks
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeColumn,
    FakeRelation,
    FakeSnowflakeProgrammingError,
    OfflineSnowflakeAdapter,
    RecordingSnowflakeWarehouse,
)

_NAME: str = r'(?P<{group}>(?:"[^"]*"|[A-Za-z0-9_$]+)(?:\.(?:"[^"]*"|[A-Za-z0-9_$]+)){{0,2}})'
_CREATE_AS_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:TRANSIENT\s+|TEMPORARY\s+)?(?P<kind>TABLE|VIEW)\s+"
    + _NAME.format(group="name")
    + r"\s+AS\s+(?P<query>.*)$",
    re.IGNORECASE | re.DOTALL,
)
_CREATE_DEFINED_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:TRANSIENT\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?"
    + _NAME.format(group="name")
    + r"\s*\(",
    re.IGNORECASE | re.DOTALL,
)
_DROP_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*DROP\s+(?P<kind>TABLE|VIEW)\s+(?:IF\s+EXISTS\s+)?" + _NAME.format(group="name") + r"\s*$",
    re.IGNORECASE,
)
_RENAME_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*ALTER\s+(?:TABLE|VIEW)\s+"
    + _NAME.format(group="name")
    + r"\s+RENAME\s+TO\s+"
    + _NAME.format(group="other")
    + r"\s*$",
    re.IGNORECASE,
)
_SWAP_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*ALTER\s+TABLE\s+"
    + _NAME.format(group="name")
    + r"\s+SWAP\s+WITH\s+"
    + _NAME.format(group="other")
    + r"\s*$",
    re.IGNORECASE,
)
_ADD_COLUMN_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*ALTER\s+TABLE\s+"
    + _NAME.format(group="name")
    + r'\s+ADD\s+COLUMN\s+"?(?P<column>[A-Za-z0-9_]+)"?\s+(?P<type>.+?)\s*$',
    re.IGNORECASE,
)
_SET_RETENTION_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*ALTER\s+TABLE\s+"
    + _NAME.format(group="name")
    + r"\s+SET\s+DATA_RETENTION_TIME_IN_DAYS\s*=\s*(?P<days>\d+)\s*$",
    re.IGNORECASE,
)
_CREATE_SCHEMA_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*CREATE\s+SCHEMA\s+IF\s+NOT\s+EXISTS\s+" + _NAME.format(group="name") + r"\s*$",
    re.IGNORECASE,
)
_DESCRIBE_QUERY_PATTERN: re.Pattern[str] = re.compile(
    r"^SELECT \* FROM \(SELECT (?P<columns>.+?) FROM \S+\) AS __describe_source LIMIT 0$",
    re.DOTALL,
)
_RUNTIME_BOUNDS_PATTERN: re.Pattern[str] = re.compile(r"^SELECT MIN\([^)]*\), MAX\([^)]*\) FROM ")
_COPY_SOURCE_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*SELECT\s+\*\s+FROM\s+" + _NAME.format(group="source") + r"\s*$", re.IGNORECASE
)
_RETENTION_PATTERN: re.Pattern[str] = re.compile(r"DATA_RETENTION_TIME_IN_DAYS = (?P<days>\d+)")
_DEFAULT_COLUMNS: tuple[FakeColumn, ...] = (FakeColumn(name="ID", data_type="NUMBER"),)
_STATUS_DESCRIPTION: tuple[tuple[str], ...] = (("status",),)
_SHIFTS_PROJECT: str = (
    'name = "catalog"\nadapter = "snowflake"\n\n[connection]\n'
    'account = "example"\nuser = "example"\ndatabase = "analytics"\nschema = "marts"\n\n'
    '[defaults]\ndatabase = "analytics"\nschema = "marts"\nmaterialized = "table"\n'
)
_SHIFTS_MODEL: str = (
    "MODEL (description 'Shift report.');\n"
    'SELECT shift_id, starts_at FROM __source("raw_shifts")\n'
)


type _Identity = tuple[str, str, str]
type _DdlAction = Callable[[re.Match[str]], frozenset[_Identity]]


@dataclass
class SimulatedSnowflakeWarehouse(RecordingSnowflakeWarehouse):
    """Recording warehouse whose relations follow CREATE, DROP, RENAME, SWAP, and ADD COLUMN."""

    model_columns: dict[str, tuple[FakeColumn, ...]] = field(default_factory=dict)
    ddl_events: list[tuple[int, frozenset[_Identity]]] = field(default_factory=list)
    max_retention_days: int = 90
    # Changes another session makes just before a matching statement runs.
    external_changes: list[
        tuple[re.Pattern[str], Callable[[SimulatedSnowflakeWarehouse], None]]
    ] = field(default_factory=list)

    def reset(self) -> None:
        """Forget recorded statements and simulated DDL events, keeping the catalog."""

        super().reset()
        with self._lock:
            self.ddl_events.clear()

    def set_model_columns(self, *, model: str, columns: tuple[FakeColumn, ...]) -> None:
        """Change the columns a model's CREATE ... AS statements produce."""

        with self._lock:
            self.model_columns[model.upper()] = columns

    def _answer(self, *, sql: str, params: tuple[object, ...]) -> tuple[list[tuple[Any, ...]], Any]:
        external: tuple[re.Pattern[str], Callable[[SimulatedSnowflakeWarehouse], None]]
        for external in tuple(
            filter(lambda item: item[0].search(sql) is not None, self.external_changes)
        ):
            self.external_changes.remove(external)
            external[1](self)
        retention: re.Match[str]
        for retention in filter(None, (_RETENTION_PATTERN.search(sql),)):
            _RETENTION_CHECKS[int(retention.group("days")) > self.max_retention_days](
                retention.group("days")
            )
        routes: tuple[tuple[re.Pattern[str], _DdlAction], ...] = (
            (_CREATE_AS_PATTERN, self._create_as),
            (_CREATE_DEFINED_PATTERN, self._create_defined),
            (_DROP_PATTERN, self._drop),
            (_RENAME_PATTERN, self._rename),
            (_SWAP_PATTERN, self._swap),
            (_ADD_COLUMN_PATTERN, self._add_column),
            (_SET_RETENTION_PATTERN, self._set_retention),
            (_CREATE_SCHEMA_PATTERN, self._create_schema),
        )
        pattern: re.Pattern[str]
        apply: _DdlAction
        for pattern, apply in routes:
            match: re.Match[str]
            for match in filter(None, (pattern.match(sql),)):
                with self._lock:
                    changed: frozenset[_Identity] = apply(match)
                    self.ddl_events.append((len(self.attempted_sql) - 1, changed))
                return [], _STATUS_DESCRIPTION
        described: tuple[re.Match[str], ...] = tuple(
            filter(None, (_DESCRIBE_QUERY_PATTERN.match(sql),))
        )
        description: re.Match[str]
        for description in described:
            return [], tuple(
                (column.strip().strip('"'),) for column in description.group("columns").split(",")
            )
        bounds: tuple[re.Match[str], ...] = tuple(
            filter(None, (_RUNTIME_BOUNDS_PATTERN.match(sql),))
        )
        for _ in bounds:
            return [self.cursor_values], (("min",), ("max",))
        return super()._answer(sql=sql, params=params)

    def _create_as(self, match: re.Match[str]) -> frozenset[_Identity]:
        identity: _Identity = self._parts(match.group("name"))
        self._require_kind(identity=identity, kind=match.group("kind"), error=_already_exists)
        copied: tuple[re.Match[str], ...] = tuple(
            filter(None, (_COPY_SOURCE_PATTERN.match(match.group("query")),))
        )
        sources: frozenset[_Identity] = frozenset(
            self._parts(copy.group("source")) for copy in copied
        )
        source_columns: tuple[tuple[FakeColumn, ...], ...] = tuple(
            fake.columns for fake in filter(lambda fake: _identity(fake) in sources, self.relations)
        )
        columns: tuple[FakeColumn, ...] = next(
            iter(source_columns), self._model_columns_for(name=identity[2])
        )
        table_type: str = ("BASE TABLE", "VIEW")[match.group("kind").upper() == "VIEW"]
        self._put(
            relation=FakeRelation(
                database=identity[0],
                schema=identity[1],
                name=identity[2],
                table_type=table_type,
                is_transient=True,
                columns=columns,
            )
        )
        return frozenset({identity})

    def _create_defined(self, match: re.Match[str]) -> frozenset[_Identity]:
        identity: _Identity = self._parts(match.group("name"))
        existing: tuple[FakeRelation, ...] = tuple(
            filter(lambda fake: _identity(fake) == identity, self.relations)
        )
        self._put(
            relation=next(
                iter(existing),
                FakeRelation(
                    database=identity[0],
                    schema=identity[1],
                    name=identity[2],
                    is_transient=True,
                    columns=_DEFAULT_COLUMNS,
                ),
            )
        )
        return frozenset({identity})

    def _drop(self, match: re.Match[str]) -> frozenset[_Identity]:
        identity: _Identity = self._parts(match.group("name"))
        self._require_kind(identity=identity, kind=match.group("kind"), error=_wrong_type)
        self.replace_relations(
            relations=tuple(filter(lambda fake: _identity(fake) != identity, self.relations))
        )
        return frozenset({identity})

    def _rename(self, match: re.Match[str]) -> frozenset[_Identity]:
        origin: _Identity = self._parts(match.group("name"))
        destination: _Identity = self._parts(match.group("other"))
        fake: FakeRelation
        for fake in filter(lambda candidate: _identity(candidate) == destination, self.relations):
            _already_exists(destination[2], ("TABLE", "VIEW")[fake.table_type == "VIEW"])
        self.replace_relations(
            relations=tuple(
                (fake, _moved(fake=fake, identity=destination))[_identity(fake) == origin]
                for fake in self.relations
            )
        )
        return frozenset({origin, destination})

    def _swap(self, match: re.Match[str]) -> frozenset[_Identity]:
        left: _Identity = self._parts(match.group("name"))
        right: _Identity = self._parts(match.group("other"))
        identity: _Identity
        for identity in (left, right):
            missing: _Identity
            for missing in filter(
                lambda wanted: all(_identity(fake) != wanted for fake in self.relations),
                (identity,),
            ):
                _does_not_exist(missing[2])
            self._require_kind(identity=identity, kind="TABLE", error=_wrong_type)
        targets: dict[_Identity, _Identity] = {left: right, right: left}
        self.replace_relations(
            relations=tuple(
                _moved(fake=fake, identity=targets.get(_identity(fake), _identity(fake)))
                for fake in self.relations
            )
        )
        return frozenset({left, right})

    def _add_column(self, match: re.Match[str]) -> frozenset[_Identity]:
        identity: _Identity = self._parts(match.group("name"))
        added: FakeColumn = FakeColumn(name=match.group("column").upper(), data_type="TEXT")
        self.replace_relations(
            relations=tuple(
                (fake, replace(fake, columns=(*fake.columns, added)))[_identity(fake) == identity]
                for fake in self.relations
            )
        )
        return frozenset({identity})

    def _set_retention(self, match: re.Match[str]) -> frozenset[_Identity]:
        identity: _Identity = self._parts(match.group("name"))
        days: int = int(match.group("days"))
        self.replace_relations(
            relations=tuple(
                (fake, replace(fake, retention_time=days))[_identity(fake) == identity]
                for fake in self.relations
            )
        )
        return frozenset({identity})

    def _require_kind(
        self, *, identity: _Identity, kind: str, error: Callable[[str, str], None]
    ) -> None:
        """Fail like Snowflake when the object at ``identity`` is not of ``kind``."""

        found: str
        for found in filter(
            lambda listed: listed != kind.upper(),
            (
                ("TABLE", "VIEW")[fake.table_type == "VIEW"]
                for fake in filter(
                    lambda candidate: _identity(candidate) == identity, self.relations
                )
            ),
        ):
            error(identity[2], found)

    def _create_schema(self, match: re.Match[str]) -> frozenset[_Identity]:
        parts: tuple[str, ...] = tuple(
            part.strip('"').upper() for part in match.group("name").split(".")
        )
        scope: tuple[str, ...] = (self.current_database, *parts)[-2:]
        self.extra_schemas = self.extra_schemas | {(scope[0], scope[1])}
        return frozenset({(scope[0], scope[1], "")})

    def _put(self, *, relation: FakeRelation) -> None:
        identity: _Identity = _identity(relation)
        self.replace_relations(
            relations=(*filter(lambda fake: _identity(fake) != identity, self.relations), relation)
        )

    def _model_columns_for(self, *, name: str) -> tuple[FakeColumn, ...]:
        candidates: tuple[str, ...] = tuple(
            filter(lambda model: _model_owns(model=model, name=name), self.model_columns)
        )
        models: list[str] = sorted(candidates, key=_name_length, reverse=True)
        return next((self.model_columns[model] for model in models), _DEFAULT_COLUMNS)

    def _parts(self, name: str) -> _Identity:
        parts: tuple[str, ...] = tuple(part.strip('"').upper() for part in name.split("."))
        full: tuple[str, ...] = (self.current_database, self.current_database, *parts)[-3:]
        return full[0], full[1], full[2]


def _does_not_exist(name: str) -> None:
    raise FakeSnowflakeProgrammingError(
        f"002003 (02000): SQL compilation error:\nTable '{name}' does not exist or not authorized.",
        errno=2003,
    )


def _already_exists(name: str, found: str) -> None:
    raise FakeSnowflakeProgrammingError(
        f"002002 (42710): SQL compilation error:\nObject '{name}' already exists as {found}",
        errno=2002,
    )


def _wrong_type(name: str, found: str) -> None:
    wanted: str = ("VIEW", "TABLE")[found == "VIEW"]
    raise FakeSnowflakeProgrammingError(
        f"002203 (02000): SQL compilation error: Object found is of type '{found}', "
        f"not specified type '{wanted}'. ({name})",
        errno=2203,
    )


def _accept_retention(days: str) -> None:
    del days


def _reject_retention(days: str) -> None:
    raise FakeSnowflakeProgrammingError(
        "001008 (22023): SQL compilation error:\n"
        f"invalid value [{days}] for parameter 'DATA_RETENTION_TIME_IN_DAYS'",
        errno=1008,
    )


_RETENTION_CHECKS: dict[bool, Callable[[str], None]] = {
    False: _accept_retention,
    True: _reject_retention,
}


def _name_length(name: str) -> int:
    return len(name)


def _model_owns(*, model: str, name: str) -> bool:
    return name == model or name.startswith(f"{model}__")


def _identity(fake: FakeRelation) -> _Identity:
    return fake.database, fake.schema, fake.name


def _moved(*, fake: FakeRelation, identity: _Identity) -> FakeRelation:
    return replace(fake, database=identity[0], schema=identity[1], name=identity[2])


@dataclass(frozen=True)
class OfflineBuild:
    """One compiled, planned, and executed offline Snowflake build."""

    result: BuildExecutionResult
    statements: tuple[str, ...]
    ddl_events: tuple[tuple[int, frozenset[_Identity]], ...]


def build_offline_snowflake_project(
    *, project_dir: Path, warehouse: SimulatedSnowflakeWarehouse, max_concurrency: int
) -> OfflineBuild:
    """Compile, plan, and build ``project_dir`` through the build pipeline, offline."""

    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    config: dict[str, object] = dict(inputs.project_config.connection)
    planned: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=inputs,
        adapter=adapter,
        options=CompilePipelineOptions(
            no_sql_validation=True, no_cache=True, connection_config=config
        ),
        hooks=ConnectionHooks(),
    )
    warehouse.reset()
    result: BuildExecutionResult = run_build_pipeline(
        plan=planned.plan_output,
        connection_config=config,
        adapter=adapter,
        settings=inputs.project_config.settings,
        runtime=BuildRuntimeParams(
            run_id="offline_build",
            runtime_dir=project_dir / "target",
            promotion_mode=TablePromotionMode(adapter.default_table_promotion_mode()),
            max_concurrency=max_concurrency,
        ),
    )
    return OfflineBuild(
        result=result,
        statements=tuple(warehouse.attempted_sql),
        ddl_events=tuple(warehouse.ddl_events),
    )


def state_table_creates(build: OfflineBuild) -> tuple[str, ...]:
    """Return the CREATE statements the build attempted for SQLBuild state tables."""

    return tuple(
        filter(
            lambda statement: _STATE_TABLE_CREATE_PATTERN.match(statement) is not None,
            build.statements,
        )
    )


def information_schema_statements(build: OfflineBuild) -> tuple[str, ...]:
    """Return build statements that read INFORMATION_SCHEMA, which needs a running warehouse."""

    return tuple(
        filter(
            lambda statement: _INFORMATION_SCHEMA_PATTERN.search(statement) is not None,
            build.statements,
        )
    )


def column_reads_by_relation(build: OfflineBuild) -> dict[str, int]:
    """Count SHOW COLUMNS reads per relation the build inspected."""

    reads: tuple[re.Match[str], ...] = tuple(
        filter(None, (_SHOW_COLUMNS_RELATION.match(statement) for statement in build.statements))
    )
    return dict(Counter(read.group(1).replace('"', "") for read in reads))


def repeated_metadata_reads(build: OfflineBuild) -> tuple[str, ...]:
    """Return metadata statements re-issued with no DDL on their relation since the last issue."""

    last_read: dict[str, int] = {}
    repeated: list[str] = []
    index: int
    statement: str
    for index, statement in enumerate(build.statements):
        identities: tuple[_Identity, ...] = _metadata_identities(statement)
        identity: _Identity
        for identity in identities:
            previous: int = last_read.get(statement, -1)
            changed: bool = any(
                previous < event_index < index and identity in names
                for event_index, names in build.ddl_events
            )
            repeated.extend(statement for _ in range(int(previous >= 0 and not changed)))
            last_read[statement] = index
    return tuple(repeated)


def _metadata_identities(statement: str) -> tuple[_Identity, ...]:
    identities: list[_Identity] = []
    pattern: re.Pattern[str]
    to_identity: Callable[[re.Match[str]], _Identity]
    for pattern, to_identity in _METADATA_STATEMENTS:
        identities.extend(map(to_identity, filter(None, (pattern.match(statement),))))
    return tuple(identities)


def _show_columns_identity(match: re.Match[str]) -> _Identity:
    parts: list[str] = match.group("relation").replace('"', "").split(".")
    return parts[0], parts[1], parts[2]


def _show_relation_identity(match: re.Match[str]) -> _Identity:
    return match.group("database"), match.group("schema"), match.group("name")


def _show_schema_identity(match: re.Match[str]) -> _Identity:
    return match.group("database"), match.group("schema"), ""


_STATE_TABLE_CREATE_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*CREATE\s+(?:TRANSIENT\s+)?TABLE\s+IF\s+NOT\s+EXISTS\s+\S*_sqlbuild_", re.IGNORECASE
)
_INFORMATION_SCHEMA_PATTERN: re.Pattern[str] = re.compile(r"information_schema", re.IGNORECASE)
_SHOW_COLUMNS_RELATION: re.Pattern[str] = re.compile(r"^SHOW COLUMNS IN (?:TABLE|VIEW) (\S+)$")
_METADATA_STATEMENTS: tuple[tuple[re.Pattern[str], Callable[[re.Match[str]], _Identity]], ...] = (
    (
        re.compile(r"^SHOW COLUMNS IN (?:TABLE|VIEW) (?P<relation>\S+)$"),
        _show_columns_identity,
    ),
    (
        re.compile(
            r"^SHOW TERSE (?:TABLES|VIEWS) LIKE '(?P<name>[^']*)' "
            r'IN SCHEMA "(?P<database>[^"]*)"\."(?P<schema>[^"]*)"$'
        ),
        _show_relation_identity,
    ),
    (
        re.compile(r"^SHOW SCHEMAS LIKE '(?P<schema>[^']*)' IN DATABASE \"(?P<database>[^\"]*)\"$"),
        _show_schema_identity,
    ),
)


ORDER_COLUMNS: tuple[FakeColumn, ...] = (
    FakeColumn(name="ORDER_ID", data_type="NUMBER", numeric_precision=38, numeric_scale=0),
    FakeColumn(name="ORDERED_AT", data_type="TIMESTAMP_NTZ", character_maximum_length=None),
    FakeColumn(name="AMOUNT", data_type="NUMBER", numeric_precision=10, numeric_scale=2),
)
ORDER_MODELS: tuple[str, ...] = ("ORDERS_TABLE", "ORDERS_VIEW", "ORDERS_MERGE", "ORDERS_DAILY")
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_platform"\n'
        'adapter = "snowflake"\n\n'
        "[connection]\n"
        'account = "example"\n'
        'user = "example"\n'
        'database = "analytics"\n'
        'schema = "marts"\n\n'
        "[connection.session_parameters]\n"
        "QUOTED_IDENTIFIERS_IGNORE_CASE = true\n\n"
        "[defaults]\n"
        'database = "analytics"\n'
        'materialized = "table"\n\n'
        "[path_defaults.marts]\n"
        'schema = "marts"\n'
    ),
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    database: analytics\n"
        "    schema: raw\n"
        "    table: orders\n"
        "    columns:\n"
        "      - name: order_id\n"
        "        type: NUMBER(38, 0)\n"
        "      - name: ordered_at\n"
        "        type: TIMESTAMP_NTZ\n"
        "      - name: amount\n"
        "        type: NUMBER(10, 2)\n"
    ),
    "models/marts/orders_table.sql": (
        "MODEL (description 'Test model orders_table.',\n  materialized table,\n);\n\n"
        'SELECT order_id, ordered_at, amount FROM __source("raw_orders")\n'
    ),
    "models/marts/orders_view.sql": (
        'MODEL (description "Test model orders_view.",'
        '\n  materialized view,\n);\n\nSELECT order_id, amount FROM __ref("orders_table")\n'
    ),
    "models/marts/orders_merge.sql": (
        "MODEL (description 'Test model orders_merge.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy merge,\n"
        "  unique_key [order_id],\n"
        "  cursor ordered_at,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        "  cursor_inputs (\n    raw_orders ordered_at,\n  ),\n"
        "  on_schema_change append_new_columns,\n"
        ");\n\n"
        'SELECT order_id, ordered_at, amount FROM __source("raw_orders")\n'
    ),
    "models/marts/orders_daily.sql": (
        "MODEL (description 'Test model orders_daily.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  incremental_mode microbatch,\n"
        "  microbatch_strategy watermark,\n"
        "  cursor ordered_at,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        "  cursor_start '2026-01-01',\n"
        "  cursor_end '2026-01-04',\n"
        "  cursor_watermark_mode all,\n"
        "  cursor_inputs (\n    raw_orders (column ordered_at, roles [filter, watermark]),\n  ),\n"
        "  batch_size 1d,\n"
        ");\n\n"
        'SELECT order_id, ordered_at, amount FROM __source("raw_orders")\n'
    ),
}


def write_bundles_project(*, project_dir: Path, select: str) -> None:
    """Write one table model selecting ``select`` from a source without declared columns."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "catalog"\nadapter = "snowflake"\n\n[connection]\n'
            'account = "example"\nuser = "example"\ndatabase = "analytics"\nschema = "marts"\n\n'
            '[defaults]\ndatabase = "analytics"\nschema = "marts"\nmaterialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_bundles\n    description: Test source raw_bundles.\n"
            "    database: analytics\n    schema: raw\n    table: bundles\n"
        ),
        "models/bundle_report.sql": (
            "MODEL (description 'Test model bundle_report.');\n"
            f'SELECT {select} FROM __source("raw_bundles")\n'
        ),
    }
    relative: str
    content: str
    for relative, content in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")


def bundles_warehouse() -> SimulatedSnowflakeWarehouse:
    """Return a simulated warehouse holding the raw bundles source with upper-case columns."""

    return SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(
                database="ANALYTICS",
                schema="RAW",
                name="BUNDLES",
                columns=(
                    FakeColumn(name="BUNDLE_SKU", data_type="TEXT"),
                    FakeColumn(name="QUANTITY", data_type="NUMBER"),
                ),
            ),
        )
    )


def plan_offline_snowflake_project(
    *, project_dir: Path, warehouse: SimulatedSnowflakeWarehouse
) -> CompilePipelineResult:
    """Compile and plan ``project_dir`` with SQL validation against the offline warehouse."""

    inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    return run_compile_pipeline(
        discovered_inputs=inputs,
        adapter=OfflineSnowflakeAdapter(warehouse=warehouse),
        options=CompilePipelineOptions(
            no_cache=True, connection_config=dict(inputs.project_config.connection)
        ),
        hooks=ConnectionHooks(),
    )


def write_orders_project(*, project_dir: Path) -> None:
    """Write a table, a view, a merge incremental, and a three-day microbatch model."""

    relative: str
    content: str
    for relative, content in _PROJECT_FILES.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")


def orders_warehouse() -> SimulatedSnowflakeWarehouse:
    """Return a simulated warehouse holding the raw orders source."""

    warehouse: SimulatedSnowflakeWarehouse = SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(database="ANALYTICS", schema="RAW", name="ORDERS", columns=ORDER_COLUMNS),
        ),
        cursor_values=("2026-01-01 00:00:00.000", "2026-01-03 12:00:00.000"),
    )
    model: str
    for model in ORDER_MODELS:
        warehouse.set_model_columns(model=model, columns=ORDER_COLUMNS)
    return warehouse


def failed_models(build: OfflineBuild) -> tuple[str, ...]:
    """Return models that did not build successfully."""

    return tuple(
        result.model_name
        for result in filter(
            lambda result: result.status != ExecutionStatus.SUCCESS, build.result.model_results
        )
    )


def relation_types(warehouse: SimulatedSnowflakeWarehouse, name: str) -> tuple[str, ...]:
    """The table types of the warehouse's relations stored as ``name``."""

    return tuple(
        relation.table_type
        for relation in filter(lambda candidate: candidate.name == name, warehouse.relations)
    )


def retention_by_prefix(warehouse: SimulatedSnowflakeWarehouse, prefix: str) -> dict[str, int]:
    """The retention of each relation whose stored name starts with ``prefix``."""

    return {
        relation.name: relation.retention_time
        for relation in filter(
            lambda candidate: candidate.name.startswith(prefix), warehouse.relations
        )
    }


def matching_statements(statements: tuple[str, ...], pattern: re.Pattern[str]) -> tuple[str, ...]:
    """The statements ``pattern`` matches at their start."""

    return tuple(filter(lambda statement: pattern.match(statement) is not None, statements))


def drop_external_relation(warehouse: SimulatedSnowflakeWarehouse, name: str) -> None:
    """Drop a relation the way another session would, outside the build's statements."""

    warehouse.replace_relations(
        relations=tuple(filter(lambda relation: relation.name != name, warehouse.relations))
    )


def create_external_table(warehouse: SimulatedSnowflakeWarehouse, name: str) -> None:
    """Create a table in the marts schema the way another session would."""

    warehouse.replace_relations(
        relations=(
            *warehouse.relations,
            FakeRelation(database="ANALYTICS", schema="MARTS", name=name),
        )
    )


def write_model_copies(
    *, project_dir: Path, count: int, retention_days: int, header_options: str = ""
) -> None:
    """Add ``count`` permanent table copies of the orders table, with ``header_options``."""

    project: Path = project_dir / "sqlbuild_project.toml"
    _ = project.write_text(
        project.read_text(encoding="utf-8")
        + '\n[materialization_defaults.table]\ntable_type = "permanent"\n'
        + f'time_travel_retention = "{retention_days}d"\n',
        encoding="utf-8",
    )
    index: int
    for index in range(count):
        _ = (project_dir / "models" / "marts" / f"order_copy_{index}.sql").write_text(
            f"MODEL ({header_options}description 'Copy {index} of the orders.', "
            "materialized table);\n\n"
            'SELECT order_id, ordered_at, amount FROM __ref("orders_table")\n',
            encoding="utf-8",
        )


def build_with_cli(
    *, project_dir: Path, warehouse: SimulatedSnowflakeWarehouse, monkeypatch: pytest.MonkeyPatch
) -> tuple[int, tuple[str, ...]]:
    """Run the real ``sqb build`` against the simulated warehouse; its exit code and statements."""

    monkeypatch.setattr(
        SnowflakeAdapter,
        "connect",
        lambda _adapter, _config: _SnowflakeConnection(warehouse.raw_connection()),
    )
    warehouse.reset()
    exit_code: int = run_cli(["--project-dir", str(project_dir), "--no-color", "build"])
    return exit_code, tuple(warehouse.attempted_sql)


_SCHEMA_LISTING_STATEMENT: re.Pattern[str] = re.compile(
    r"^SHOW (?P<kind>TABLES|VIEWS) IN SCHEMA (?P<scope>\S+)"
)


def schema_listing_counts(statements: tuple[str, ...]) -> dict[str, int]:
    """How often each schema was listed, keyed by ``TABLES``/``VIEWS`` and the schema."""

    searches: list[re.Match[str] | None] = [
        _SCHEMA_LISTING_STATEMENT.search(statement) for statement in statements
    ]
    matches: list[re.Match[str]] = list(filter(None, searches))
    return dict(Counter(f"{match['kind']} {match['scope']}" for match in matches))


def quoted_configured_column_files(
    *, unique_key: str, updated_at: str, cursor: str
) -> dict[str, str]:
    """A snapshot and a microbatch model over listings stored with quoted camelCase columns."""

    return {
        "sqlbuild_project.toml": (
            'name = "partner_feed"\nadapter = "snowflake"\n\n'
            '[connection]\naccount = "example"\nuser = "example"\n'
            'database = "analytics"\nschema = "marts"\n\n'
            '[defaults]\ndatabase = "analytics"\nschema = "marts"\nmaterialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_listings\n    description: Partner listings in quoted camelCase.\n"
            "    database: analytics\n    schema: raw\n    table: listings\n"
            "  - name: raw_listing_events\n    description: Partner listing events.\n"
            "    database: analytics\n    schema: raw\n    table: listing_events\n"
        ),
        "models/listing_history.sql": (
            "MODEL (\n  description 'Listing price history.',\n  materialized snapshot,\n"
            f"  unique_key [{unique_key}],\n  snapshot_strategy timestamp,\n"
            f"  updated_at {updated_at},\n);\n\n"
            'SELECT "listingId", "unitPrice", "updatedAt" FROM __source("raw_listings")\n'
        ),
        "models/listing_activity.sql": (
            "MODEL (\n  description 'Listing events by hour.',\n  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n  incremental_mode microbatch,\n"
            "  microbatch_strategy watermark,\n  cursor_watermark_mode all,\n"
            f"  cursor {cursor},\n  cursor_type timestamp,\n  cursor_grain day,\n"
            "  cursor_start '2026-01-01',\n  cursor_end '2026-01-03',\n"
            "  cursor_inputs (\n"
            "    raw_listing_events (column '\"eventAt\"', roles [filter, watermark]),\n  ),\n"
            "  batch_size 1d,\n);\n\n"
            'SELECT "listingId", "eventAt" FROM __source("raw_listing_events")\n'
        ),
    }


def shifts_warehouse(warehouse_type: str) -> SimulatedSnowflakeWarehouse:
    """Return a simulated warehouse holding a shifts source whose start column has a type."""

    return SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(
                database="ANALYTICS",
                schema="RAW",
                name="SHIFTS",
                columns=(
                    FakeColumn(
                        name="SHIFT_ID", data_type="NUMBER", numeric_precision=38, numeric_scale=0
                    ),
                    FakeColumn(name="STARTS_AT", data_type=warehouse_type),
                ),
            ),
        )
    )


def write_shifts_project(*, project_dir: Path, declared_type: str) -> None:
    """Write one model reading a shifts source whose enforced contract declares a start type."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _SHIFTS_PROJECT,
        "sources/raw.yml": (
            "sources:\n  - name: raw_shifts\n    description: Shift rows.\n"
            "    database: analytics\n    schema: raw\n    table: shifts\n"
            "    contract: enforced\n    columns:\n"
            "      - name: shift_id\n        type: NUMBER(38,0)\n"
            f"      - name: starts_at\n        type: {declared_type}\n"
        ),
        "models/shift_report.sql": _SHIFTS_MODEL,
    }
    relative: str
    content: str
    for relative, content in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")
