"""Randomized trees, projects, and model files for shared-snapshot discovery tests."""

from __future__ import annotations

import random
import re
from collections.abc import Callable
from functools import partial
from itertools import compress
from pathlib import Path
from typing import cast

import sqlbuild._native as _native
from sqlbuild._native import (
    DiscoveredProjectInputs,
    discover_audit_files,
    discover_constant_files,
    discover_enum_files,
    discover_macro_files,
    discover_model_files,
    discover_model_schema_files,
    discover_schema_files,
    discover_seed_files,
    discover_sql_function_files,
    discover_sql_hook_files,
    discover_test_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import (
    load_local_config,
    load_project_config,
)

# Test-only reference grammar of the MODEL header the native matcher replaced.
_REFERENCE_MODEL_HEADER_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*MODEL\s*\("
    r"(?P<header>(?>(?:[^\"')\\]|\\.)++"
    r"|\"(?:[^\"\\]|\\.)*+\""
    r"|'(?:[^'\\]|\\.)*+'"
    r"|[\"'](?:[^\\)]|\\.)*+"
    r"|\)(?!\s*;))*+)"
    r"\)\s*;\s*(?P<sql>.*)\Z",
    re.DOTALL,
)
_DIRECTORY_NAMES: tuple[str, ...] = (
    "orders",
    "customers",
    "_sqlbuild",
    "enums",
    "_enums",
    "macros",
    ".hidden",
)
_FILE_NAMES: tuple[str, ...] = (
    "schema.yml",
    "daily.sql",
    "daily.SQL",
    "helper.py",
    "notes.yml",
    ".draft.sql",
)
_LINK_NAMES: tuple[str, ...] = ("shared", "enums_link", "_sqlbuild_link")
_ISOLATED_DISCOVERERS: dict[str, Callable[..., tuple[object, ...]]] = {
    "model_files": partial(
        discover_model_files,
        extract_implicit_alias_columns=True,
        extract_output_column_locations=True,
    ),
    "enum_files": discover_enum_files,
    "constant_files": discover_constant_files,
    "model_schema_files": discover_model_schema_files,
    "sql_function_files": discover_sql_function_files,
    "sql_hook_files": discover_sql_hook_files,
    "schema_files": discover_schema_files,
    "seed_files": discover_seed_files,
    "test_files": discover_test_files,
    "audit_files": discover_audit_files,
    "macro_files": discover_macro_files,
}
_SCOPED_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "shop"\nadapter = "duckdb"\n',
    "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
    "sql/models/sales/daily.sql": "MODEL (description 'Daily.');\nSELECT 1 AS day_id\n",
    "sql/models/sales/schema.yml": "models: []\n",
    "sql/models/sales/enums/region.sql": "ENUM (name region, members [NORTH, SOUTH]);\n",
    "sql/models/sales/_sqlbuild/constants/limit.sql": "CONSTANT (name order_limit, value 10);\n",
    "sql/models/sales/_sqlbuild/audits/generic/positive.sql": (
        "AUDIT (name positive);\nSELECT 1 WHERE FALSE\n"
    ),
    "sql/macros/python/money.py": "def money(value):\n    return value\n",
    "sql/enums/status.sql": "ENUM (name status, members [OPEN, CLOSED]);\n",
    "sql/functions/sql/tax.sql": (
        "FUNCTION (arguments (amount DOUBLE), returns DOUBLE);\namount * 0.2\n"
    ),
}
_HEADER_FRAGMENTS: tuple[str, ...] = (
    "MODEL",
    "model",
    "(",
    ")",
    ";",
    "'",
    '"',
    "\\",
    "name orders",
    "columns (id INT)",
    "SELECT 1",
    "\n",
    " ",
    "é",
    "\u00a0",
    "\u3000",
    "\u2028",
    "\x1c",
    "\U0001f600",
)
_PYTHON_WHITESPACE: tuple[str, ...] = tuple(re.findall(r"\s", "".join(map(chr, range(0x3001)))))


def write_random_tree(*, root: Path, seed: int, entry_count: int) -> None:
    """Write a seeded tree of folders, files, hidden entries, and directory links."""

    rng: random.Random = random.Random(seed)
    directories: list[Path] = [root]
    creators: tuple[Callable[[random.Random, Path, list[Path]], None], ...] = (
        *(_add_directory,) * 9,
        *(_add_file,) * 9,
        *(_add_link,) * 2,
    )
    for _index in range(entry_count):
        rng.choice(creators)(rng, rng.choice(directories), directories)


def _is_module(path: Path) -> bool:
    return path.stem != "__init__"


def snapshot_glob_mismatches(*, root: Path) -> tuple[str, ...]:
    """Return every root where the native Python module listing disagrees with ``Path.rglob``."""

    directories: list[Path] = sorted(filter(Path.is_dir, root.rglob("*")))
    labels: list[str] = []
    matches: list[bool] = []
    for directory in directories:
        relative_root: str = directory.relative_to(root).as_posix()
        expected: list[Path] = list(filter(_is_module, sorted(directory.rglob("*.py"))))
        labels.append(f"{directory}:*.py")
        matches.append(
            list(
                _native.discover_python_module_files(
                    project_dir=root, root=relative_root, public_only=False
                )
            )
            == expected
        )
    labels.append("absent root")
    matches.append(
        _native.discover_python_module_files(project_dir=root, root="absent", public_only=False)
        == ()
    )
    return tuple(compress(labels, [not matched for matched in matches]))


def write_scoped_project(*, root: Path) -> None:
    """Write a project with resources, scoped and grouped declarations, and global roles."""

    for relative_path, contents in _SCOPED_PROJECT_FILES.items():
        path: Path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def isolated_discovery_mismatches(*, project_dir: Path) -> tuple[str, ...]:
    """Return categories where one shared-snapshot pass differs from isolated discovery."""

    inputs: DiscoveredProjectInputs = _native.build_discovered_project_inputs(
        project_dir=project_dir,
        project_config=load_project_config(project_dir=project_dir),
        local_config=load_local_config(project_dir=project_dir),
        sql_analysis_enabled=True,
    )
    return tuple(
        compress(
            _ISOLATED_DISCOVERERS,
            [
                getattr(inputs, name) != discover(project_dir=project_dir)
                for name, discover in _ISOLATED_DISCOVERERS.items()
            ],
        )
    )


def model_paths(*, inputs: DiscoveredProjectInputs) -> tuple[str, ...]:
    """Return discovered model paths in discovery order."""

    return tuple(file.relative_path.as_posix() for file in inputs.model_files)


def _python_header_offsets(contents: str) -> tuple[int, int, int] | None:
    return _mapped_offsets(_REFERENCE_MODEL_HEADER_PATTERN.match(contents))


def _mapped_offsets(header_match: re.Match[str] | None) -> tuple[int, int, int] | None:
    return {
        True: lambda: None,
        False: lambda: (
            cast(re.Match[str], header_match).start("header"),
            cast(re.Match[str], header_match).end("header"),
            cast(re.Match[str], header_match).start("sql"),
        ),
    }[header_match is None]()


def _random_model_file(rng: random.Random) -> str:
    prefix: str = "".join(rng.choice(_PYTHON_WHITESPACE) for _ in range(rng.randint(0, 2)))
    body: str = "".join(rng.choice(_HEADER_FRAGMENTS) for _ in range(rng.randint(0, 14)))
    return f"{prefix}MODEL{rng.choice(_PYTHON_WHITESPACE)}({body}"


def _add_directory(rng: random.Random, parent: Path, directories: list[Path]) -> None:
    path: Path = parent / rng.choice(_DIRECTORY_NAMES)
    path.mkdir(exist_ok=True)
    directories.append(path)


def _add_file(rng: random.Random, parent: Path, _directories: list[Path]) -> None:
    (parent / rng.choice(_FILE_NAMES)).write_text("SELECT 1\n", encoding="utf-8")


def _add_link(rng: random.Random, parent: Path, directories: list[Path]) -> None:
    link: Path = parent / rng.choice(_LINK_NAMES)
    link.unlink(missing_ok=True)
    link.symlink_to(rng.choice([*directories, parent / "missing"]), target_is_directory=True)
