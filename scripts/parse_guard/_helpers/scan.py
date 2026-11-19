"""Find Polyglot and SQLQuack shared or Snowflake parses that use a library default depth."""

from __future__ import annotations

import re
from pathlib import Path

from scripts.parse_guard.constants import (
    CRATES_DIRECTORY,
    DEFAULT_GUARD_PATTERNS,
    GUARD_FIELD,
    GUARDED_OPTION_LITERALS,
    SKIPPED_DIRECTORIES,
    SKIPPED_FILES,
    SOURCE_SUFFIX,
    TYPE_POSITION_SUFFIXES,
)

_PATTERNS: tuple[re.Pattern[str], ...] = tuple(
    re.compile(pattern, re.DOTALL) for pattern in DEFAULT_GUARD_PATTERNS
)


def crate_sources(*, root: Path) -> tuple[Path, ...]:
    """Every non-test Rust source under the workspace crates, except the shared guard."""
    sources: list[Path] = []
    path: Path
    for path in (root / CRATES_DIRECTORY).rglob(f"*{SOURCE_SUFFIX}"):
        skipped: bool = bool(SKIPPED_DIRECTORIES.intersection(path.relative_to(root).parts))
        if path.name not in SKIPPED_FILES and not skipped:
            sources.append(path)
    return tuple(sorted(sources))


def unguarded_sites(*, label: str, text: str) -> tuple[str, ...]:
    """Each place in `text` a parse uses a default guard, as `label:line pattern`."""
    sites: list[str] = []
    pattern: re.Pattern[str]
    for pattern in _PATTERNS:
        sites.extend(
            f"{label}:{_line(text=text, offset=found.start())} {pattern.pattern}"
            for found in pattern.finditer(text)
        )
    literal: str
    for literal in GUARDED_OPTION_LITERALS:
        sites.extend(
            f"{label}:{_line(text=text, offset=found.start())} {literal}"
            for found in re.finditer(re.escape(literal), text)
            if _unguarded_literal(text=text, start=found.start(), literal=literal)
        )
    return tuple(sites)


def get_parse_guard_errors(root: Path) -> tuple[str, ...]:
    """Every unguarded parse site of the workspace crates."""
    errors: list[str] = []
    path: Path
    for path in crate_sources(root=root):
        errors.extend(
            unguarded_sites(
                label=path.relative_to(root).as_posix(), text=path.read_text(encoding="utf-8")
            )
        )
    return tuple(errors)


def _line(*, text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _unguarded_literal(*, text: str, start: int, literal: str) -> bool:
    if text[:start].rstrip().endswith(TYPE_POSITION_SUFFIXES):
        return False
    return GUARD_FIELD not in _block(text=text, open_at=start + len(literal) - 1)


def _block(*, text: str, open_at: int) -> str:
    depth: int = 0
    offset: int
    character: str
    for offset, character in enumerate(text[open_at:]):
        depth += {"{": 1, "}": -1}.get(character, 0)
        if depth == 0:
            return text[open_at : open_at + offset + 1]
    return text[open_at:]
