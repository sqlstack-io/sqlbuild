"""Delegate snapshot compilation to the native retained snapshot owner."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
    DiscoveredProjectInputs,
    NativeOrchestrationSnapshot,
    build_orchestration_snapshot,
)
from sqlbuild.compiler.compile.types import CompiledProjectView


def compile_snapshot(
    *,
    project: CompiledProjectView,
    metadata_json: str,
    project_dir: Path,
    discovered_inputs: DiscoveredProjectInputs,
) -> NativeOrchestrationSnapshot:
    """Compile the orchestration declarations and retained facts into a data-only snapshot."""
    return build_orchestration_snapshot(
        project,
        metadata_json,
        project_dir,
        discovered_inputs=discovered_inputs,
    )
