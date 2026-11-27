from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SessionLoadTestCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_rows: tuple[tuple[int, str], ...]


@dataclass(frozen=True)
class UnsupportedAdapterTestCase:
    description: str
    adapter_name: str
    expected_error_fragment: str
