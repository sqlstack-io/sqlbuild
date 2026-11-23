"""Compiles that reuse stored macro call results must match an uncached compile after every edit."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

from scripts.compiler_differential.constants import FAILURE_BASE_FILES
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    BrokenMacroCallStoreKeyTestCase,
    EngineMacroCallGateTestCase,
    MacroCallStoreEditSequenceTestCase,
    MacroCallStoreEditStep,
    MacroReferenceCallStoreTestCase,
    SecondCompileStoreTestCase,
    StaleMacroModuleStoreTestCase,
    StaleStoreArrangement,
    UnkeyableMacroCallTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    STORE_ARGUMENT_ENV_VAR,
    STORE_ENVIRONMENT_REGION_VAR,
    CompileReuseRun,
    FullMacroCallRuns,
    MacroCallStoreRun,
    MacroReferenceCallRuns,
    backdate_flavor_between_processes,
    block_proc_reads,
    compile_as_new_process_here,
    compile_in_new_process,
    edit_flavor_while_saving,
    edit_label_helper,
    edit_south_model,
    forget_store_flavor_module,
    freeze_macro_call_store_environment,
    full_macro_call_runs,
    in_process_reuse_run,
    logged_in_process_compile,
    macro_call_store_compile,
    macro_reference_call_runs,
    move_flavor_while_saving,
    prepare_macro_call_store_project,
    prepare_macro_reference_call_project,
    pretend_fresh_process,
    recompile_in_process_after_edit,
    replace_project_text,
    rezip_flavor_between_processes,
    run_reuse_compile,
    store_files,
    uncached_reference_compile,
    write_project_file,
    write_store_flavor,
)
from tests.integration.src.sqlbuild.compiler.compile.helpers import MACRO_CALL_LOG_ENV_VAR

_MACRO_REFERENCE_CALL_RUNS: int = 2
_MACRO_REFERENCE_CALL_MESSAGE: str = (
    "__ref(customers) is not a valid __ref() call, returned by macro customer_relation()"
)
_MACRO_REFERENCE_CALL_PATH: str = "sql/models/marts/_sqlbuild/_macros/python/customer_relation.py"
_EDIT_STEPS: tuple[MacroCallStoreEditStep, ...] = (
    MacroCallStoreEditStep(
        description="no_change",
        edit=lambda _root: None,
        expected_logged_calls=0,
    ),
    MacroCallStoreEditStep(
        description="model_comment",
        edit=lambda root: edit_south_model(root, "FROM (", "-- reviewed\nFROM ("),
        expected_logged_calls=0,
    ),
    MacroCallStoreEditStep(
        description="model_adds_a_call",
        edit=lambda root: edit_south_model(
            root, "  @py.target_label()", '  @py.logged("tax") AS logged_tax,\n  @py.target_label()'
        ),
        expected_logged_calls=1,
    ),
    MacroCallStoreEditStep(
        description="model_private_constant_read_by_context_macro",
        edit=lambda root: edit_south_model(root, "_south_bonus 3", "_south_bonus 4"),
        expected_logged_calls=0,
    ),
    MacroCallStoreEditStep(
        description="cli_vars_read_by_context_macro",
        edit=lambda _root: None,
        expected_logged_calls=0,
        args=("--vars", '{"region": "south"}'),
    ),
    MacroCallStoreEditStep(
        description="target_read_by_context_macro",
        edit=lambda _root: None,
        expected_logged_calls=0,
        args=("--target", "prod"),
    ),
    MacroCallStoreEditStep(
        description="model_adds_env_interpolated_argument",
        edit=lambda root: edit_south_model(
            root,
            "  @py.target_label()",
            f'  @py.logged("@@ENV:{STORE_ARGUMENT_ENV_VAR}") AS logged_env,\n  @py.target_label()',
        ),
        expected_logged_calls=1,
    ),
    MacroCallStoreEditStep(
        description="env_interpolated_argument_changes_call_text",
        edit=lambda _root: None,
        expected_logged_calls=1,
        env=((STORE_ARGUMENT_ENV_VAR, "second"),),
    ),
    MacroCallStoreEditStep(
        description="environment_variable_read_by_macro",
        edit=lambda _root: None,
        expected_logged_calls=3,
        env=((STORE_ENVIRONMENT_REGION_VAR, "south"),),
    ),
    MacroCallStoreEditStep(
        description="project_var_edit",
        edit=lambda root: replace_project_text(
            root, "sqlbuild_project.toml", 'region = "north"', 'region = "east"'
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="constant_file_edit",
        edit=lambda root: replace_project_text(
            root, "sql/constants/base_rate.sql", "value 1)", "value 5)"
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="enum_edit",
        edit=lambda root: replace_project_text(
            root, "sql/enums/order_status.sql", 'PLACED "placed"', 'PLACED "open"'
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="macro_body_edit",
        edit=lambda root: replace_project_text(
            root, "sql/macros/python/common.py", 'return f"{column} * 100"', 'return f"{column} * 1000"'
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="helper_module_edit",
        edit=lambda root: edit_label_helper(root, "other"),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="macro_file_added",
        edit=lambda root: (
            write_project_file(
                root,
                "sql/macros/python/badges.py",
                'def badge() -> str:\n    """Return a badge literal."""\n    return "\'gold\'"\n',
            ),
            edit_south_model(root, "  @py.label() AS label,\n", "  @py.badge() AS badge,\n"),
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="macro_file_removed",
        edit=lambda root: (
            edit_south_model(root, "  @py.badge() AS badge,\n", ""),
            (root / "sql/macros/python/badges.py").unlink(),
        ),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="outside_module_rewritten_in_place",
        edit=lambda root: write_store_flavor(extlib=root.parent / "extlib", value="salty"),
        expected_logged_calls=3,
    ),
    MacroCallStoreEditStep(
        description="model_comment_after_every_edit",
        edit=lambda root: edit_south_model(root, "-- reviewed\n", "-- reviewed twice\n"),
        expected_logged_calls=0,
    ),
)


_TAGGED_STAGING: dict[str, str] = {
    "sql/models/staging/_sqlbuild/_macros/python/tags.py": (
        'def tagged(ctx, expr: str) -> str:\n    """Tag."""\n    return expr\n'
    ),
    "sql/models/staging/stg_orders.sql": (
        'MODEL (\n  description "Staged orders",\n);\n\n'
        "SELECT order_id, customer_id, @py.tagged('amount') AS amount, status\n"
        'FROM __source("raw_orders")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        MacroCallStoreEditSequenceTestCase(
            description="full_render_each_step", project_reuse=False, steps=_EDIT_STEPS
        ),
        MacroCallStoreEditSequenceTestCase(
            description="with_project_reuse", project_reuse=True, steps=_EDIT_STEPS
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_edit_sequence_when_compiling_with_macro_call_store_then_each_step_matches_uncached(
    tmp_path: Path, test_case: MacroCallStoreEditSequenceTestCase
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    log_path: Path = tmp_path / "macro-calls.log"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)
    cold: MacroCallStoreRun = macro_call_store_compile(
        project_dir=project_dir,
        extlib=extlib,
        log_path=log_path,
        project_reuse=test_case.project_reuse,
    )
    assert (cold.incremental.returncode, cold.matches) == (0, True), cold.incremental.stderr

    for step in test_case.steps:
        _ = step.edit(project_dir)
        run: MacroCallStoreRun = macro_call_store_compile(
            project_dir=project_dir,
            extlib=extlib,
            log_path=log_path,
            project_reuse=test_case.project_reuse,
            args=step.args,
            extra_env=step.env,
        )

        assert run.incremental.returncode == 0, (step.description, run.incremental.stderr)
        assert run.matches is test_case.expected_matches_uncached, step.description
        assert run.logged_calls == step.expected_logged_calls, step.description


@pytest.mark.parametrize(
    "test_case",
    [
        BrokenMacroCallStoreKeyTestCase(
            description="helper_module_edit",
            edit=lambda root: edit_label_helper(root, "other"),
            expected_matches_uncached=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_broken_store_key_when_compiling_an_edit_then_the_oracle_reports_a_mismatch(
    tmp_path: Path,
    test_case: BrokenMacroCallStoreKeyTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)
    monkeypatch.syspath_prepend(str(extlib))
    monkeypatch.setenv("SQLBUILD_DISABLE_COMPILE_REUSE", "1")
    monkeypatch.setenv(MACRO_CALL_LOG_ENV_VAR, str(tmp_path / "macro-calls.log"))
    freeze_macro_call_store_environment(monkeypatch)
    cold: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    assert cold.returncode == 0, cold.stderr
    test_case.edit(project_dir)

    broken: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir,
        env={"PYTHONPATH": str(extlib)},
        args=("--no-cache",),
    )

    assert (broken.returncode, reference.returncode) == (0, 0)
    assert (broken.compiled == reference.compiled) is test_case.expected_matches_uncached


@pytest.mark.parametrize(
    "test_case",
    [
        EngineMacroCallGateTestCase(
            description="default",
            expected_logged_calls=(1, 0),
            expected_store_files=("target/cache/compiler-native-v1/macro-calls.bin",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_project_when_compiling_repeatedly_then_macro_calls_are_batched_and_stored(
    tmp_path: Path, test_case: EngineMacroCallGateTestCase
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)

    runs: FullMacroCallRuns = full_macro_call_runs(
        project_dir=project_dir,
        extlib=extlib,
        log_path=tmp_path / "macro-calls.log",
        runs=2,
    )

    assert runs.returncodes == (0, 0)
    assert runs.logged_calls == test_case.expected_logged_calls
    assert runs.store_files == test_case.expected_store_files


@pytest.mark.parametrize(
    "test_case",
    [
        MacroReferenceCallStoreTestCase(
            description="native_memo_then_store",
            expected_logged_calls=(1, 0),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_returning_rejected_reference_call_when_replaying_then_reports_match_first(
    tmp_path: Path, test_case: MacroReferenceCallStoreTestCase
) -> None:
    oracle: MacroReferenceCallRuns = macro_reference_call_runs(
        project_dir=prepare_macro_reference_call_project(tmp_path=tmp_path / "oracle"),
        log_path=tmp_path / "oracle.log",
        runs=1,
    )

    runs: MacroReferenceCallRuns = macro_reference_call_runs(
        project_dir=prepare_macro_reference_call_project(tmp_path=tmp_path / "replayed"),
        log_path=tmp_path / "replayed.log",
        runs=_MACRO_REFERENCE_CALL_RUNS,
    )

    assert [diagnostic[:4] for diagnostic in oracle.diagnostics[0]] == [
        ("P012", _MACRO_REFERENCE_CALL_MESSAGE, name, _MACRO_REFERENCE_CALL_PATH)
        for name in ("customer_orders", "customer_returns")
    ]
    assert runs.returncodes == (1,) * _MACRO_REFERENCE_CALL_RUNS
    assert runs.logged_calls == test_case.expected_logged_calls
    assert runs.diagnostics == oracle.diagnostics * _MACRO_REFERENCE_CALL_RUNS


@pytest.mark.parametrize(
    "test_case",
    [
        StaleMacroModuleStoreTestCase(
            description="edited_between_two_compiles_of_one_process",
            arrange=recompile_in_process_after_edit,
            compile_afresh=compile_as_new_process_here,
            expected_flavor="'salty'",
        ),
        StaleMacroModuleStoreTestCase(
            description="edited_between_store_attach_and_save",
            arrange=edit_flavor_while_saving,
            compile_afresh=compile_as_new_process_here,
            expected_flavor="'salty'",
        ),
        StaleMacroModuleStoreTestCase(
            description="moved_away_while_loaded",
            arrange=move_flavor_while_saving,
            compile_afresh=compile_as_new_process_here,
            expected_flavor="'salty'",
        ),
        StaleMacroModuleStoreTestCase(
            description="served_from_a_rebuilt_zip_archive",
            arrange=rezip_flavor_between_processes,
            compile_afresh=compile_in_new_process,
            expected_flavor="'salty'",
        ),
        StaleMacroModuleStoreTestCase(
            description="replaced_keeping_size_and_modification_time",
            arrange=backdate_flavor_between_processes,
            compile_afresh=compile_in_new_process,
            expected_flavor="'salty'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_module_changed_after_use_when_compiling_in_a_new_process_then_no_stale_replay(
    tmp_path: Path,
    test_case: StaleMacroModuleStoreTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)
    monkeypatch.syspath_prepend(str(extlib))
    monkeypatch.setenv("SQLBUILD_DISABLE_COMPILE_REUSE", "1")
    monkeypatch.setenv(MACRO_CALL_LOG_ENV_VAR, str(tmp_path / "macro-calls.log"))
    monkeypatch.setattr(sys, "dont_write_bytecode", True)
    forget_store_flavor_module()
    arrangement: StaleStoreArrangement = StaleStoreArrangement(
        project_dir=project_dir, extlib=extlib, monkeypatch=monkeypatch, capsys=capsys
    )
    test_case.arrange(arrangement)

    fresh: CompileReuseRun = test_case.compile_afresh(arrangement)
    forget_store_flavor_module()
    reference: CompileReuseRun = uncached_reference_compile(project_dir=project_dir, extlib=extlib)

    assert (fresh.returncode, reference.returncode) == (0, 0), fresh.stderr
    assert fresh.compiled == reference.compiled
    assert test_case.expected_flavor in b"".join(fresh.compiled.values()).decode()


@pytest.mark.parametrize(
    "test_case",
    [
        SecondCompileStoreTestCase(
            description="second_compile_of_a_process", expected_second_matches_first=True
        )
    ],
    ids=lambda case: case.description,
)
def test_given_second_compile_in_one_process_when_compiling_then_the_store_is_not_touched(
    tmp_path: Path,
    test_case: SecondCompileStoreTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    log_path: Path = tmp_path / "macro-calls.log"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)
    monkeypatch.syspath_prepend(str(extlib))
    monkeypatch.setenv("SQLBUILD_DISABLE_COMPILE_REUSE", "1")
    monkeypatch.setenv(MACRO_CALL_LOG_ENV_VAR, str(log_path))
    forget_store_flavor_module()
    pretend_fresh_process(monkeypatch)
    first: int = logged_in_process_compile(
        project_dir=project_dir, log_path=log_path, capsys=capsys
    )
    stored: dict[str, bytes] = store_files(project_dir)

    second: int = logged_in_process_compile(
        project_dir=project_dir, log_path=log_path, capsys=capsys
    )
    forget_store_flavor_module()

    assert first > 0
    assert stored != {}
    assert (second == first) is test_case.expected_second_matches_first
    assert store_files(project_dir) == stored


@pytest.mark.parametrize(
    "test_case",
    [
        EngineMacroCallGateTestCase(
            description="native_without_proc",
            expected_logged_calls=(1, 0),
            expected_store_files=("target/cache/compiler-native-v1/macro-calls.bin",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_no_proc_filesystem_when_compiling_repeatedly_then_the_store_still_serves_calls(
    tmp_path: Path, test_case: EngineMacroCallGateTestCase
) -> None:
    project_dir: Path = tmp_path / "project"
    extlib: Path = tmp_path / "extlib"
    prepare_macro_call_store_project(project_dir=project_dir, extlib=extlib)
    block_proc_reads(extlib)

    runs: FullMacroCallRuns = full_macro_call_runs(
        project_dir=project_dir,
        extlib=extlib,
        log_path=tmp_path / "macro-calls.log",
        runs=2,
    )

    assert runs.returncodes == (0, 0)
    assert runs.logged_calls == test_case.expected_logged_calls
    assert runs.store_files == test_case.expected_store_files


@pytest.mark.parametrize(
    "test_case",
    [
        UnkeyableMacroCallTestCase(
            description="a lone surrogate var is rejected before any macro runs",
            project_files=_TAGGED_STAGING,
            compile_args=("--vars", '{"regions": ["\\udcff"]}'),
            expected_returncode=1,
            expected_report_fragment=(
                "Variable 'regions' holds a lone surrogate in its value[0] at UTF-8 byte 0, "
                "which is not valid Unicode text"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unencodable_var_when_compiling_then_it_is_rejected(
    test_case: UnkeyableMacroCallTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "orders"
    for relative_path, contents in {**FAILURE_BASE_FILES, **test_case.project_files}.items():
        write_project_file(project_dir, relative_path, contents)
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir, args=test_case.compile_args)

    assert (
        run.returncode,
        test_case.expected_report_fragment in run.report + run.stderr,
    ) == (test_case.expected_returncode, True), run.report + run.stderr
