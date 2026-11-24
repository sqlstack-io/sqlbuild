"""Place a refusal in a unit test's comparison SQL back in the model or test file that holds it."""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from sqlbuild._native import SqlDocument
from sqlbuild.executor.local.constants import (
    SQLBUILD_SOURCES_HELP,
    SQLQUACK_SOURCES_HELP,
    STATEMENT_CONSTRUCT,
)
from sqlbuild.executor.local.models import LocalRefusal

_MODEL_ORIGIN: re.Pattern[str] = re.compile(r"^model '(?P<name>.+)'$")


@dataclass(frozen=True)
class Segment:
    """Where one CTE body sits in the comparison SQL, and what it stands for."""

    start: int
    end: int
    origin: str


@dataclass(frozen=True)
class TestSources:
    """What a unit test's comparison SQL was built from: its test file, models and compiled file."""

    sql: str
    segments: Sequence[Segment]
    test_path: str | None
    test_text: str | None
    compiled_path: str
    mocked_models: frozenset[str]
    models: Mapping[str, Any]
    chain_sql: Mapping[str, str]
    dialect: str


@dataclass(frozen=True)
class _Place:
    path: str
    line: int
    column: int
    snippet: str
    expanded: bool = False


def placed_refusal(*, refusal: Mapping[str, Any], sources: TestSources) -> LocalRefusal:
    """One SQLQuack refusal as a diagnostic in the model or test file, or the compiled test file."""

    span: Mapping[str, int] | None = refusal.get("span")
    place: _Place | None = None
    width: int = 1
    if span is not None:
        start: int = int(span["start"])
        end: int = max(int(span["end"]), start + 1)
        refused: str = sources.sql[start:end]
        width = max(1, len(refused.split("\n", 1)[0]))
        place = _place(start=start, refused=refused, sources=sources)
    help_text: str | None = refusal.get("help")
    if help_text is not None:
        help_text = help_text.replace(SQLQUACK_SOURCES_HELP, SQLBUILD_SOURCES_HELP)
    return LocalRefusal(
        category=str(refusal.get("category", "")),
        construct=str(refusal.get("construct") or STATEMENT_CONSTRUCT),
        reason=str(refusal.get("reason", "")),
        code=refusal.get("code"),
        message=refusal.get("message"),
        label=refusal.get("label"),
        notes=tuple(refusal.get("notes") or ()),
        help=help_text,
        path=None if place is None else place.path,
        line=None if place is None else place.line,
        column=None if place is None else place.column,
        width=width,
        snippet=None if place is None else place.snippet,
    )


def _place(*, start: int, refused: str, sources: TestSources) -> _Place:
    segment: Segment | None = next(
        (item for item in sources.segments if item.start <= start < item.end), None
    )
    if segment is not None:
        body: str = sources.sql[segment.start : segment.end]
        offset: int = start - segment.start
        match: re.Match[str] | None = _MODEL_ORIGIN.match(segment.origin)
        if match is not None and match["name"] not in sources.mocked_models:
            placed: _Place | None = _model_place(
                name=match["name"], body=body, offset=offset, refused=refused, sources=sources
            )
            if placed is not None:
                return placed
        elif sources.test_text is not None and sources.test_path is not None:
            placed = _text_place(
                path=sources.test_path, text=sources.test_text, body=body, offset=offset
            )
            if placed is not None:
                return placed
    line, column = _line_column(sources.sql, start)
    return _Place(
        path=sources.compiled_path,
        line=line,
        column=column,
        snippet=_line_text(sources.sql, line),
    )


def _model_place(
    *, name: str, body: str, offset: int, refused: str, sources: TestSources
) -> _Place | None:
    """The model file position of `offset` in a body lifted from the model's resolved SQL."""

    model: Any = sources.models.get(name)
    resolved: str | None = sources.chain_sql.get(name)
    if model is None or resolved is None:
        return None
    at: int = resolved.find(body)
    if at < 0:
        return None
    line, column = _line_column(resolved, at + offset)
    query: str = model.query_sql
    query_line: str = _line_text(query, line)
    token: str = refused.split("\n", 1)[0]
    found: int = query_line.find(token) if token else -1
    if found >= 0:
        column = found + 1
    document: SqlDocument = SqlDocument(
        source=query,
        dialect=sources.dialect,
        place=model.query_place,
        file=model.authored_sql,
    )
    mapped: tuple[int, int] | None = document.file_position(
        node=SimpleNamespace(line=line, column=column)
    )
    if mapped is None:
        return None
    file_line, file_column = int(mapped[0]), int(mapped[1])
    authored_line: str = _line_text(model.authored_sql, file_line)
    expanded: bool = bool(token) and token not in authored_line
    return _Place(
        path=Path(model.relative_path).as_posix(),
        line=file_line,
        column=file_column,
        snippet=query_line if expanded else authored_line,
        expanded=expanded,
    )


def _text_place(*, path: str, text: str, body: str, offset: int) -> _Place | None:
    """The position of `offset` in `body` where `body` appears verbatim in `text`."""

    at: int = text.find(body.strip())
    if at < 0:
        return None
    at += offset - (len(body) - len(body.lstrip()))
    line, column = _line_column(text, max(at, 0))
    return _Place(path=path, line=line, column=column, snippet=_line_text(text, line))


def _line_column(text: str, offset: int) -> tuple[int, int]:
    before: str = text[:offset]
    line: int = before.count("\n") + 1
    column: int = offset - (before.rfind("\n") + 1) + 1
    return line, column


def _line_text(text: str, line: int) -> str:
    lines: list[str] = text.split("\n")
    return lines[line - 1] if 0 < line <= len(lines) else ""
