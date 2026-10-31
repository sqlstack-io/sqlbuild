"""Delegate authoring-record compilation to the native retained snapshot owner."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

from sqlbuild._native import NativeOrchestrationSnapshot, build_orchestration_snapshot
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.orchestration_declarations.models import Job, JobGroup


def compile_snapshot(
    *,
    project: CompiledProjectView,
    metadata_json: str,
    project_dir: Path,
    declarations: Sequence[Job | JobGroup] | None = None,
) -> NativeOrchestrationSnapshot:
    """Compile authoring records and retained project facts into a data-only snapshot."""
    return build_orchestration_snapshot(
        project, metadata_json, project_dir, declarations=declarations
    )
