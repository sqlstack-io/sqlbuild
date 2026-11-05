from dataclasses import dataclass, field


@dataclass(frozen=True)
class NativeLayeringTestCase:
    description: str
    crate_dependencies: dict[str, tuple[str, ...]]
    external_dependencies: dict[str, tuple[str, ...]]
    order: tuple[str, ...]
    expected_errors: tuple[str, ...]
    workspace_aliases: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class RepositoryLayeringTestCase:
    description: str
    expected_errors: tuple[str, ...]


@dataclass(frozen=True)
class NativeVersionTestCase:
    description: str
    workspace_version: str
    crate_version: str
    pyproject_version: str
    release_version: str
    cli_version: str
    expected_errors: tuple[str, ...]
    lock_versions: dict[str, str] = field(default_factory=dict)
    python_lock_version: str | None = None
    version_readers: tuple[str, ...] = ()
