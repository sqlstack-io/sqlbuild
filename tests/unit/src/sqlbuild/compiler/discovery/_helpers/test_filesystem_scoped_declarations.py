"""Tests for bounded scoped declaration filesystem discovery."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Protocol

import pytest

from sqlbuild._native import (
    DeclarationParseError,
    DiscoveredConstantFile,
    DiscoveredEnumFile,
    DiscoveredMacroFile,
    DiscoveredProjectInputs,
    discover_audit_files,
    discover_hook_functions,
    discover_macro_files,
    discover_project_inputs,
    discover_python_function_files,
    discover_scenario_files,
    discover_source_files,
    discover_sql_function_files,
    discover_sql_hook_files,
    discover_test_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverGlobalDeclarationTestCase,
    DiscoverGroupedDeclarationTestCase,
    DiscoverScopedDeclarationTestCase,
    DiscoveryPathInventoryTestCase,
    InvalidScopedDeclarationRootTestCase,
    OrdinaryDiscoveryExclusionTestCase,
    OrdinaryFolderTestCase,
    StrictScopedDiscoveryTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import (
    declaration_contents,
    declaration_kind_folder,
    discover_declarations,
)
from tests.unit.src.sqlbuild.compiler.discovery.helpers import discover_all_model_files


class _DiscoveredFile(Protocol):
    relative_path: Path


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverScopedDeclarationTestCase("models", "models", "models"),
        DiscoverScopedDeclarationTestCase("unit_tests", "tests/unit", "tests/unit"),
        DiscoverScopedDeclarationTestCase("scenarios", "tests/scenarios", "tests/scenarios"),
        DiscoverScopedDeclarationTestCase("sql_functions", "sql/functions/sql", "sql/functions/sql"),
        DiscoverScopedDeclarationTestCase("sources", "sources", "sources"),
    ),
    ids=lambda case: case.description,
)
def test_given_declaration_below_authored_root_when_discovering_then_records_bounded_scope_facts(
    test_case: DiscoverScopedDeclarationTestCase,
    tmp_path: Path,
) -> None:
    for declaration_kind in ("macro", "enum", "constant"):
        for scope_kind in ("inherited", "local"):
            suffix, contents = declaration_contents(kind=declaration_kind)
            directory_name: str = {
                "inherited": f"{declaration_kind}s",
                "local": f"_{declaration_kind}s",
            }[scope_kind]
            relative_root: Path = Path(test_case.authored_root) / "domain" / directory_name
            file_path: Path = (
                tmp_path
                / relative_root
                / declaration_kind_folder(kind=declaration_kind)
                / "organization"
                / f"z_value{suffix}"
            )
            file_path.parent.mkdir(parents=True)
            file_path.write_text(contents, encoding="utf-8")

            result: tuple[
                DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile, ...
            ] = discover_declarations(project_dir=tmp_path, kind=declaration_kind)

            assert len(result) == 1
            discovered: DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile = result[
                0
            ]
            assert discovered.relative_path == (
                relative_root
                / declaration_kind_folder(kind=declaration_kind)
                / "organization"
                / f"z_value{suffix}"
            )
            assert discovered.scope_kind.value == scope_kind
            assert discovered.ownership_root == Path(test_case.expected_ownership_root)
            assert discovered.owning_path == Path(test_case.authored_root) / "domain"
            assert discovered.declaration_root == relative_root
            file_path.unlink()


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverGroupedDeclarationTestCase(
            "public_macro", "macro", "macros", "inherited", "sql/models/orders"
        ),
        DiscoverGroupedDeclarationTestCase(
            "private_macro", "macro", "_macros", "local", "sql/models/orders"
        ),
        DiscoverGroupedDeclarationTestCase(
            "public_enum", "enum", "enums", "inherited", "sql/models/orders"
        ),
        DiscoverGroupedDeclarationTestCase(
            "private_enum", "enum", "_enums", "local", "sql/models/orders"
        ),
        DiscoverGroupedDeclarationTestCase(
            "public_constant", "constant", "constants", "inherited", "sql/models/orders"
        ),
        DiscoverGroupedDeclarationTestCase(
            "private_constant", "constant", "_constants", "local", "sql/models/orders"
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_grouped_declaration_when_discovering_then_owner_is_above_sqlbuild_group(
    test_case: DiscoverGroupedDeclarationTestCase,
    tmp_path: Path,
) -> None:
    suffix, contents = declaration_contents(kind=test_case.declaration_kind)
    declaration_root: Path = Path("sql/models/orders/_sqlbuild") / test_case.directory_name
    file_name: Path = declaration_kind_folder(kind=test_case.declaration_kind) / f"policy{suffix}"
    file_path: Path = tmp_path / declaration_root / file_name
    file_path.parent.mkdir(parents=True)
    file_path.write_text(contents, encoding="utf-8")

    discovered: tuple[DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile, ...] = (
        discover_declarations(project_dir=tmp_path, kind=test_case.declaration_kind)
    )

    assert len(discovered) == 1
    assert discovered[0].relative_path == declaration_root / file_name
    assert discovered[0].scope_kind.value == test_case.expected_scope_kind
    assert discovered[0].owning_path == Path(test_case.expected_owning_path)
    assert discovered[0].declaration_root == declaration_root


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverGlobalDeclarationTestCase("macro", "macro", "macros", "global"),
        DiscoverGlobalDeclarationTestCase("enum", "enum", "enums", "global"),
        DiscoverGlobalDeclarationTestCase("constant", "constant", "constants", "global"),
    ),
    ids=lambda case: case.description,
)
def test_given_top_level_public_declarations_when_discovering_then_scope_remains_global(
    test_case: DiscoverGlobalDeclarationTestCase,
    tmp_path: Path,
) -> None:
    suffix, contents = declaration_contents(kind=test_case.declaration_kind)
    file_path: Path = (
        tmp_path
        / test_case.directory_name
        / declaration_kind_folder(kind=test_case.declaration_kind)
        / "organization"
        / f"value{suffix}"
    )
    file_path.parent.mkdir(parents=True)
    file_path.write_text(contents, encoding="utf-8")

    discovered: DiscoveredMacroFile | DiscoveredEnumFile | DiscoveredConstantFile = (
        discover_declarations(project_dir=tmp_path, kind=test_case.declaration_kind)[0]
    )

    assert discovered.scope_kind.value == test_case.expected_scope_kind
    assert discovered.ownership_root == Path(test_case.directory_name)
    assert discovered.owning_path is None
    assert discovered.declaration_root == Path(test_case.directory_name)


@pytest.mark.parametrize(
    "test_case",
    (
        InvalidScopedDeclarationRootTestCase(
            "top_level_inherited",
            "_macros/python/value.py",
            "must be below a canonical authored root",
        ),
        InvalidScopedDeclarationRootTestCase(
            "nested_scoped_roots",
            "sql/models/macros/organization/_constants/value.sql",
            "nested inside another declaration tree",
        ),
        InvalidScopedDeclarationRootTestCase(
            "scoped_root_below_global_tree",
            "sql/macros/organization/_macros/python/value.py",
            "nested inside another declaration tree",
        ),
        InvalidScopedDeclarationRootTestCase(
            "global_root_below_global_tree",
            "sql/macros/organization/constants/value.sql",
            "nested inside another declaration tree",
        ),
        InvalidScopedDeclarationRootTestCase(
            "global_root_below_scoped_tree",
            "sql/models/macros/organization/enums/value.sql",
            "nested inside another declaration tree",
        ),
        InvalidScopedDeclarationRootTestCase(
            "grouped_root_with_intermediate_folder",
            "sql/models/orders/_sqlbuild/organization/macros/python/value.py",
            "contains unsupported entries",
        ),
        InvalidScopedDeclarationRootTestCase(
            "top_level_grouped_root",
            "_sqlbuild/_macros/python/value.py",
            "must be below a canonical authored root",
        ),
        InvalidScopedDeclarationRootTestCase(
            "authored_root_grouped_root",
            "sql/models/_sqlbuild/macros/python/value.py",
            "must be below a concrete owner directory",
        ),
        InvalidScopedDeclarationRootTestCase(
            "grouped_root_with_unsupported_entry",
            "sql/models/orders/_sqlbuild/notes.py",
            "contains unsupported entries",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_declaration_root_when_discovering_then_raises(
    test_case: InvalidScopedDeclarationRootTestCase,
    tmp_path: Path,
) -> None:
    file_path: Path = tmp_path / test_case.relative_path
    file_path.parent.mkdir(parents=True)
    file_path.write_text("", encoding="utf-8")

    with pytest.raises(DeclarationParseError, match=test_case.expected_error_fragment):
        discover_macro_files(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        OrdinaryFolderTestCase(
            description="old local spelling is ordinary",
            root_relative_path="_local_constants/value.sql",
            model_relative_path="sql/models/_local_constants/orders.sql",
            expected_model_relative_paths=("sql/models/_local_constants/orders.sql",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_old_local_spelling_when_discovering_then_directory_has_no_special_meaning(
    test_case: OrdinaryFolderTestCase,
    tmp_path: Path,
) -> None:
    root_file: Path = tmp_path / test_case.root_relative_path
    model_file: Path = tmp_path / test_case.model_relative_path
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    root_file.parent.mkdir(parents=True)
    model_file.parent.mkdir(parents=True)
    root_file.write_text("arbitrary project file", encoding="utf-8")
    model_file.write_text("MODEL ();\nSELECT 1", encoding="utf-8")

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    assert tuple(file.relative_path for file in discovered.model_files) == tuple(
        Path(path) for path in test_case.expected_model_relative_paths
    )
    assert discovered.constant_files == ()


@pytest.mark.parametrize(
    "test_case",
    (
        StrictScopedDiscoveryTestCase(
            "top_level_enum",
            "_enums/value.sql",
            "ENUM (name value, members [ONE]);\n",
            "must be below a canonical authored root",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_scoped_root_when_discovering_project_inputs_then_strict_discovery_raises(
    test_case: StrictScopedDiscoveryTestCase,
    tmp_path: Path,
) -> None:
    config_path: Path = tmp_path / "sqlbuild_project.toml"
    config_path.write_text('name = "demo"\nadapter = "duckdb"\n', encoding="utf-8")
    file_path: Path = tmp_path / test_case.relative_path
    file_path.parent.mkdir()
    file_path.write_text(test_case.contents, encoding="utf-8")

    with pytest.raises(DeclarationParseError, match=test_case.expected_error_fragment):
        discover_project_inputs(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        OrdinaryDiscoveryExclusionTestCase(
            "model",
            "sql/models/domain/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "model",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "grouped_model",
            "sql/models/domain/_sqlbuild/constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "model",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "unit_test",
            "tests/unit/domain/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "test",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "scenario",
            "tests/scenarios/domain/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "scenario",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "hook",
            "sql/models/domain/_sqlbuild/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "hook",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "function",
            "sql/functions/sql/domain/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "function",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "audit",
            "sql/models/domain/_sqlbuild/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "audit",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "source",
            "sql/sources/domain/_constants/value.sql",
            "CONSTANT (name scoped_value, value 1);\n",
            "source",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "python_function",
            "sql/functions/sql/domain/_macros/python/value.py",
            "raise RuntimeError('must not import')\n",
            "python_function",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "python_hook",
            "sql/models/domain/_sqlbuild/_macros/python/value.py",
            "raise RuntimeError('must not import')\n",
            "python_hook",
            (),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "project_macro_sql_test",
            "tests/unit/macros/orders/test_order_policy__returns_quantity.sql",
            'TEST (mode macro, name "order_policy__returns_quantity");\nSELECT 1\n',
            "test",
            ("tests/unit/macros/orders/test_order_policy__returns_quantity.sql",),
        ),
        OrdinaryDiscoveryExclusionTestCase(
            "sql_file_in_test_helper_role",
            "tests/unit/orders/_sqlbuild/macros/not_a_test.sql",
            'TEST (mode macro, name "order_policy__returns_quantity");\nSELECT 1\n',
            "test",
            (),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_file_in_declaration_role_when_discovering_resources_then_returns_expected_files(
    test_case: OrdinaryDiscoveryExclusionTestCase,
    tmp_path: Path,
) -> None:
    file_path: Path = tmp_path / test_case.relative_path
    file_path.parent.mkdir(parents=True)
    file_path.write_text(test_case.contents, encoding="utf-8")
    discoverers: dict[str, Callable[..., tuple[_DiscoveredFile, ...]]] = {
        "model": discover_all_model_files,
        "test": discover_test_files,
        "scenario": discover_scenario_files,
        "hook": discover_sql_hook_files,
        "function": discover_sql_function_files,
        "audit": discover_audit_files,
        "source": discover_source_files,
        "python_function": discover_python_function_files,
        "python_hook": discover_hook_functions,
    }

    result: tuple[_DiscoveredFile, ...] = discoverers[test_case.discoverer_name](
        project_dir=tmp_path
    )

    assert tuple(item.relative_path.as_posix() for item in result) == (
        test_case.expected_relative_paths
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoveryPathInventoryTestCase(
            "similarly_prefixed_directory",
            ("sql/models/_constants_archive/orders.sql",),
            ("sql/models/_constants_archive/orders.sql",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_similarly_prefixed_directory_when_discovering_then_treats_it_as_ordinary_resource(
    test_case: DiscoveryPathInventoryTestCase,
    tmp_path: Path,
) -> None:
    file_path: Path = tmp_path / test_case.files[0]
    file_path.parent.mkdir(parents=True)
    file_path.write_text("MODEL ();\nSELECT 1\n", encoding="utf-8")

    result: tuple[_DiscoveredFile, ...] = discover_all_model_files(project_dir=tmp_path)

    assert tuple(file.relative_path.as_posix() for file in result) == (
        test_case.expected_relative_paths
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoveryPathInventoryTestCase(
            "unsorted_scoped_macros",
            (
                "sql/models/z/_macros/python/z.py",
                "sql/models/a/_macros/python/z.py",
                "sql/models/a/_macros/python/a.py",
                "sql/models/a/_macros/python/__init__.py",
            ),
            (
                "sql/models/a/_macros/python/a.py",
                "sql/models/a/_macros/python/z.py",
                "sql/models/z/_macros/python/z.py",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unsorted_scoped_files_and_initializer_when_discovering_then_sorts_and_skips_initializer(
    test_case: DiscoveryPathInventoryTestCase,
    tmp_path: Path,
) -> None:
    for relative_path in test_case.files:
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text("def value():\n    return 1\n", encoding="utf-8")

    result: tuple[DiscoveredMacroFile, ...] = discover_macro_files(project_dir=tmp_path)

    assert tuple(file.relative_path.as_posix() for file in result) == (
        test_case.expected_relative_paths
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
