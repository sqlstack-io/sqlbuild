"""Place a refusal in a unit test's comparison SQL back in the model or test file that holds it."""

from __future__ import annotations

import difflib
import re
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from sqlbuild._native import SqlDocument
from sqlbuild.executor.local.constants import (
    SQLBUILD_ALLOW_HELP,
    SQLBUILD_SOURCES_HELP,
    SQLQUACK_ALLOW_HELP,
    SQLQUACK_SOURCES_HELP,
    STATEMENT_CONSTRUCT,
)
from sqlbuild.executor.local.models import LocalRefusal, MacroPlace, Segment, TestSources

_MODEL_ORIGIN: re.Pattern[str] = re.compile(r"^model '(?P<name>.+)'$")
_SIMILAR_LINE_RATIO: float = 0.6
_EQUAL_OPCODE: str = "equal"
_MACRO_CALL: re.Pattern[str] = re.compile(r"@(?:py|sql)\.[A-Za-z_][A-Za-z0-9_.]*")
_PYTHON_MACRO: str = "@py."


@dataclass(frozen=True)
class _Place:
    path: str
    line: int
    column: int
    snippet: str
    expanded: bool = False
    snippet_column: int | None = None
    expanded_from: str | None = None
    macro_name: str | None = None
    macro_place: MacroPlace | None = None


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
        help_text = sqlbuild_help(help_text)
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
        snippet_column=None if place is None else place.snippet_column,
        expanded_from=None if place is None else place.expanded_from,
        macro_name=None if place is None else place.macro_name,
        macro_place=None if place is None else place.macro_place,
    )


def sqlbuild_help(help_text: str) -> str:
    """SQLQuack's help with its settings and paths replaced by SQLBuild's."""

    replaced: str = help_text.replace(SQLQUACK_SOURCES_HELP, SQLBUILD_SOURCES_HELP)
    return re.sub(
        SQLQUACK_ALLOW_HELP,
        lambda match: SQLBUILD_ALLOW_HELP.format(key=match["key"]),
        replaced,
    )


def file_placed_refusal(
    *, refusal: Mapping[str, Any], sql: str, file: tuple[str, str] | None
) -> dict[str, Any]:
    """A refusal of converted `sql` placed in `file` (path, text) where its text appears there."""

    placed: dict[str, Any] = dict(refusal)
    help_text: str | None = refusal.get("help")
    if help_text is not None:
        placed["help"] = sqlbuild_help(help_text)
    span: Mapping[str, int] | None = refusal.get("span")
    if span is None or file is None:
        return placed
    start: int = int(span["start"])
    end: int = max(int(span["end"]), start + 1)
    refused: str = sql[start:end].split("\n", 1)[0]
    line, column = _line_column(text=sql, offset=start)
    path, text = file
    place: _Place | None = _token_place(
        path=path, text=text, token=refused, near=line
    ) or _similar_line_place(
        path=path, text=text, line_text=_line_text(text=sql, line=line), column=column
    )
    if place is None:
        return placed
    placed.update(
        path=place.path,
        line=place.line,
        column=place.column,
        width=max(1, len(refused)),
        snippet=place.snippet,
    )
    return placed


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
            ) or _similar_line_place(
                path=sources.test_path,
                text=sources.test_text,
                line_text=_line_text(text=body, line=_line_column(text=body, offset=offset)[0]),
                column=_line_column(text=body, offset=offset)[1],
            )
            if placed is not None:
                return placed
    line, column = _line_column(text=sources.sql, offset=start)
    return _Place(
        path=sources.compiled_path,
        line=line,
        column=column,
        snippet=_line_text(text=sources.sql, line=line),
    )


def _model_place(
    *, name: str, body: str, offset: int, refused: str, sources: TestSources
) -> _Place | None:
    """The model file position of `offset` in a body lifted from the model's resolved SQL."""

    model: Any = sources.models.get(name)
    if model is None:
        return None
    body_line, column = _line_column(text=body, offset=offset)
    line_text: str = _line_text(text=body, line=body_line)
    resolved: str = sources.chain_sql.get(name) or ""
    at: int = resolved.find(body)
    line: int = _line_column(text=resolved, offset=at + offset)[0] if at >= 0 else body_line
    authored: str = model.authored_sql
    path: str = Path(model.relative_path).as_posix()
    token: str = refused.split("\n", 1)[0]
    found: _Place | None = _token_place(path=path, text=authored, token=token, near=line)
    if found is not None:
        return found
    expanded: _Place | None = _expanded_place(
        model=model, path=path, line_text=line_text, token=token, sources=sources
    )
    if expanded is not None and expanded.expanded_from is not None:
        return expanded
    return (
        _similar_line_place(path=path, text=authored, line_text=line_text, column=column)
        or expanded
    )


def _expanded_place(
    *, model: Any, path: str, line_text: str, token: str, sources: TestSources
) -> _Place | None:
    """The macro call that generated `line_text`, showing the generated SQL as the snippet."""

    query: str = model.query_sql
    query_lines: list[str] = query.split("\n")
    wanted: str = line_text.strip()
    query_line: int | None = next(
        (index for index, text in enumerate(query_lines, start=1) if text.strip() == wanted),
        None,
    )
    if query_line is None:
        return None
    generated: str = query_lines[query_line - 1]
    found: int = generated.find(token) if token else -1
    snippet_column: int = found + 1 if found >= 0 else len(generated) - len(generated.lstrip()) + 1
    document: SqlDocument = SqlDocument(
        source=query, dialect=sources.dialect, place=model.query_place, file=model.authored_sql
    )
    mapped: tuple[int, int] | None = document.file_position(
        node=SimpleNamespace(line=query_line, column=snippet_column)
    )
    if mapped is None:
        return None
    call_line, call_column = int(mapped[0]), int(mapped[1])
    call: re.Match[str] | None = _MACRO_CALL.match(
        _line_text(text=model.authored_sql, line=call_line)[call_column - 1 :]
    )
    macro_name: str | None = None if call is None else call.group(0)
    return _Place(
        path=path,
        line=call_line,
        column=call_column,
        snippet=generated,
        expanded=True,
        snippet_column=snippet_column,
        expanded_from=None
        if call is None
        else f"{call.group(0)} at {path}:{call_line}:{call_column}",
        macro_name=macro_name,
        macro_place=None
        if macro_name is None
        else _macro_place(call=macro_name, token=token, macros=sources.macros),
    )


_COMMENT_PREFIXES: tuple[str, ...] = ("#", "--")
"""Lines that only comment on the macro, where a refused construct is never written."""


def _macro_place(*, call: str, token: str, macros: Mapping[str, Any]) -> MacroPlace | None:
    """Where macro `call` writes `token` in its own source: its body, for a Python macro."""

    name: str = call.split(".", 1)[1]
    macro: Any = macros.get(name) or macros.get(name.rsplit(".", 1)[-1])
    word: str = token.strip().split("(", 1)[0].strip()
    if macro is None or not word:
        return None
    lines: list[str] = str(macro.raw_source).split("\n")
    start, end = 0, len(lines)
    if call.startswith(_PYTHON_MACRO):
        short: str = name.rsplit(".", 1)[-1]
        start = next(
            (index for index, text in enumerate(lines) if text.startswith(f"def {short}(")), -1
        )
        if start < 0:
            return None
        end = next(
            (
                index
                for index in range(start + 1, len(lines))
                if lines[index] and not lines[index][0].isspace()
            ),
            len(lines),
        )
    call_suffix: str = r"\s*\(" if "(" in token else ""
    pattern: re.Pattern[str] = re.compile(rf"\b{re.escape(word)}\b{call_suffix}", re.IGNORECASE)
    for index in range(start, end):
        if lines[index].lstrip().startswith(_COMMENT_PREFIXES):
            continue
        found: re.Match[str] | None = pattern.search(lines[index])
        if found is not None:
            return MacroPlace(
                path=Path(macro.relative_path).as_posix(),
                line=index + 1,
                column=found.start() + 1,
                snippet=lines[index],
            )
    return None


def _token_place(*, path: str, text: str, token: str, near: int) -> _Place | None:
    """The occurrence of `token` in `text` nearest line `near`; `None` when it does not appear."""

    if not token.strip():
        return None
    starts: list[int] = [match.start() for match in re.finditer(re.escape(token), text)]
    if not starts:
        return None
    placed: list[tuple[int, int]] = [_line_column(text=text, offset=start) for start in starts]
    line, column = min(placed, key=lambda position: abs(position[0] - near))
    return _Place(path=path, line=line, column=column, snippet=_line_text(text=text, line=line))


def _similar_line_place(*, path: str, text: str, line_text: str, column: int) -> _Place | None:
    """The line of `text` most like `line_text`, with `column` carried across the differences."""

    wanted: str = line_text.strip()
    if not wanted:
        return None
    best: tuple[float, int] = (0.0, 0)
    for index, candidate in enumerate(text.split("\n"), start=1):
        ratio: float = difflib.SequenceMatcher(None, wanted, candidate.strip()).ratio()
        if ratio > best[0]:
            best = (ratio, index)
    if best[0] < _SIMILAR_LINE_RATIO:
        return None
    target: str = _line_text(text=text, line=best[1])
    matcher: difflib.SequenceMatcher[str] = difflib.SequenceMatcher(None, line_text, target)
    mapped: int = column - 1
    for tag, start, end, target_start, target_end in matcher.get_opcodes():
        if start <= column - 1 < end or (start == end == column - 1):
            mapped = target_start + (column - 1 - start) if tag == _EQUAL_OPCODE else target_start
            mapped = min(mapped, max(target_end - 1, target_start))
            break
    return _Place(path=path, line=best[1], column=mapped + 1, snippet=target)


def _text_place(*, path: str, text: str, body: str, offset: int) -> _Place | None:
    """The position of `offset` in `body` where `body` appears verbatim in `text`."""

    at: int = text.find(body.strip())
    if at < 0:
        return None
    at += offset - (len(body) - len(body.lstrip()))
    line, column = _line_column(text=text, offset=max(at, 0))
    return _Place(path=path, line=line, column=column, snippet=_line_text(text=text, line=line))


def _line_column(*, text: str, offset: int) -> tuple[int, int]:
    before: str = text[:offset]
    line: int = before.count("\n") + 1
    column: int = offset - (before.rfind("\n") + 1) + 1
    return line, column


def _line_text(*, text: str, line: int) -> str:
    lines: list[str] = text.split("\n")
    return lines[line - 1] if 0 < line <= len(lines) else ""
