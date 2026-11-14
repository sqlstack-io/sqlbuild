"""Frozen cases for the orchestration migration shim's import boundary."""

from dataclasses import dataclass


@dataclass(frozen=True)
class ShimImportCase:
    description: str
    module: str
    allowed_python_importers: frozenset[str]
    allowed_rust_references: frozenset[str]
