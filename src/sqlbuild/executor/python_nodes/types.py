"""Type declarations for Python-node execution."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

from sqlbuild._native import DiscoveredAssetFunction, PythonNodeIdentity

type ExecutablePythonNode = DiscoveredAssetFunction

if TYPE_CHECKING:
    from sqlbuild.executor.python_nodes.models import PythonNodeExecutionResult


class PythonIdentityRecorder(Protocol):
    def __call__(
        self, *, identity: PythonNodeIdentity | None, _target_name: str | None
    ) -> None: ...


class OwnedResultCallback(Protocol):
    def __call__(
        self, *, node: ExecutablePythonNode, result: PythonNodeExecutionResult
    ) -> None: ...
