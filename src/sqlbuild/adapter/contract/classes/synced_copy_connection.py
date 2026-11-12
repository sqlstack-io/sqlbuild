"""A connection for plan previews that never reaches a warehouse."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.exceptions import WarehouseAccessRefusedError


class SyncedCopyConnection:
    """Refuse every use, so adapter code that reaches for the warehouse fails before it connects."""

    def __getattr__(self, name: str) -> Any:
        raise WarehouseAccessRefusedError(f"connection.{name}")
