from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class DebugCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_stdout_fragment: str
    expected_returncode: int = 0
    project_toml: str = ""


@dataclass(frozen=True)
class DebugJsonCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_success: bool
    expected_check_tail: list[dict[str, str]]
