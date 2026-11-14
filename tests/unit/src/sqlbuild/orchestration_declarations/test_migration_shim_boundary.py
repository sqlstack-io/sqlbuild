"""Only `sqb migrate orchestration` reaches the removed Python job records."""

import ast
from pathlib import Path

import pytest

from tests.unit.src.sqlbuild.orchestration_declarations._test_types import ShimImportCase

_LEGACY_AUTHORING: str = "sqlbuild.orchestration_declarations.models.legacy_authoring"
_MIGRATE: str = "sqlbuild.orchestration_declarations.main.migrate_orchestration"


def _python_references(path: Path) -> set[str]:
    """Modules `path` imports, plus dotted strings it could hand to an import function."""
    found: set[str] = set()
    for node in ast.walk(ast.parse(path.read_text(), filename=str(path))):
        if isinstance(node, ast.Import):
            found.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module is not None:
            found.add(node.module)
            found.update(f"{node.module}.{alias.name}" for alias in node.names)
        elif isinstance(node, ast.Constant) and isinstance(node.value, str):
            found.add(node.value)
    return found


@pytest.mark.parametrize(
    "test_case",
    [
        ShimImportCase(
            description="the removed job records reach only the migration command",
            module=_LEGACY_AUTHORING,
            allowed_python_importers=frozenset(
                {"src/sqlbuild/orchestration_declarations/main/migrate_orchestration.py"}
            ),
            # Names the module only to skip its frames when locating a declaration's line.
            allowed_rust_references=frozenset(
                {"crates/sqlbuild-python/src/bindings/_helpers/native_host/constants.rs"}
            ),
        ),
        ShimImportCase(
            description="the migration runs only from `sqb migrate orchestration`",
            module=_MIGRATE,
            allowed_python_importers=frozenset(),
            allowed_rust_references=frozenset(
                {"crates/sqlbuild-python/src/bindings/_helpers/workspace_commands/commands.rs"}
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_migration_shim_when_scanning_sources_then_only_the_migrate_command_reaches_it(
    test_case: ShimImportCase,
) -> None:
    python_importers: set[str] = {
        path.as_posix()
        for path in Path("src/sqlbuild").rglob("*.py")
        if test_case.module in _python_references(path)
    }
    rust_references: set[str] = {
        path.as_posix()
        for path in Path("crates").rglob("*.rs")
        if test_case.module in path.read_text()
    }

    assert python_importers == test_case.allowed_python_importers
    assert rust_references == test_case.allowed_rust_references
