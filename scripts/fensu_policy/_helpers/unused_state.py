"""Find control-store state nothing reads and Rust enum variants nothing names."""

from __future__ import annotations

import itertools
import re
from collections.abc import Iterable
from pathlib import Path

from scripts.fensu_policy.constants import (
    BRACE_CLOSE,
    BRACE_OPEN,
    BRACKET_CLOSE,
    BRACKET_OPEN,
    LIST_SEPARATOR,
    PARENTHESIS_CLOSE,
    PARENTHESIS_OPEN,
    REMOVED_CODE,
    RUST_ATTRIBUTE_PATTERN,
    RUST_BARE_WORD_PATTERN,
    RUST_ENUM_PATTERN,
    RUST_FROM_ATTRIBUTE_PATTERN,
    RUST_IMPL_HEADER_PATTERN,
    RUST_LINE_CONTINUATION,
    RUST_LITERAL_PATTERN,
    RUST_MACRO_INVOCATION_PATTERN,
    RUST_PATH_PAIR_PATTERN,
    RUST_SELF_PATH_PATTERN,
    RUST_SIMPLE_ESCAPES,
    RUST_TEST_FILE_NAME,
    RUST_TEST_MODULE_PATTERN,
    RUST_TEST_PATH_SEGMENTS,
    RUST_VARIANT_IMPORT_PATTERN,
    SQL_ADD_COLUMN_PATTERN,
    SQL_ASSIGNMENT,
    SQL_COLUMN_CONSTRAINT_WORDS,
    SQL_COMMENT_PATTERN,
    SQL_CONFLICT_TARGET_WORDS,
    SQL_CREATE_TABLE_PATTERN,
    SQL_DDL_PREFIX_PATTERN,
    SQL_DELETE_WORD,
    SQL_EXCLUDED_QUALIFIER,
    SQL_FILE_PATTERN,
    SQL_FRAGMENT_MARKERS,
    SQL_FROM_WORD,
    SQL_NEUTRAL_WORDS,
    SQL_QUALIFIER,
    SQL_READ_WORDS,
    SQL_SET_WORD,
    SQL_STAR,
    SQL_STAR_PREDECESSORS,
    SQL_STATEMENT_SEPARATOR,
    SQL_TOKEN_PATTERN,
    SQL_VIEW_PREFIX_PATTERN,
    SQL_WORD_PATTERN,
    SQL_WRITE_WORDS,
)
from scripts.fensu_policy.models import EnumVariant, StateDefinition, StateReads

_LEXEMES: re.Pattern[str] = re.compile(RUST_LITERAL_PATTERN, re.DOTALL)
_CONTINUATION: re.Pattern[str] = re.compile(RUST_LINE_CONTINUATION)
_TEST_MODULE: re.Pattern[str] = re.compile(RUST_TEST_MODULE_PATTERN)
_ENUM: re.Pattern[str] = re.compile(RUST_ENUM_PATTERN)
_ATTRIBUTE: re.Pattern[str] = re.compile(RUST_ATTRIBUTE_PATTERN)
_FROM_ATTRIBUTE: re.Pattern[str] = re.compile(RUST_FROM_ATTRIBUTE_PATTERN)
_MACRO_INVOCATION: re.Pattern[str] = re.compile(RUST_MACRO_INVOCATION_PATTERN)
_PATH_PAIR: re.Pattern[str] = re.compile(RUST_PATH_PAIR_PATTERN)
_IMPL_HEADER: re.Pattern[str] = re.compile(RUST_IMPL_HEADER_PATTERN)
_SELF_PATH: re.Pattern[str] = re.compile(RUST_SELF_PATH_PATTERN)
_VARIANT_IMPORT: re.Pattern[str] = re.compile(RUST_VARIANT_IMPORT_PATTERN)
_BARE_WORD: re.Pattern[str] = re.compile(RUST_BARE_WORD_PATTERN)
_SQL_COMMENTS: re.Pattern[str] = re.compile(SQL_COMMENT_PATTERN, re.DOTALL)
_SQL_TOKENS: re.Pattern[str] = re.compile(SQL_TOKEN_PATTERN)
_SQL_WORD: re.Pattern[str] = re.compile(SQL_WORD_PATTERN)
_DDL_PREFIX: re.Pattern[str] = re.compile(SQL_DDL_PREFIX_PATTERN, re.IGNORECASE)
_VIEW_PREFIX: re.Pattern[str] = re.compile(SQL_VIEW_PREFIX_PATTERN, re.IGNORECASE)
_CREATE_TABLE: re.Pattern[str] = re.compile(SQL_CREATE_TABLE_PATTERN, re.IGNORECASE)
_ADD_COLUMN: re.Pattern[str] = re.compile(SQL_ADD_COLUMN_PATTERN, re.IGNORECASE)


def is_rust_test_path(*, path: str) -> bool:
    """Whether a repository-relative Rust path belongs to tests, benches or examples."""

    parts: list[str] = path.split("/")
    return parts[-1] == RUST_TEST_FILE_NAME or any(
        part in RUST_TEST_PATH_SEGMENTS for part in parts[:-1]
    )


def code_view(*, source: str) -> str:
    """Source with comments and literals blanked and `#[cfg(test)]` modules removed."""

    pieces: list[str] = []
    end: int = 0
    match: re.Match[str]
    for match in _LEXEMES.finditer(source):
        pieces.append(source[end : match.start()])
        pieces.append(_blank(text=match.group(0), fill=" "))
        end = match.end()
    pieces.append(source[end:])
    code: str = "".join(pieces)
    module: re.Match[str] | None = _TEST_MODULE.search(code)
    while module is not None:
        closing: int = _closing(text=code, opening=module.end() - 1, pair=(BRACE_OPEN, BRACE_CLOSE))
        removed: str = _blank(text=code[module.start() : closing + 1], fill=REMOVED_CODE)
        code = code[: module.start()] + removed + code[closing + 1 :]
        module = _TEST_MODULE.search(code, module.start() + len(removed))
    return code


def string_literals(*, source: str) -> list[str]:
    """Values of the string literals in production code, skipping test modules."""

    code: str = code_view(source=source)
    literals: list[str] = []
    match: re.Match[str]
    for match in _LEXEMES.finditer(source):
        if code[match.start()] == REMOVED_CODE:
            continue
        if match.group("raw") is not None:
            literals.append(match.group("raw_body"))
        elif match.group("plain") is not None:
            literals.append(_unescape(text=match.group("plain_body")))
    return literals


def schema_files(*, root: Path, directories: tuple[str, ...]) -> dict[str, str]:
    """Control-store SQL files under the configured directories, keyed by relative path."""

    files: dict[str, str] = {}
    directory: str
    for directory in directories:
        path: Path
        for path in sorted((root / directory).glob(SQL_FILE_PATTERN)):
            files[path.relative_to(root).as_posix()] = path.read_text(encoding="utf-8")
    return files


def schema_definitions(*, path: str, sql: str) -> list[StateDefinition]:
    """Tables and columns that one control-store SQL file creates, in source order."""

    text: str = _blank_sql_comments(sql=sql)
    definitions: list[StateDefinition] = []
    table: re.Match[str]
    for table in _CREATE_TABLE.finditer(text):
        name: str = table.group(1).lower()
        definitions.append(
            StateDefinition(path=path, line=_line(text=text, offset=table.start()), table=name)
        )
        closing: int = _closing(
            text=text, opening=table.end() - 1, pair=(PARENTHESIS_OPEN, PARENTHESIS_CLOSE)
        )
        offset: int
        piece: str
        for offset, piece in _top_level_pieces(text=text, start=table.end(), end=closing):
            column: str | None = _column_name(piece=piece)
            if column is not None:
                definitions.append(
                    StateDefinition(
                        path=path, line=_line(text=text, offset=offset), table=name, column=column
                    )
                )
    added: re.Match[str]
    for added in _ADD_COLUMN.finditer(text):
        definitions.append(
            StateDefinition(
                path=path,
                line=_line(text=text, offset=added.start()),
                table=added.group(1).lower(),
                column=added.group(2).lower(),
            )
        )
    return sorted(definitions, key=lambda definition: definition.line)


def sql_read_fragments(*, sql: str) -> list[str]:
    """Statements of a SQL file that can read state: everything except plain DDL."""

    return [
        statement
        for statement in _blank_sql_comments(sql=sql).split(SQL_STATEMENT_SEPARATOR)
        if statement.strip() and not _plain_ddl(text=statement)
    ]


def collect_reads(*, fragments: Iterable[str], definitions: list[StateDefinition]) -> StateReads:
    """Tables and columns some fragment reads, attributing columns to mentioned tables."""

    columns_by_table: dict[str, set[str]] = {}
    definition: StateDefinition
    for definition in definitions:
        columns: set[str] = columns_by_table.setdefault(definition.table, set())
        if definition.column is not None:
            columns.add(definition.column)
    reads: StateReads = StateReads(tables=set(), columns=set())
    fragment: str
    for fragment in fragments:
        if not _plain_ddl(text=fragment):
            reads = _read_fragment(
                fragment=fragment, columns_by_table=columns_by_table, reads=reads
            )
    return reads


def unread_definitions(
    *, definitions: list[StateDefinition], reads: StateReads
) -> list[StateDefinition]:
    """Unread tables, and unread columns of tables that are themselves read."""

    return [
        definition
        for definition in definitions
        if (definition.column is None and definition.table not in reads.tables)
        or (
            definition.column is not None
            and definition.table in reads.tables
            and (definition.table, definition.column) not in reads.columns
        )
    ]


def enum_variants(*, path: str, code: str) -> list[EnumVariant]:
    """Variants of every enum declared in one file's production code view."""

    variants: list[EnumVariant] = []
    generated: list[tuple[int, int]] = [
        (
            invocation.start(),
            _closing(text=code, opening=invocation.end() - 1, pair=(BRACE_OPEN, BRACE_CLOSE)),
        )
        for invocation in _MACRO_INVOCATION.finditer(code)
    ]
    declaration: re.Match[str]
    for declaration in _ENUM.finditer(code):
        if any(start <= declaration.start() <= end for start, end in generated):
            continue
        opening: int = declaration.end() - 1
        closing: int = _closing(text=code, opening=opening, pair=(BRACE_OPEN, BRACE_CLOSE))
        offset: int
        piece: str
        for offset, piece in _variant_pieces(code=code, start=opening + 1, end=closing):
            name: re.Match[str] | None = _SQL_WORD.match(piece)
            if (
                name is not None
                and _FROM_ATTRIBUTE.search(code, offset, offset + len(piece)) is None
            ):
                variants.append(
                    EnumVariant(
                        path=path,
                        line=_line(text=code, offset=offset),
                        enum=declaration.group("name"),
                        variant=name.group(0),
                        body=(opening, closing),
                    )
                )
    return variants


def named_variants(*, variants: list[EnumVariant], codes: dict[str, str]) -> set[EnumVariant]:
    """Variants named by a path, by `Self` in an impl's file, or by an imported variant."""

    bodies: dict[str, list[tuple[int, int]]] = {}
    variant: EnumVariant
    for variant in variants:
        bodies.setdefault(variant.path, []).append(variant.body)
    names: set[tuple[str, str]] = set()
    path: str
    code: str
    for path, code in codes.items():
        names.update(_file_names(code=_without_spans(text=code, spans=bodies.get(path, []))))
    return {variant for variant in variants if (variant.enum, variant.variant) in names}


def _file_names(*, code: str) -> set[tuple[str, str]]:
    names: set[tuple[str, str]] = {
        (match.group(1), match.group(2)) for match in _PATH_PAIR.finditer(code)
    }
    implemented: set[str] = set()
    header: re.Match[str]
    for header in _IMPL_HEADER.finditer(code):
        implemented.update(_SQL_WORD.findall(header.group(0)))
    self_names: set[str] = {match.group(1) for match in _SELF_PATH.finditer(code)}
    imported: set[str] = {match.group(1) for match in _VARIANT_IMPORT.finditer(code)}
    bare: set[str] = set(_BARE_WORD.findall(code)) if imported else set()
    enum: str
    for enum in implemented:
        names.update(itertools.product((enum,), self_names))
    for enum in imported:
        names.update(itertools.product((enum,), bare))
    return names


def _without_spans(*, text: str, spans: list[tuple[int, int]]) -> str:
    start: int
    end: int
    for start, end in spans:
        text = text[:start] + _blank(text=text[start:end], fill=" ") + text[end:]
    return text


def _read_fragment(
    *, fragment: str, columns_by_table: dict[str, set[str]], reads: StateReads
) -> StateReads:
    tokens: list[str] = [token.strip('"').lower() for token in _SQL_TOKENS.findall(fragment)]
    known_columns: set[str] = set().union(*columns_by_table.values())
    mentioned: list[str] = [token for token in tokens if token in columns_by_table]
    keywords: bool = any(token in SQL_READ_WORDS | SQL_WRITE_WORDS for token in tokens)
    words: set[str] = {
        token for token in tokens if _SQL_WORD.fullmatch(token) and token not in SQL_NEUTRAL_WORDS
    }
    shaped: bool = any(token in SQL_FRAGMENT_MARKERS for token in tokens)
    if not keywords and not mentioned and not (shaped and words and words <= known_columns):
        return reads
    read_words: set[str] = set()
    star: bool = False
    reading: bool = not keywords
    deleting: bool = False
    set_depth: int | None = None
    depth: int = 0
    position: int
    token: str
    for position, token in enumerate(tokens):
        before: str = tokens[position - 1] if position else ""
        qualifier: str = tokens[position - 2] if position > 1 and before == SQL_QUALIFIER else ""
        depth += (token == PARENTHESIS_OPEN) - (token == PARENTHESIS_CLOSE)
        deleting = deleting or token == SQL_DELETE_WORD
        if token in SQL_WRITE_WORDS or (before, token) in SQL_CONFLICT_TARGET_WORDS:
            reading = False
            set_depth = depth if token == SQL_SET_WORD else None
        elif token == SQL_FROM_WORD and deleting:
            deleting = False
        elif token in SQL_READ_WORDS:
            reading = True
            set_depth = None
        elif set_depth == depth:
            reading = token == SQL_ASSIGNMENT or (reading and token != LIST_SEPARATOR)
        if not reading or token in SQL_READ_WORDS or qualifier == SQL_EXCLUDED_QUALIFIER:
            continue
        star = star or (token == SQL_STAR and before in SQL_STAR_PREDECESSORS)
        read_words.add(token)
        if token in columns_by_table:
            reads.tables.add(token)
    owner: str
    for owner in dict.fromkeys(mentioned or list(columns_by_table)):
        owned: set[str] = columns_by_table[owner]
        credited: set[str] = owned if star and mentioned else owned & read_words
        reads.columns.update((owner, column) for column in credited)
    return reads


def _plain_ddl(*, text: str) -> bool:
    return _DDL_PREFIX.match(text) is not None and _VIEW_PREFIX.match(text) is None


def _blank(*, text: str, fill: str) -> str:
    return re.sub(r"[^\n]", fill, text)


def _blank_sql_comments(*, sql: str) -> str:
    return _SQL_COMMENTS.sub(lambda comment: _blank(text=comment.group(0), fill=" "), sql)


def _unescape(*, text: str) -> str:
    return re.sub(
        r"\\(.)",
        lambda escaped: RUST_SIMPLE_ESCAPES.get(escaped.group(1), ""),
        _CONTINUATION.sub("", text),
    )


def _line(*, text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _closing(*, text: str, opening: int, pair: tuple[str, str]) -> int:
    depth: int = 0
    offset: int
    for offset in range(opening, len(text)):
        depth += (text[offset] == pair[0]) - (text[offset] == pair[1])
        if depth == 0:
            return offset
    return len(text)


def _top_level_pieces(*, text: str, start: int, end: int) -> list[tuple[int, str]]:
    pieces: list[tuple[int, str]] = []
    depth: int = 0
    piece_start: int = start
    offset: int
    for offset in range(start, end):
        character: str = text[offset]
        depth += (character in (PARENTHESIS_OPEN, BRACKET_OPEN, BRACE_OPEN)) - (
            character in (PARENTHESIS_CLOSE, BRACKET_CLOSE, BRACE_CLOSE)
        )
        if character == LIST_SEPARATOR and depth == 0:
            pieces.append((piece_start, text[piece_start:offset]))
            piece_start = offset + 1
    pieces.append((piece_start, text[piece_start:end]))
    stripped: list[tuple[int, str]] = []
    piece: str
    for piece_start, piece in pieces:
        if piece.strip():
            stripped.append((piece_start + len(piece) - len(piece.lstrip()), piece.strip()))
    return stripped


def _variant_pieces(*, code: str, start: int, end: int) -> list[tuple[int, str]]:
    body: str = _without_attributes(text=code[start:end])
    return [
        (start + offset, piece)
        for offset, piece in _top_level_pieces(text=body, start=0, end=len(body))
    ]


def _without_attributes(*, text: str) -> str:
    match: re.Match[str] | None = _ATTRIBUTE.search(text)
    while match is not None:
        closing: int = _closing(
            text=text, opening=match.end() - 1, pair=(BRACKET_OPEN, BRACKET_CLOSE)
        )
        text = (
            text[: match.start()]
            + _blank(text=text[match.start() : closing + 1], fill=" ")
            + text[closing + 1 :]
        )
        match = _ATTRIBUTE.search(text, match.start() + 1)
    return text


def _column_name(*, piece: str) -> str | None:
    first: str = piece.split(None, 1)[0].strip('"').lower()
    if first in SQL_COLUMN_CONSTRAINT_WORDS or _SQL_WORD.fullmatch(first) is None:
        return None
    return first
