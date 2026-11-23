"""Editing an outside module behind a stored macro call must not replay a stale compile."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    StoredMacroDeltaFallbackTestCase,
    StoredMacroModuleEditTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    CompileReuseRun,
    IncrementalEditComparison,
    add_stored_flavor_macro,
    compiled_text,
    logged_calls,
    replace_project_text,
    run_reuse_compile,
    staging_comment,
    write_external_modules,
)

_MODULE: dict[str, str] = {"extflavor.py": "VALUE = \"'vanilla'\"\n"}
_EDITED_MODULE: dict[str, str] = {"extflavor.py": "VALUE = \"'choco'\"\n"}
_PACKAGE: dict[str, str] = {
    "extpkg/__init__.py": "",
    "extpkg/flavor.py": "from extpkg._values import VALUE\n",
    "extpkg/_values.py": "VALUE = \"'vanilla'\"\n",
}
_EDITED_PACKAGE: dict[str, str] = {"extpkg/_values.py": "VALUE = \"'choco'\"\n"}


@pytest.mark.parametrize(
    "test_case",
    [
        StoredMacroModuleEditTestCase(
            description="module_default",
            import_name="extflavor",
            module_files=_MODULE,
            edited_files=_EDITED_MODULE,
            expected_compiled_value="'choco' AS flavor",
        ),
        StoredMacroModuleEditTestCase(
            description="package_transitive_import_default",
            import_name="extpkg.flavor",
            module_files=_PACKAGE,
            edited_files=_EDITED_PACKAGE,
            expected_compiled_value="'choco' AS flavor",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_call_reused_when_its_lazily_imported_module_changes_then_output_is_fresh(
    compile_reuse_project: Path, tmp_path: Path, test_case: StoredMacroModuleEditTestCase
) -> None:
    extlib: Path = tmp_path / "extlib"
    log_path: Path = tmp_path / "flavor-calls.log"
    write_external_modules(extlib=extlib, files=test_case.module_files)
    env: dict[str, str] = add_stored_flavor_macro(
        project_dir=compile_reuse_project,
        extlib=extlib,
        import_name=test_case.import_name,
        log_path=log_path,
    )
    cold: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)
    staging_comment(compile_reuse_project)
    store_hit: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)
    calls_before_module_edit: int = logged_calls(log_path)

    write_external_modules(extlib=extlib, files=test_case.edited_files)
    comparison: IncrementalEditComparison = IncrementalEditComparison(
        incremental=run_reuse_compile(project_dir=compile_reuse_project, env=env),
        reference=run_reuse_compile(
            project_dir=compile_reuse_project, env=env, args=("--no-cache",)
        ),
    )

    assert (
        cold.returncode,
        store_hit.returncode,
        store_hit.reused,
        calls_before_module_edit,
        comparison.incremental.reused,
        comparison.matches,
        test_case.expected_compiled_value
        in compiled_text(run=comparison.incremental, suffix="fact_orders.sql"),
    ) == (0, 0, False, 1, False, True, True), (
        comparison.mismatched_artifacts,
        store_hit.stderr,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        StoredMacroDeltaFallbackTestCase(
            description="a model error found by the delta round",
            edited_path="sql/models/staging/stg_customers.sql",
            old="  email,\n",
            new="  email,\n  unknown_column,\n",
            expected_outcome=(0, 1, False, 1),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_call_when_a_delta_falls_back_after_its_round_then_the_full_compile_reuses_it(
    compile_reuse_project: Path, tmp_path: Path, test_case: StoredMacroDeltaFallbackTestCase
) -> None:
    extlib: Path = tmp_path / "extlib"
    log_path: Path = tmp_path / "flavor-calls.log"
    write_external_modules(extlib=extlib, files=_MODULE)
    env: dict[str, str] = add_stored_flavor_macro(
        project_dir=compile_reuse_project, extlib=extlib, import_name="extflavor", log_path=log_path
    )
    cold: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)
    replace_project_text(compile_reuse_project, test_case.edited_path, test_case.old, test_case.new)

    edited: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)

    assert (
        cold.returncode,
        edited.returncode,
        edited.reused,
        logged_calls(log_path),
    ) == test_case.expected_outcome, edited.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
