from collections.abc import Callable
from pathlib import Path
from typing import cast

from sqlbuild import _native
from sqlbuild._native import (
    DiscoveredConstantFile,
    DiscoveredEnumFile,
    DiscoveredMacroFile,
    DiscoveredPythonFunctionFile,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestCase,
    LocalConfig,
    ProjectConfig,
    SqlTestParameterDeclaration,
    discover_audit_files,
    discover_constant_files,
    discover_enum_files,
    discover_macro_files,
    discover_model_schema_files,
    discover_python_function_files,
    discover_sql_function_files,
    discover_sql_hook_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    LoadProjectConfigTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery.helpers import (
    parse_model_sql,
    parse_sql_scenario_file,
    parse_sql_test_file,
)

_DECLARATION_CONTENTS_BY_KIND: dict[str, tuple[str, str]] = {
    "macro": (".py", "def scoped_value():\n    return 1\n"),
    "enum": (".sql", "ENUM (name scoped_value, members [ONE, TWO]);\n"),
    "constant": (".sql", "CONSTANT (name scoped_value, value 1);\n"),
}
_DECLARATION_DISCOVERER_BY_KIND: dict[
    str,
    Callable[
        ...,
        tuple[DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile, ...],
    ],
] = {
    "macro": discover_macro_files,
    "enum": discover_enum_files,
    "constant": discover_constant_files,
}


def expected_or_actual[T](expected: T | None, actual: T) -> T:
    return (actual, cast(T, expected))[expected is not None]


def write_project_config_test_files(
    *, tmp_path: Path, test_case: LoadProjectConfigTestCase
) -> None:
    project_file: Path = tmp_path / "sqlbuild_project.toml"
    project_file.write_text(test_case.project_file_contents, encoding="utf-8")


def declaration_contents(*, kind: str) -> tuple[str, str]:
    return _DECLARATION_CONTENTS_BY_KIND[kind]


def declaration_kind_folder(*, kind: str) -> Path:
    """Return the folder a declaration file of this kind lives in below its root."""

    return {"macro": Path("python")}.get(kind, Path())


def discover_declarations(
    *, project_dir: Path, kind: str
) -> tuple[DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile, ...]:
    return _DECLARATION_DISCOVERER_BY_KIND[kind](project_dir=project_dir)


def discovered_test_parameters(
    *, blocks: tuple[DiscoveredSqlTestBlock, ...]
) -> tuple[SqlTestParameterDeclaration, ...]:
    parameters: list[SqlTestParameterDeclaration] = []
    block: DiscoveredSqlTestBlock
    for block in blocks:
        parameters.extend(block.parameters)
    return tuple(parameters)


def discovered_test_cases(
    *, blocks: tuple[DiscoveredSqlTestBlock, ...]
) -> tuple[DiscoveredSqlTestCase, ...]:
    cases: list[DiscoveredSqlTestCase] = []
    block: DiscoveredSqlTestBlock
    for block in blocks:
        cases.extend(block.cases)
    return tuple(cases)


def discovered_test_case_values(
    *, cases: tuple[DiscoveredSqlTestCase, ...]
) -> tuple[tuple[object, ...], ...]:
    case_values: list[tuple[object, ...]] = []
    test_case: DiscoveredSqlTestCase
    for test_case in cases:
        case_values.append(tuple(value.value for _name, value in test_case.values))
    return tuple(case_values)


def write_unreadable_files(*, project_dir: Path, relative_paths: tuple[str, ...]) -> None:
    """Write files whose bytes are not valid UTF-8 under project_dir."""

    relative_path: str
    for relative_path in relative_paths:
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        _ = file_path.write_bytes(b"\xff\xfe\xfa")


_DECLARATION_FILE_DISCOVERERS: dict[str, Callable[..., tuple[object, ...]]] = {
    "enums": discover_enum_files,
    "constants": discover_constant_files,
    "schemas": discover_model_schema_files,
    "functions": discover_sql_function_files,
    "hooks": discover_sql_hook_files,
    "audits": discover_audit_files,
}


def discover_declaration_file(*, project_dir: Path, relative_path: str, contents: str) -> object:
    """Write one declaration file into a project and return the file discovery reads from it."""

    file_path: Path = project_dir / relative_path
    file_path.parent.mkdir(parents=True, exist_ok=True)
    _ = file_path.write_text(contents, encoding="utf-8")
    role: str = relative_path.removeprefix("sql/").split("/")[0]
    discovered: tuple[object, ...] = _DECLARATION_FILE_DISCOVERERS[role](
        project_dir=project_dir
    )
    return discovered[0]


def discover_python_function_file(
    *, project_dir: Path, file_name: str, contents: str
) -> DiscoveredPythonFunctionFile:
    """Write one ``sql/functions/python`` file into a project and return its discovered record."""

    file_path: Path = project_dir / "sql" / "functions" / "python" / file_name
    file_path.parent.mkdir(parents=True, exist_ok=True)
    _ = file_path.write_text(contents, encoding="utf-8")
    return discover_python_function_files(project_dir=project_dir)[0]


_STATEMENT_HEADER_PARSERS: dict[str, Callable[..., object]] = {
    "MODEL": parse_model_sql,
    "AUDIT": lambda *, contents, file_path: discover_declaration_file(
        project_dir=file_path.parents[2],
        relative_path=file_path.relative_to(file_path.parents[2]).as_posix(),
        contents=contents,
    ),
    "TEST": parse_sql_test_file,
    "HOOK": lambda *, contents, file_path: discover_declaration_file(
        project_dir=file_path.parents[2],
        relative_path=file_path.relative_to(file_path.parents[2]).as_posix(),
        contents=contents,
    ),
    "SCENARIO": lambda *, contents, file_path: parse_sql_scenario_file(
        contents=contents, file_path=file_path, relative_path=file_path
    ),
}


_STATEMENT_FILE_PATHS: dict[str, Callable[[Path], Path]] = {
    "AUDIT": lambda project_dir: project_dir / "sql" / "audits" / "generic" / "orders.sql",
    "HOOK": lambda project_dir: project_dir / "sql" / "hooks" / "sql" / "orders.sql",
}


def statement_header_file_path(*, statement: str, project_dir: Path) -> Path:
    """Return where a statement file is parsed from: in the project for declaration kinds."""

    return _STATEMENT_FILE_PATHS.get(statement, lambda _project_dir: Path("orders.sql"))(
        project_dir
    )


def parse_statement_header_file(*, statement: str, contents: str, file_path: Path) -> object:
    """Parse one authored statement file with the discovery parser for its header kind."""

    return _STATEMENT_HEADER_PARSERS[statement](contents=contents, file_path=file_path)


def load_project_config(*, project_dir: Path) -> ProjectConfig:
    """Load the shared project configuration through the native reader."""

    return _native.load_project_config_file(project_dir)


def load_local_config(*, project_dir: Path) -> LocalConfig:
    """Load the local configuration overrides through the native reader."""

    return _native.load_local_config_file(project_dir)
