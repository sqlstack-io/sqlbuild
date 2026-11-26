from dataclasses import dataclass


@dataclass(frozen=True)
class BuildSchemaPreflightTestCase:
    description: str
    expected_schemas: tuple[tuple[str | None, str], ...]


@dataclass(frozen=True)
class RunnableGraphWidthTestCase:
    description: str
    expected_width: int


@dataclass(frozen=True)
class PipelineIdentityTestCase:
    description: str
    expected_invocation_id: str
    expected_run_id: str
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class RunTerminalTestCase:
    description: str
    expected_event_type: str
    expected_error_type: str | None
    expected_failed_count: int | None


@dataclass(frozen=True)
class EmptySqlPhaseTestCase:
    description: str
    execution_announced: bool
    expected_event_types: tuple[str, ...]
    expected_phase: dict[str, object] | None
    expected_connects: int
