from dataclasses import dataclass, field


@dataclass(frozen=True)
class LoadSelectionTestCase:
    description: str
    select: tuple[str, ...]
    expected_entry_names: tuple[str, ...]
    expected_loader_node_flags: tuple[bool, ...]


@dataclass(frozen=True)
class LoadReferenceSelectionTestCase:
    description: str
    select: tuple[str, ...]
    expected_entry_names: tuple[str, ...]


@dataclass(frozen=True)
class LoaderDestinationSelectionTestCase:
    description: str
    destination: str
    expected_parts: tuple[str | None, str | None, str | None]


@dataclass(frozen=True)
class LoaderDefaultResolutionTestCase:
    description: str
    adapter: str
    connection: dict[str, object]
    expected_defaults: tuple[str | None, str | None]
    project_defaults: tuple[str | None, str | None] = (None, None)
    connections: dict[str, dict[str, object]] = field(default_factory=dict)
    project_vars: dict[str, str] = field(default_factory=dict)
    cli_vars: dict[str, object] | None = None
