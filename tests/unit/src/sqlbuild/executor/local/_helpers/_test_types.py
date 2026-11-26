from dataclasses import dataclass

from sqlbuild.executor.local.models import MacroPlace


@dataclass(frozen=True)
class ConversionWorkTestCase:
    """Statements checked for a run, and the work each kind of step may take."""

    description: str
    statements: tuple[str, ...]
    repeats: int
    expected_conversions: int
    expected_schema_keys: int
    expected_relations: frozenset[str]


@dataclass(frozen=True)
class PooledConversionWorkTestCase:
    """Enough statements to convert in worker processes, then read again for each test."""

    description: str
    statement_count: int
    expected_conversions: int
    expected_schema_keys: int


@dataclass(frozen=True)
class BindingWorkTestCase:
    """DuckDB statements bound once each against an empty database."""

    description: str
    statements: tuple[str, ...]
    repeats: int
    expected_bindings: int
    expected_missing: tuple[str | None, ...]


@dataclass(frozen=True)
class CachedConversionTestCase:
    """Statements converted in one run, then read from the disk cache by a later one."""

    description: str
    statement_count: int
    expected_first_conversions: int
    expected_later_conversions: int
    expected_later_cached: int


@dataclass(frozen=True)
class CachePruningTestCase:
    """Entries one run keeps, a later run using some of them `days_later`, then pruning."""

    description: str
    statement_count: int
    later_used_count: int
    days_later: int
    expected_pruned: int
    expected_kept: int


@dataclass(frozen=True)
class WarehouseRelationsTestCase:
    """A statement over declared relations, and those only the warehouse can give it."""

    description: str
    sql: str
    expected_relations: tuple[str, ...]


@dataclass(frozen=True)
class LocalRunRefusalTestCase:
    """A DuckDB failure of converted SQL that keeps the test off DuckDB, and the refusal title."""

    description: str
    error_type: str
    message: str
    fatal: bool
    expected_title: str


@dataclass(frozen=True)
class OwnFailureTestCase:
    """A DuckDB failure of converted SQL that is the test's own result."""

    description: str
    error_type: str
    message: str
    fatal: bool
    expected_refused: bool


@dataclass(frozen=True)
class FailureTextRefusalTestCase:
    """A scenario step's failure text that keeps the scenario off DuckDB, and the refusal title."""

    description: str
    message: str
    expected_title: str


@dataclass(frozen=True)
class OwnFailureTextTestCase:
    """A scenario step's failure text that Snowflake would raise too: the scenario's own result."""

    description: str
    message: str
    expected_refused: bool


@dataclass(frozen=True)
class SqlbuildHelpTestCase:
    """SQLQuack help text, and the help SQLBuild shows for it."""

    description: str
    help_text: str
    expected_help: str


@dataclass(frozen=True)
class MacroPlaceTestCase:
    """A macro's source, the construct refused in its output, and where its source writes it."""

    description: str
    call: str
    token: str
    raw_source: str
    expected_place: MacroPlace | None
