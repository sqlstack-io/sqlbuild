"""Models, declarations and the project registry every layer generator writes into."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import PurePosixPath


@dataclass(frozen=True)
class Column:
    name: str
    type: str
    audits: tuple[str, ...] = ()
    nullable: bool = True

    def render(self) -> str:
        parts = [f"type {self.type}"]
        if not self.nullable:
            parts.append("nullable false")
        if self.audits:
            parts.append(f"audits [{', '.join(self.audits)}]")
        return f"    {self.name} ({', '.join(parts)}),"


@dataclass
class Model:
    name: str
    folder: str
    layer: str
    description: str
    columns: list[Column]
    body: str
    config: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    refs: list[str] = field(default_factory=list)
    sources: list[str] = field(default_factory=list)

    @property
    def path(self) -> str:
        return f"{self.folder}/{self.name}.sql"

    def column(self, name: str) -> Column:
        return next(column for column in self.columns if column.name == name)

    def has(self, name: str) -> bool:
        return any(column.name == name for column in self.columns)

    def render(self) -> str:
        lines = ["MODEL ("]
        lines.extend(_description_lines(self.description))
        if self.tags:
            lines.append(f"  tags [{', '.join(self.tags)}],")
        lines.extend(
            f"  {line}," if not line.endswith(",") else f"  {line}" for line in self.config
        )
        lines.append("  columns (")
        lines.extend(column.render() for column in self.columns)
        lines.append("  ),")
        lines.append(");")
        return "\n".join(lines) + "\n\n" + self.body.strip() + "\n"


def _description_lines(text: str) -> list[str]:
    if len(text) <= 80:
        return [f'  description "{text}",']
    words = text.split()
    wrapped: list[str] = []
    line = ""
    for word in words:
        if line and len(line) + 1 + len(word) > 96:
            wrapped.append(line)
            line = word
        else:
            line = f"{line} {word}" if line else word
    wrapped.append(line)
    return ['  description """', *(f"    {item}" for item in wrapped), '  """,']


@dataclass(frozen=True)
class Declaration:
    """A reusable declaration whose folder is decided by where it is used."""

    name: str
    role: str  # sql/macros/sql, sql/macros/python, constants, enums, sql/audits/generic, schemas
    content: str
    file_name: str = ""

    @property
    def filename(self) -> str:
        if self.file_name:
            return self.file_name
        suffix = ".py" if self.role == "sql/macros/python" else ".sql"
        return f"{self.name}{suffix}"


_SQL_MACRO = re.compile(r"@sql\.([a-z_0-9]+)\(")
_PY_MACRO = re.compile(r"@py\.([a-z_0-9]+)\(")
_CONST = re.compile(r'@const\("([a-z_0-9]+)"\)')
_ENUM = re.compile(r'@enum\("([a-z_0-9]+)"\)')
_AUDIT = re.compile(r"\baudits \[([^\]]*)\]")
_AUDIT_NAME = re.compile(r"(?:^|,)\s*([a-z_0-9]+)")


def declaration_uses(text: str) -> set[tuple[str, str]]:
    """(kind, name) of every macro, constant, enum and generic audit a file uses by name."""
    uses: set[tuple[str, str]] = set()
    uses.update(("macro", name) for name in _SQL_MACRO.findall(text))
    uses.update(("macro", name) for name in _PY_MACRO.findall(text))
    uses.update(("constant", name) for name in _CONST.findall(text))
    uses.update(("enum", name) for name in _ENUM.findall(text))
    for group in _AUDIT.findall(text):
        for name in _AUDIT_NAME.findall(re.sub(r"\([^)]*\)", "", group)):
            uses.add(("audit", name))
    return uses


_KIND_OF_ROLE = {
    "sql/macros/sql": "macro",
    "sql/macros/python": "macro",
    "constants": "constant",
    "enums": "enum",
    "sql/audits/generic": "audit",
    "schemas": "schema",
}

BUILT_IN_AUDITS = frozenset({"not_null", "unique", "accepted_values", "relationships"})


class Project:
    def __init__(self) -> None:
        self.models: dict[str, Model] = {}
        self.files: dict[str, str] = {}
        self.declarations: dict[tuple[str, str], Declaration] = {}
        # (rule, declaration key or model path, reason), rendered into sqlbuild_project.toml
        self.exceptions: list[tuple[str, tuple[str, str] | str, str]] = []

    def add(self, model: Model) -> Model:
        if model.name in self.models:
            raise ValueError(f"duplicate model {model.name}")
        self.models[model.name] = model
        return model

    def ref(self, name: str) -> Model:
        return self.models[name]

    def write(self, path: str, content: str) -> None:
        if path in self.files:
            raise ValueError(f"duplicate file {path}")
        self.files[path] = content

    def declare(self, declaration: Declaration) -> None:
        key = (_KIND_OF_ROLE[declaration.role], declaration.name)
        if key in self.declarations:
            raise ValueError(f"duplicate declaration {key}")
        self.declarations[key] = declaration

    def render(self) -> dict[str, str]:
        """Every file of the project, with each declaration placed beside its users."""
        files = dict(self.files)
        for model in self.models.values():
            files[model.path] = model.render()
        consumers = self._consumers(files)
        placed: dict[tuple[str, str], str] = {}
        for key, declaration in sorted(self.declarations.items()):
            folders = consumers.get(key)
            if not folders:
                continue
            path = f"{_placement(folders, declaration.role)}/{declaration.filename}"
            placed[key] = path
            files[path] = declaration.content
        blocks: list[str] = []
        for rule, subject, reason in self.exceptions:
            path = placed.get(subject) if isinstance(subject, tuple) else subject
            if path is None:
                continue
            blocks.append(
                f'[[rules.rule_exceptions]]\nrule = "{rule}"\npath = "{path}"\n'
                f'reason = """\n{reason.strip()}\n"""\n'
            )
        if blocks:
            files["sqlbuild_project.toml"] += "\n" + "\n".join(blocks)
        return dict(sorted(files.items()))

    def _consumers(self, files: dict[str, str]) -> dict[tuple[str, str], set[str]]:
        direct: dict[tuple[str, str], set[str]] = {}
        for path, text in files.items():
            if not path.endswith(".sql") or not path.startswith(("sql/models/", "tests/")):
                continue
            folder = str(PurePosixPath(path).parent)
            uses = declaration_uses(text)
            if "audit_factories [fact_table_quality]" in text:
                uses.add(("audit", "has_rows"))
            for use in uses:
                if use[0] == "audit" and use[1] in BUILT_IN_AUDITS:
                    continue
                direct.setdefault(use, set()).add(folder)
        for path, text in files.items():
            if path.startswith(("sql/sources/", "sql/seeds/")):
                for use in declaration_uses(text):
                    direct.setdefault(use, set()).add(path.split("/")[0])
        # A declaration used inside another declaration is used wherever that one is.
        uses_of: dict[tuple[str, str], set[tuple[str, str]]] = {
            key: declaration_uses(declaration.content) & set(self.declarations)
            for key, declaration in self.declarations.items()
        }
        resolved: dict[tuple[str, str], set[str]] = {}

        def resolve(key: tuple[str, str], stack: tuple[tuple[str, str], ...]) -> set[str]:
            if key in resolved:
                return resolved[key]
            folders = set(direct.get(key, set()))
            for other, used in uses_of.items():
                if key in used and other not in stack:
                    folders |= resolve(other, (*stack, key))
            resolved[key] = folders
            return folders

        for key in self.declarations:
            resolve(key, ())
        return resolved


def _placement(folders: set[str], role: str) -> str:
    roots = {folder.split("/")[0] for folder in folders}
    if len(roots) > 1:
        return role
    parts = [PurePosixPath(folder).parts for folder in folders]
    common: list[str] = []
    for level in zip(*parts, strict=False):
        if len(set(level)) != 1:
            break
        common.append(level[0])
    if len(common) < 2:
        return role
    owner = "/".join(common)
    private = all(folder == owner for folder in folders)
    return f"{owner}/_sqlbuild/{'_' if private else ''}{role}"


def cte(name: str, body: str) -> str:
    return f"{name} AS (\n{_indent(body.strip())}\n)"


def _indent(text: str, spaces: int = 2) -> str:
    pad = " " * spaces
    return "\n".join(f"{pad}{line}" if line else line for line in text.splitlines())


def select_list(items: list[str]) -> str:
    return ",\n".join(f"  {item}" for item in items)


def query(ctes: list[str], final: str = "final") -> str:
    return "WITH " + ",\n\n".join(ctes) + f"\n\nSELECT *\nFROM {final}\n"


def import_cte(name: str, relation: str) -> str:
    return cte(name, f"SELECT *\nFROM {relation}")
