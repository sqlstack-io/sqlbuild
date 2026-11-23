"""Source scanning for setting names in authored diagnostic text."""

from __future__ import annotations

import ast
from collections.abc import Iterator
from pathlib import Path

from scripts.setting_help.constants import (
    DOTTED_SETTING_INSTRUCTION_PATTERN,
    RUST_TEST_PATH_MARKER,
    RUST_TOKEN_PATTERN,
    YAML_FILE_TEXT_MODULES,
    YAML_STYLE_SETTING_PATTERN,
)


def python_findings(repository_root: Path) -> list[str]:
    findings: list[str] = []
    for path in sorted((repository_root / "src" / "sqlbuild").rglob("*.py")):
        relative: str = path.relative_to(repository_root).as_posix()
        if relative in YAML_FILE_TEXT_MODULES:
            continue
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if isinstance(node, ast.Constant | ast.JoinedStr):
                text: str = _literal_text(node)
                if names_setting_without_snippet(text):
                    findings.append(f"{relative}:{node.lineno}: {text}")
    return findings


def rust_findings(repository_root: Path) -> list[str]:
    findings: list[str] = []
    for path in sorted((repository_root / "crates").rglob("src/**/*.rs")):
        relative: str = path.relative_to(repository_root).as_posix()
        if RUST_TEST_PATH_MARKER in relative or relative in YAML_FILE_TEXT_MODULES:
            continue
        source: str = path.read_text(encoding="utf-8")
        lines: list[str] = source.splitlines()
        for number, literal in rust_string_literals(source):
            if names_setting_without_snippet(literal):
                findings.append(f"{relative}:{number}: {lines[number - 1].strip()}")
    return findings


def rust_string_literals(source: str) -> Iterator[tuple[int, str]]:
    """Each string literal's line and text, skipping comments, char literals and lifetimes."""

    for token in RUST_TOKEN_PATTERN.finditer(source):
        text: str | None = (
            token.group("raw") if token.group("raw") is not None else token.group("text")
        )
        if text is not None:
            yield source.count("\n", 0, token.start()) + 1, text


def names_setting_without_snippet(text: str) -> bool:
    return bool(
        YAML_STYLE_SETTING_PATTERN.search(text) or DOTTED_SETTING_INSTRUCTION_PATTERN.search(text)
    )


def _literal_text(node: ast.Constant | ast.JoinedStr) -> str:
    parts: list[ast.expr] = list(node.values) if isinstance(node, ast.JoinedStr) else [node]
    return "".join(
        part.value
        for part in parts
        if isinstance(part, ast.Constant) and isinstance(part.value, str)
    )
