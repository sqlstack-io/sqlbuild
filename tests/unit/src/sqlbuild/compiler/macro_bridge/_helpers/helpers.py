from __future__ import annotations

import datetime
import decimal
import json
import os
import sys
import types
import zipfile
import zipimport
from dataclasses import replace
from importlib.machinery import ModuleSpec
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path

import sqlbuild._native as _native
from sqlbuild._native import (
    ConstantDeclaration,
    DeclarationResolutionContext,
    LoadedMacro,
    MacroContext,
    SqlLogicalType,
    SqlValue,
    SqlValueKind,
)
from sqlbuild.compiler.macro_bridge._helpers.store_environment import (
    project_fingerprint,
    store_environment,
)

BASE_CONTEXT: MacroContext = MacroContext(
    adapter_name="duckdb",
    sql_analysis_enabled=False,
    target_name="dev",
    vars={
        "region": "north",
        "limits": [1, 2.5, decimal.Decimal("3.10")],
        "since": datetime.date(2026, 1, 1),
    },
)


def with_vars(**values: object) -> MacroContext:
    """The base context with some vars replaced."""

    return replace(BASE_CONTEXT, vars={**BASE_CONTEXT.vars, **values})


def constant_declarations(value: int) -> DeclarationResolutionContext:
    """Declarations holding one integer constant."""

    return DeclarationResolutionContext(
        constants={
            "base_rate": ConstantDeclaration(
                name="base_rate",
                value=SqlValue(logical_type=SqlLogicalType(kind=SqlValueKind.INTEGER), value=value),
                relative_path=Path("sql/constants/base_rate.sql"),
            )
        }
    )


def loaded_macro(*, relative_path: Path, raw_source: str) -> LoadedMacro:
    """A loaded `cents` macro from the given file."""

    return LoadedMacro(
        name="cents",
        file_path=Path("/project") / relative_path,
        relative_path=relative_path,
        raw_source=raw_source,
        function=lambda column: column,
    )


def write_module(path: Path, text: str) -> bytes:
    """Write a module file and return metadata recording its current content digest."""

    _ = path.write_text(text, encoding="utf-8")
    return json.dumps([[str(path), _native.digest_files([str(path)])[0]]]).encode()


def backdated_module(path: Path) -> bytes:
    """Record a module, then replace it with other bytes of the same size and modification time."""

    metadata: bytes = write_module(path, "VALUE = 1\n")
    status: os.stat_result = path.stat()
    _ = path.write_text("VALUE = 2\n", encoding="utf-8")
    os.utime(path, ns=(status.st_atime_ns, status.st_mtime_ns))
    return metadata


def removed_module(path: Path) -> bytes:
    """Record a module, then remove it."""

    metadata: bytes = write_module(path, "VALUE = 1\n")
    path.unlink()
    return metadata


def file_module(root: Path) -> object:
    """A module loaded from a source file."""

    path: Path = root / "flavor.py"
    _ = path.write_text("VALUE = 1\n", encoding="utf-8")
    spec: ModuleSpec | None = spec_from_file_location("flavor", path)
    assert spec is not None
    return module_from_spec(spec)


def zip_member_module(root: Path) -> object:
    """A module whose location is a member of a zip archive on the import path."""

    archive: Path = root / "flavors.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr("zipped_flavor.py", "VALUE = 1\n")
    return _located_module("zipped_flavor", str(archive / "zipped_flavor.py"))


def launcher_main_module(root: Path) -> object:
    """The entry point of a console-script launcher that runs `__main__` from its own zip."""

    archive: Path = root / "sqb.exe"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr("__main__.py", "import sys\n")
    spec: ModuleSpec | None = zipimport.zipimporter(str(archive)).find_spec("__main__")
    assert spec is not None
    return module_from_spec(spec)


def zipped_module(root: Path) -> object:
    """A module other than `__main__` imported from a zip archive on the import path."""

    archive: Path = root / "flavors.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr("zipped_flavor.py", "VALUE = 1\n")
    spec: ModuleSpec | None = zipimport.zipimporter(str(archive)).find_spec("zipped_flavor")
    assert spec is not None
    return module_from_spec(spec)


def _located_module(name: str, origin: str) -> object:
    spec: ModuleSpec = ModuleSpec(name, None, origin=origin)
    spec.has_location = True
    return module_from_spec(spec)


def moved_module(root: Path) -> object:
    """A module loaded from a file that has since been moved away."""

    module: object = file_module(root)
    _ = (root / "flavor.py").rename(root / "flavor_moved.py")
    return module


def custom_loader_module(_root: Path) -> object:
    """A module from a loader that reports no file location."""

    return module_from_spec(ModuleSpec("generated_flavor", None, origin="generated"))


def synthetic_module(_root: Path) -> object:
    """A module object created in code, without an import spec."""

    return types.ModuleType("synthetic_flavor")


def interpreter_module(_root: Path) -> object:
    """A module built into the interpreter."""

    return sys.modules["sys"]


def standard_library_module(_root: Path) -> object:
    """A standard library module loaded from source."""

    return sys.modules["json"]


def sqlbuild_module(_root: Path) -> object:
    """A module of SQLBuild itself."""

    return sys.modules["sqlbuild.compiler.macro_bridge.constants"]


def write_environment_project(root: Path) -> None:
    """Write a model and a macro for store environment checks."""

    (root / "sql" / "models").mkdir()
    (root / "sql" / "macros" / "python").mkdir(parents=True)
    _ = (root / "sql/models/orders.sql").write_text("SELECT 1\n", encoding="utf-8")
    _ = (root / "sql/macros/python/common.py").write_text("X = 1\n", encoding="utf-8")


EXCLUDED_MODEL_PATHS: list[str] = ["sql/models/orders.sql"]


def environment_of(root: Path) -> str:
    """The store environment of a project, excluding its one model file."""

    return store_environment(
        project_dir=root,
        fingerprint=project_fingerprint(project_dir=root, model_paths=EXCLUDED_MODEL_PATHS),
    )


def deeply_nested_list(*, depth: int) -> list[object]:
    """Return a list nested `depth` levels deep."""

    nested: list[object] = []
    for _ in range(depth):
        nested = [nested]
    return nested
