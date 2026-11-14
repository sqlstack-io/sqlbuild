from dataclasses import dataclass

from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo


@dataclass(frozen=True)
class SqliteQualifiedNameTestCase:
    description: str
    schema: str | None
    name: str
    expected_name: str | None


@dataclass(frozen=True)
class SqliteRejectedNameTestCase:
    description: str
    database: str | None
    schema: str
    expected_code: str


@dataclass(frozen=True)
class SqliteConnectErrorTestCase:
    description: str
    config: dict[str, object]
    expected_code: str


@dataclass(frozen=True)
class SqliteNamespaceListingTestCase:
    description: str
    schemas: tuple[str, ...] | None
    expected_relations: tuple[RelationInfo, ...]


@dataclass(frozen=True)
class SqliteTypedTableTestCase:
    description: str
    select_sql: str
    config: dict[str, object]
    expected_columns: tuple[ColumnInfo, ...]
    expected_rows: tuple[tuple[object, ...], ...]
    setup_statements: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqliteStrategyTestCase:
    description: str
    strategy: str
    source_sql: str
    select_columns: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteAtomicityTestCase:
    description: str
    strategy: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteSwapTestCase:
    description: str
    expected_view_rows: tuple[tuple[object, ...], ...]
    expected_dependent_views: tuple[str, ...]


@dataclass(frozen=True)
class SqliteSchemaEvolutionTestCase:
    description: str
    operation: str
    expected_columns: tuple[ColumnInfo, ...]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteSeedTestCase:
    description: str
    csv_contents: str
    columns: tuple[ColumnInfo, ...]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteStateTableTestCase:
    description: str
    schema: str
    expected_tables: tuple[str, ...]
    expected_index_count: int


@dataclass(frozen=True)
class SqliteDiffTestCase:
    description: str
    right_sql: str
    expected_counts: tuple[int, int, int, int, int]
    expected_changed_columns: tuple[str, ...]


@dataclass(frozen=True)
class SqliteViewRenameTestCase:
    description: str
    expected_definition: str
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class SqliteLockTestCase:
    description: str
    busy_timeout: float
    hold_seconds: float
    expected_code: str
    expected_reader_rows: int
    max_wait_seconds: float


@dataclass(frozen=True)
class SqliteInProcessWritersTestCase:
    description: str
    busy_timeout: float
    hold_seconds: float
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteConcurrentConnectTestCase:
    description: str
    connections: int
    attempts: int
    expected_open_connections: int


@dataclass(frozen=True)
class SqliteKilledWriterTestCase:
    description: str
    expected_rows: tuple[tuple[object, ...], ...]
    expected_tables: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteVersionTestCase:
    description: str
    linked_version: tuple[int, int, int]
    expected_code: str


@dataclass(frozen=True)
class SqlitePragmaTestCase:
    description: str
    pragmas: tuple[str, ...]
    expected_values: tuple[object, ...]


@dataclass(frozen=True)
class SqliteCastTestCase:
    description: str
    target_type: str
    expected_sql: str


@dataclass(frozen=True)
class SqliteCollisionTestCase:
    description: str
    first: tuple[str, str]
    second: tuple[str, str]
    expected_code: str


@dataclass(frozen=True)
class SqliteJanitorViewRenameTestCase:
    description: str
    expected_relations: tuple[RelationInfo, ...]
    expected_registry_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteDecimalDiffTestCase:
    description: str
    left_sql: str
    right_sql: str
    expected_counts: tuple[int, int, int, int]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteInterruptTestCase:
    description: str
    interrupt_after_seconds: float
    expected_error_fragment: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteMergeCountTestCase:
    description: str
    source_sql: str
    expected_affected: int


@dataclass(frozen=True)
class SqliteReadOnlyPathTestCase:
    description: str
    file_name: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteRetypeIndexTestCase:
    description: str
    expected_indexes: tuple[tuple[object, ...], ...]
    expected_columns: tuple[ColumnInfo, ...]


@dataclass(frozen=True)
class SqliteUnkeyedSampleTestCase:
    description: str
    left_sql: str
    limit: int
    expected_row_count: int
    expected_order_ids: frozenset[object]


@dataclass(frozen=True)
class SqliteStatisticsTestCase:
    description: str
    fixture_name: str
    expected_interactive_row_count: int


@dataclass(frozen=True)
class SqliteCursorRowCountTestCase:
    description: str
    unique_key: tuple[str, ...]
    expected_inserted: int
