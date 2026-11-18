from dataclasses import dataclass


@dataclass(frozen=True)
class UnreplaceableConnectionTestCase:
    """A timed-out item whose connection can neither be recovered nor replaced."""

    description: str
    later_items: int
    expected_completions: tuple[tuple[str, str], ...]
    expected_reconnect_attempts: int
    expected_maximum_seconds: float


@dataclass(frozen=True)
class SessionWarehousesTestCase:
    """Nodes taking connections in turn, each on its job's warehouse or the build's (None)."""

    description: str
    configured: str | None
    session_warehouse: str | None
    steps: tuple[tuple[str, str | None], ...]
    restore: bool
    expected_statements: tuple[tuple[str, str], ...]
    expected_session_reads: int
    expected_error: str | None = None
