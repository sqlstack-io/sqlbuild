from dataclasses import dataclass, field


@dataclass(frozen=True)
class SqliteOracleE2ETestCase:
    description: str
    compared_models: tuple[tuple[str, str], ...]
    unexpected_rebuild_fragments: tuple[str, ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqliteCommandE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_fragments: tuple[str, ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqliteProjectE2ETestCase:
    description: str
    files: dict[str, str]
    changed_files: dict[str, str]
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    absent_tables: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqliteFailureE2ETestCase:
    description: str
    files: dict[str, str]
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqliteLockE2ETestCase:
    description: str
    busy_timeout: float
    hold_seconds: float
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqliteScenarioE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_outcomes: dict[str, tuple[str, str | None]]
    expected_tables: tuple[str, ...]
    expected_return_code: int = 1


@dataclass(frozen=True)
class SqliteDecimalContractE2ETestCase:
    description: str
    source_files: dict[str, str]
    expected_payment_rows: tuple[tuple[object, ...], ...]
    expected_agreeing_rows: int
    expected_evolved_rows: tuple[tuple[object, ...], ...]
    expected_fee_declaration: str


@dataclass(frozen=True)
class SqliteCollisionE2ETestCase:
    description: str
    first_files: dict[str, str]
    second_files: dict[str, str]
    expected_fragments: tuple[str, ...]
    expected_tables: tuple[tuple[object, ...], ...]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteCursorWindowE2ETestCase:
    description: str
    events_csv: str
    commands: tuple[tuple[str, ...], ...]
    expected_order_ids: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SqliteWaffleShopE2ETestCase:
    description: str
    commands: tuple[tuple[str, ...], ...]
    expected_fragments: tuple[str, ...]
    expected_row_counts: dict[str, int]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqliteLoaderE2ETestCase:
    description: str
    expected_fragment: str


@dataclass(frozen=True)
class TextTimestampCursorE2ETestCase:
    """A daily model whose timestamp cursor is a text expression, on one adapter."""

    description: str
    adapter: str
    connection: str
    day_expression: str
    expected_exit_code: int
    expected_fragment: str


@dataclass(frozen=True)
class SqliteTemporalCastE2ETestCase:
    """A SQLite model selecting one expression, and the B104 warnings compile reports for it."""

    description: str
    expression: str
    expected_warning_count: int
