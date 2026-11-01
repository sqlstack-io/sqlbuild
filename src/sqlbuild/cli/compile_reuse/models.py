"""Compile reuse requests and provider settings inputs."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class CompileReuseRequest:
    """The compile command choices that decide whether and how a compile is reused."""

    project_dir: Path | None
    selected_target: str | None
    json_output: bool
    no_color: bool
    manifest: bool
    dag_path: str | None
    defer_to: str | None
    no_cache: bool
    no_sql_validation: bool
    strict: bool
    lineage_mode: str
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    profiling: bool
    debug: bool


@dataclass(frozen=True)
class SettingsEnvironmentInputs:
    """Where one provider settings class can read values outside the project files."""

    case_sensitive: bool
    names: tuple[str, ...]
    prefixes: tuple[str, ...]
    env_files: tuple[str, ...]
    secrets_dirs: tuple[str, ...]


@dataclass(frozen=True)
class SettingsInputsResult:
    """Enumerated provider settings inputs, or why they cannot be enumerated exactly."""

    inputs: tuple[SettingsEnvironmentInputs, ...]
    unsupported_reason: str | None = None
