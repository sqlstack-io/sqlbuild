from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    digest_module_files,
    module_digests_metadata,
    unchanged_module_digests,
)
from sqlbuild.compiler.macro_bridge._helpers.store_environment import module_sources
from sqlbuild.compiler.macro_bridge.models import ModuleSources
from tests.unit.src.sqlbuild.compiler.macro_bridge._helpers._test_types import (
    ModuleDigestsRoundTripTestCase,
    ModuleDigestsTestCase,
    ModuleSourcesTestCase,
    StoreEnvironmentTestCase,
    StoreEnvironmentVariableTestCase,
)
from tests.unit.src.sqlbuild.compiler.macro_bridge._helpers.helpers import (
    backdated_module,
    custom_loader_module,
    environment_of,
    file_module,
    interpreter_module,
    launcher_main_module,
    moved_module,
    removed_module,
    sqlbuild_module,
    standard_library_module,
    synthetic_module,
    write_environment_project,
    write_module,
    zip_member_module,
    zipped_module,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ModuleDigestsTestCase(
            description="unchanged module",
            change=lambda path: write_module(path, "VALUE = 1\n"),
            expected_valid=True,
        ),
        ModuleDigestsTestCase(
            description="module replaced keeping its size and modification time",
            change=backdated_module,
            expected_valid=False,
        ),
        ModuleDigestsTestCase(
            description="module removed", change=removed_module, expected_valid=False
        ),
        ModuleDigestsTestCase(
            description="unreadable metadata", change=lambda _path: b"{", expected_valid=False
        ),
        ModuleDigestsTestCase(
            description="unexpected metadata shape",
            change=lambda _path: b'[["flavor.py", 1]]',
            expected_valid=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_module_digests_when_validating_then_only_unchanged_contents_pass(
    tmp_path: Path, test_case: ModuleDigestsTestCase
) -> None:
    metadata: bytes = test_case.change(tmp_path / "flavor.py")

    digests: dict[str, str] | None = unchanged_module_digests(metadata=metadata, known={})

    assert (digests is not None) is test_case.expected_valid


@pytest.mark.parametrize(
    "test_case",
    [
        ModuleDigestsRoundTripTestCase(
            description="one module", module_text="VALUE = 1\n", expected_valid=True
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_module_digests_when_encoding_then_validation_returns_them(
    tmp_path: Path, test_case: ModuleDigestsRoundTripTestCase
) -> None:
    path: Path = tmp_path / "flavor.py"
    _ = write_module(path, test_case.module_text)
    digests: dict[str, str] | None = digest_module_files([str(path)])
    assert digests is not None

    validated: dict[str, str] | None = unchanged_module_digests(
        metadata=module_digests_metadata(digests), known={}
    )

    assert (validated == digests) is test_case.expected_valid


@pytest.mark.parametrize(
    "test_case",
    [
        StoreEnvironmentTestCase(
            description="no edit", edit=lambda _root: None, expected_unchanged=True
        ),
        StoreEnvironmentTestCase(
            description="model edit",
            edit=lambda root: (root / "sql/models/orders.sql").write_text("SELECT 2\n"),
            expected_unchanged=True,
        ),
        StoreEnvironmentTestCase(
            description="macro edit",
            edit=lambda root: (root / "sql/macros/python/common.py").write_text("X = 2\n"),
            expected_unchanged=False,
        ),
        StoreEnvironmentTestCase(
            description="model the caller does not exclude",
            edit=lambda root: (root / "sql/models/customers.sql").write_text("SELECT 1\n"),
            expected_unchanged=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_edit_when_digesting_store_environment_then_only_model_edits_keep_it(
    tmp_path: Path, test_case: StoreEnvironmentTestCase
) -> None:
    write_environment_project(tmp_path)
    original: str = environment_of(tmp_path)
    _ = test_case.edit(tmp_path)

    edited: str = environment_of(tmp_path)

    assert (edited == original) is test_case.expected_unchanged


@pytest.mark.parametrize(
    "test_case",
    [
        StoreEnvironmentVariableTestCase(
            description="variable a macro may read",
            name="SQB_STORE_REGION",
            value="south",
            expected_unchanged=False,
        ),
        StoreEnvironmentVariableTestCase(
            description="variable outside the tracked prefixes",
            name="STORE_REGION",
            value="south",
            expected_unchanged=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_environment_variable_change_when_digesting_store_environment_then_tracked_ones_count(
    tmp_path: Path, test_case: StoreEnvironmentVariableTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_environment_project(tmp_path)
    monkeypatch.delenv(test_case.name, raising=False)
    original: str = environment_of(tmp_path)
    monkeypatch.setenv(test_case.name, test_case.value)

    changed: str = environment_of(tmp_path)

    assert (changed == original) is test_case.expected_unchanged


@pytest.mark.parametrize(
    "test_case",
    [
        ModuleSourcesTestCase(
            description="source file",
            module=file_module,
            expected_paths=lambda root: (str(root / "flavor.py"),),
            expected_complete=True,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="zip archive member",
            module=zip_member_module,
            expected_paths=lambda root: (str(root / "flavors.zip" / "zipped_flavor.py"),),
            expected_complete=True,
            expected_digestible=False,
        ),
        ModuleSourcesTestCase(
            description="module imported from a zip archive",
            module=zipped_module,
            expected_paths=lambda root: (str(root / "flavors.zip" / "zipped_flavor.py"),),
            expected_complete=True,
            expected_digestible=False,
        ),
        ModuleSourcesTestCase(
            description="launcher entry point run from the launcher's own zip",
            module=launcher_main_module,
            expected_paths=lambda root: (str(root / "sqb.exe"),),
            expected_complete=True,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="file moved after loading",
            module=moved_module,
            expected_paths=lambda root: (str(root / "flavor.py"),),
            expected_complete=True,
            expected_digestible=False,
        ),
        ModuleSourcesTestCase(
            description="loader without a file location",
            module=custom_loader_module,
            expected_paths=lambda _root: (),
            expected_complete=False,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="module object created in code",
            module=synthetic_module,
            expected_paths=lambda _root: (),
            expected_complete=True,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="built into the interpreter",
            module=interpreter_module,
            expected_paths=lambda _root: (),
            expected_complete=True,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="standard library source",
            module=standard_library_module,
            expected_paths=lambda _root: (),
            expected_complete=True,
            expected_digestible=True,
        ),
        ModuleSourcesTestCase(
            description="sqlbuild itself",
            module=sqlbuild_module,
            expected_paths=lambda _root: (),
            expected_complete=True,
            expected_digestible=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loaded_module_when_identifying_its_code_then_only_readable_files_are_trusted(
    tmp_path: Path, test_case: ModuleSourcesTestCase
) -> None:
    module: object = test_case.module(tmp_path)

    sources: ModuleSources = module_sources([module])

    assert sources == ModuleSources(
        paths=test_case.expected_paths(tmp_path), complete=test_case.expected_complete
    )
    assert (digest_module_files(sources.paths) is not None) is test_case.expected_digestible
