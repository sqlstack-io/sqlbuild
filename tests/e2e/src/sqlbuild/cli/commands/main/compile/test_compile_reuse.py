"""Whole-project compile reuse must replay exactly what a full compile would produce."""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    CompiledTreeStampTestCase,
    CompileReuseBypassTestCase,
    CompileReuseConcurrentWriterTestCase,
    CompileReuseCorruptEntryTestCase,
    CompileReuseDeprecatedSettingTestCase,
    CompileReuseEventTestCase,
    CompileReuseHitTestCase,
    CompileReuseInvalidationTestCase,
    CompileReuseLargeFileTestCase,
    CompileReuseProviderSettingsTestCase,
    CompileReuseRacyRefreshTestCase,
    CompileReuseRedirectTestCase,
    CompileReuseReplayTestCase,
    CompileReuseStoreFailureTestCase,
    CompileReuseTimingsTestCase,
    RetiredCompilerCacheTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    COMPILE_REUSE_HIT_LINE,
    COMPILE_REUSE_REGION_ENV_VAR,
    ORDERS_API_TIMEOUT_ENV_VAR,
    ORDERS_API_TOKEN_ENV_VAR,
    CompileReuseRun,
    add_stale_compiled_file,
    add_stale_compiled_file_beside_models,
    add_stale_file_in_compiled_root,
    compile_in_process,
    compile_in_process_output,
    compile_in_process_reused,
    compile_notes,
    compile_reuse_entry_paths,
    compile_reuse_hit_count,
    compiled_artifacts,
    compiled_tree_stamp_path,
    delete_compiled_model,
    edit_compiled_model,
    edit_project_model_during_compile,
    empty_file,
    enable_compile_reuse,
    fail_reuse_store,
    flip_header_byte,
    flip_stored_stdout_bytes,
    flip_trailing_bytes,
    garbage_file,
    garbage_stored_output,
    in_process_compile_reads,
    link_compiled_root,
    previous_format_slot,
    progress_only,
    record_digested_files,
    recorded_events,
    remove_stored_output,
    replace_project_text,
    rewrite_compiled_model_unchanged,
    rewrite_macro_without_change,
    run_reuse_compile,
    run_reuse_compile_into_file,
    run_reuse_text_compile,
    settle_racy_window,
    stored_output_path,
    touch_back,
    touch_model_without_change,
    truncate_file,
    truncate_stored_output,
    write_before_reuse_store,
    write_custom_source_provider,
    write_large_file,
    write_orders_api_provider,
    write_orders_api_timeout_file,
    write_project_file,
    write_recording_sink,
    write_retired_compiler_cache_files,
)

# Longer than the racy window, so folders the first compile created are trusted when stamped.
_SETTLED_TREE_SECONDS: float = 2.2
_CONTRACT_ERROR_MODEL: str = (
    "MODEL (description 'Order contract.', columns (missing_col (type INTEGER)));\n\n"
    'SELECT order_id FROM __ref("stg_orders")\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseInvalidationTestCase(
            description="model_sql_edit",
            edit=lambda root: replace_project_text(
                root, "models/staging/stg_orders.sql", "  quantity,", "  quantity + 0 AS quantity,"
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="model_reference_edit",
            edit=lambda root: replace_project_text(
                root,
                "models/marts/referenced_orders.sql",
                '__ref("stg_orders")',
                '__ref("regional_orders")',
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="macro_built_reference_edit",
            edit=lambda root: replace_project_text(
                root,
                "models/marts/_sqlbuild/_macros/python/order_source.py",
                '"stg_orders"',
                '"regional_orders"',
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="variable_reference_change",
            edited_args=("--vars", json.dumps({"orders_model": "regional_orders"})),
        ),
        CompileReuseInvalidationTestCase(
            description="model_added",
            edit=lambda root: write_project_file(
                root,
                "models/marts/order_quantities.sql",
                "MODEL (description 'Order quantities.');\n\n"
                'SELECT order_id, quantity FROM __ref("stg_orders")\n',
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="model_deleted",
            edit=lambda root: (root / "models/marts/regional_orders.sql").unlink(),
        ),
        CompileReuseInvalidationTestCase(
            description="source_yaml_edit",
            edit=lambda root: replace_project_text(
                root,
                "sources/raw.yml",
                "      - name: quantity\n        type: INTEGER",
                "      - name: quantity\n        type: BIGINT",
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="seed_csv_edit",
            edit=lambda root: replace_project_text(
                root, "seeds/waffle_types.csv", "Classic Belgian", "Classic Brussels"
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="macro_file_edit",
            edit=lambda root: replace_project_text(
                root,
                "macros/python/currency.py",
                "{price_cents} * {quantity}",
                "{quantity} * {price_cents}",
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="module_imported_by_macro_edit",
            edit=lambda root: replace_project_text(
                root, "macros/python/_rounding.py", "_SCALE: int = 2", "_SCALE: int = 3"
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="scoped_macro_edit",
            edit=lambda root: replace_project_text(
                root,
                "models/marts/_sqlbuild/_macros/python/currency.py",
                'return f"ROUND(({column}) / 100.0, 2)"',
                'return f"ROUND(({column}) / 100.0, 3)"',
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="enum_declaration_edit",
            edit=lambda root: replace_project_text(
                root, "models/marts/_sqlbuild/_enums/order_channel.sql", 'WEB "web"', 'WEB "online"'
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="project_toml_edit",
            edit=lambda root: replace_project_text(
                root,
                "sqlbuild_project.toml",
                'quantity_multiplier = "2"',
                'quantity_multiplier = "5"',
            ),
        ),
        CompileReuseInvalidationTestCase(
            description="hidden_root_directory_file_added",
            edit=lambda root: write_project_file(root, ".orders_rules/limits.txt", "10\n"),
        ),
        CompileReuseInvalidationTestCase(
            description="local_toml_added",
            edit=lambda root: write_project_file(root, "sqlbuild_local.toml", 'target = "prod"\n'),
        ),
        CompileReuseInvalidationTestCase(
            description="cli_vars_change",
            edited_args=("--vars", json.dumps({"quantity_multiplier": "7"})),
        ),
        CompileReuseInvalidationTestCase(
            description="template_environment_variable_change",
            edited_env={COMPILE_REUSE_REGION_ENV_VAR: "south"},
        ),
        CompileReuseInvalidationTestCase(
            description="sqlbuild_prefixed_environment_variable_added",
            edited_env={"SQB_ORDERS_UNUSED_SETTING": "1"},
        ),
        CompileReuseInvalidationTestCase(
            description="custom_rule_file_edit",
            edit=lambda root: replace_project_text(
                root,
                "rules/naming.py",
                "<= limits.MAX_NAME_LENGTH:",
                "<= limits.MAX_NAME_LENGTH - 30:",
            ),
            expected_returncode=1,
        ),
        CompileReuseInvalidationTestCase(
            description="module_imported_by_custom_rule_edit",
            edit=lambda root: replace_project_text(
                root, "rules/limits.py", "MAX_NAME_LENGTH: int = 40", "MAX_NAME_LENGTH: int = 10"
            ),
            expected_returncode=1,
        ),
        CompileReuseInvalidationTestCase(
            description="contract_error_replayed_as_failure",
            edit=lambda root: write_project_file(
                root, "models/marts/order_contract.sql", _CONTRACT_ERROR_MODEL
            ),
            expected_returncode=1,
        ),
        CompileReuseInvalidationTestCase(
            description="stopped_compile_is_never_replayed",
            edit=lambda root: replace_project_text(
                root, "models/staging/stg_orders.sql", "materialized view,", "materialized view,,(("
            ),
            expected_returncode=1,
            expected_rewarm_hit=False,
        ),
        CompileReuseInvalidationTestCase(
            description="compiled_artifact_deleted", edit=delete_compiled_model
        ),
        CompileReuseInvalidationTestCase(
            description="compiled_artifact_edited", edit=edit_compiled_model
        ),
        CompileReuseInvalidationTestCase(
            description="stale_compiled_artifact_added", edit=add_stale_compiled_file
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reused_compile_when_input_changes_then_it_misses_and_matches_uncached_compile(
    compile_reuse_project: Path, test_case: CompileReuseInvalidationTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    warm: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    assert cold.returncode == 0, cold.stderr
    assert (cold.reused, cold.timings["project_reuse_misses"]) == (False, 1)
    assert (warm.reused, warm.timings["project_reuse_hits"]) == (True, 1)
    assert (warm.returncode, warm.report, warm.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )

    test_case.edit(project_dir)
    edited: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env=test_case.edited_env, args=test_case.edited_args
    )
    reference: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir,
        env=test_case.edited_env,
        args=(*test_case.edited_args, "--no-cache"),
    )
    rewarmed: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env=test_case.edited_env, args=test_case.edited_args
    )

    assert (edited.reused, reference.reused) == (False, False)
    assert edited.returncode == test_case.expected_returncode, edited.stderr
    assert (edited.returncode, edited.report, edited.compiled) == (
        reference.returncode,
        reference.report,
        reference.compiled,
    )
    assert rewarmed.reused is test_case.expected_rewarm_hit
    assert (rewarmed.returncode, rewarmed.report, rewarmed.compiled) == (
        reference.returncode,
        reference.report,
        reference.compiled,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseHitTestCase(
            description="unrelated_environment_variable",
            edit=lambda _root: None,
            edited_env={"ORDERS_UNRELATED_SETTING": "1"},
        ),
        CompileReuseHitTestCase(
            description="editor_settings_added",
            edit=lambda root: write_project_file(root, ".vscode/settings.json", "{}\n"),
        ),
        CompileReuseHitTestCase(
            description="database_file_content_change",
            edit=lambda root: (root / "waffle_shop_control.duckdb").write_bytes(b"changed pages"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reused_compile_when_no_input_changes_then_rerun_reuses_it(
    compile_reuse_project: Path, test_case: CompileReuseHitTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    test_case.edit(project_dir)
    rerun: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=test_case.edited_env)

    assert rerun.reused is test_case.expected_reused, rerun.stderr
    assert (rerun.returncode, rerun.report, rerun.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseHitTestCase(
            description="touch_without_change", edit=touch_model_without_change
        ),
        CompileReuseHitTestCase(
            description="rewrite_without_change", edit=rewrite_macro_without_change
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_only_change_when_it_repeats_then_the_known_content_is_reused(
    compile_reuse_project: Path, test_case: CompileReuseHitTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    test_case.edit(project_dir)
    first: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    test_case.edit(project_dir)
    second: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    assert second.reused is test_case.expected_reused, second.stderr
    assert (first.returncode, first.report, first.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )
    assert (second.returncode, second.report, second.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseBypassTestCase(description="no_cache_flag", compile_args=("--no-cache",)),
        CompileReuseBypassTestCase(
            description="reuse_disabled_environment_variable",
            env={"SQLBUILD_DISABLE_COMPILE_REUSE": "1"},
        ),
        CompileReuseBypassTestCase(
            description="compile_cache_disabled_environment_variable",
            env={"SQLBUILD_DISABLE_COMPILE_CACHE": "1"},
        ),
        CompileReuseBypassTestCase(
            description="target_compile_cache_false",
            edit=lambda root: replace_project_text(
                root,
                "sqlbuild_project.toml",
                'schema = "dev"',
                'schema = "dev"\ncompile_cache = false',
            ),
        ),
        CompileReuseBypassTestCase(description="manifest_flag", compile_args=("--manifest",)),
        CompileReuseBypassTestCase(description="debug_flag", global_args=("--debug",)),
        CompileReuseBypassTestCase(
            description="provider_with_custom_settings_sources",
            edit=write_custom_source_provider,
        ),
        CompileReuseBypassTestCase(
            description="model_reads_run_id",
            edit=lambda root: write_project_file(
                root,
                "models/marts/stamped_orders.sql",
                "MODEL (description 'Orders stamped by run ${CTX:run.id}.');\n\n"
                'SELECT order_id FROM __ref("stg_orders")\n',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reuse_bypass_when_compiling_twice_then_both_compiles_run_in_full(
    compile_reuse_project: Path, test_case: CompileReuseBypassTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    test_case.edit(project_dir)

    first: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir,
        env=test_case.env,
        args=test_case.compile_args,
        global_args=test_case.global_args,
    )
    second: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir,
        env=test_case.env,
        args=test_case.compile_args,
        global_args=test_case.global_args,
    )

    assert (first.returncode, second.returncode) == (0, 0), first.stderr + second.stderr
    assert (first.reused, second.reused, second.timings["project_reuse_hits"]) == (False, False, 0)
    assert len(compile_reuse_entry_paths(project_dir=project_dir)) == (
        test_case.expected_stored_entries
    )
    assert second.report == first.report


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseCorruptEntryTestCase(description="truncated", corrupt=truncate_file),
        CompileReuseCorruptEntryTestCase(description="header_tail", corrupt=flip_trailing_bytes),
        CompileReuseCorruptEntryTestCase(description="header_bytes", corrupt=flip_header_byte),
        CompileReuseCorruptEntryTestCase(description="empty", corrupt=empty_file),
        CompileReuseCorruptEntryTestCase(description="garbage", corrupt=garbage_file),
        CompileReuseCorruptEntryTestCase(
            description="previous_store_format", corrupt=previous_format_slot
        ),
        CompileReuseCorruptEntryTestCase(
            description="output_truncated", corrupt=truncate_stored_output
        ),
        CompileReuseCorruptEntryTestCase(
            description="output_report_bytes", corrupt=flip_stored_stdout_bytes
        ),
        CompileReuseCorruptEntryTestCase(
            description="output_missing", corrupt=remove_stored_output
        ),
        CompileReuseCorruptEntryTestCase(
            description="output_garbage", corrupt=garbage_stored_output
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_corrupt_stored_compile_when_compiling_then_it_falls_back_and_restores_it(
    compile_reuse_project: Path, test_case: CompileReuseCorruptEntryTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    entries: tuple[Path, ...] = compile_reuse_entry_paths(project_dir=project_dir)
    assert len(entries) == 1
    test_case.corrupt(entries[0])

    fallback: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    restored: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    assert fallback.reused is test_case.expected_fallback_reused
    assert restored.reused is test_case.expected_restored_reused
    assert (fallback.returncode, fallback.report, fallback.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )
    assert (restored.returncode, restored.report, restored.compiled) == (
        cold.returncode,
        cold.report,
        cold.compiled,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseRacyRefreshTestCase(
            description="project_written_just_before_the_first_compile",
            expected_reused=(False, True, True),
            expected_output_rewritten=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_racy_stored_inputs_when_reusing_then_only_the_slot_file_is_refreshed(
    compile_reuse_project: Path, test_case: CompileReuseRacyRefreshTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    (entry,) = compile_reuse_entry_paths(project_dir=project_dir)
    output: Path = stored_output_path(entry)
    stored_output: tuple[bytes, int] = (output.read_bytes(), output.stat().st_mtime_ns)
    slot_mtime_ns: int = entry.stat().st_mtime_ns

    refreshed: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    rerun: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    assert (cold.reused, refreshed.reused, rerun.reused) == test_case.expected_reused
    assert entry.stat().st_mtime_ns != slot_mtime_ns, "the hit refreshed the racy stamps"
    assert (
        (output.read_bytes(), output.stat().st_mtime_ns) != stored_output
    ) is test_case.expected_output_rewritten
    for run in (refreshed, rerun):
        assert (run.returncode, run.report, run.compiled) == (
            cold.returncode,
            cold.report,
            cold.compiled,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseReplayTestCase(
            description="failed_text_compile_without_sql_analysis",
            compile_args=("--no-sql-analysis",),
            expected_returncode=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failed_text_compile_when_rerun_then_report_notes_and_exit_code_are_replayed(
    compile_reuse_project: Path, test_case: CompileReuseReplayTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    replace_project_text(
        project_dir,
        "sqlbuild_project.toml",
        'select = ["XSQBR"]',
        'select = ["XSQBR", "SQBRCONTRACT105"]',
    )
    replace_project_text(
        project_dir, "rules/limits.py", "MAX_NAME_LENGTH: int = 40", "MAX_NAME_LENGTH: int = 10"
    )

    cold: subprocess.CompletedProcess[str] = run_reuse_text_compile(
        project_dir=project_dir, args=test_case.compile_args
    )
    warm: subprocess.CompletedProcess[str] = run_reuse_text_compile(
        project_dir=project_dir, args=test_case.compile_args
    )

    assert (cold.returncode, warm.returncode) == (
        test_case.expected_returncode,
        test_case.expected_returncode,
    )
    assert COMPILE_REUSE_HIT_LINE not in cold.stderr
    assert COMPILE_REUSE_HIT_LINE in warm.stderr
    assert warm.stdout.strip()
    assert cold.stdout.endswith(warm.stdout)
    assert progress_only(text=cold.stdout.removesuffix(warm.stdout))
    assert compile_notes(stderr=cold.stderr)[0].startswith(test_case.expected_note_prefix)
    assert compile_notes(stderr=warm.stderr) == compile_notes(stderr=cold.stderr)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseDeprecatedSettingTestCase(
            description="column_contract_mode_alias",
            settings_line='column_contract_mode = "explicit"',
            expected_warning=(
                "Warning: settings.column_contract_mode in sqlbuild_project.toml is deprecated; "
                "replace it with settings.check_declared_columns = false"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_deprecated_setting_alias_when_compile_is_reused_then_the_warning_still_prints_once(
    compile_reuse_project: Path,
    test_case: CompileReuseDeprecatedSettingTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = compile_reuse_project
    replace_project_text(
        project_dir, "sqlbuild_project.toml", "[settings]", f"[settings]\n{test_case.settings_line}"
    )

    cold: subprocess.CompletedProcess[str] = run_reuse_text_compile(
        project_dir=project_dir, args=()
    )
    warm: subprocess.CompletedProcess[str] = run_reuse_text_compile(
        project_dir=project_dir, args=()
    )
    enable_compile_reuse(monkeypatch)
    _ = compile_in_process_output(project_dir=project_dir, capsys=capsys)
    in_process: tuple[int, str, str] = compile_in_process_output(
        project_dir=project_dir, capsys=capsys
    )

    assert (cold.returncode, warm.returncode, in_process[0]) == (0, 0, 0), cold.stderr
    assert (COMPILE_REUSE_HIT_LINE in warm.stderr, COMPILE_REUSE_HIT_LINE in in_process[2]) == (
        True,
        True,
    )
    assert (
        cold.stderr.count(test_case.expected_warning),
        warm.stderr.count(test_case.expected_warning),
        in_process[2].count(test_case.expected_warning),
    ) == (1, 1, 1)
    assert test_case.expected_warning not in cold.stdout + warm.stdout + in_process[1]


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseRedirectTestCase(
            description="json_report_redirected_into_project_root",
            report_name="compile-report.json",
            expected_hit_counts=(0, 1),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_stdout_redirected_into_project_when_rerun_then_it_reuses_the_compile(
    compile_reuse_project: Path, test_case: CompileReuseRedirectTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    report_path: Path = project_dir / test_case.report_name

    first: subprocess.CompletedProcess[str] = run_reuse_compile_into_file(
        project_dir=project_dir, report_path=report_path
    )
    first_hits: int = compile_reuse_hit_count(report_path=report_path)
    second: subprocess.CompletedProcess[str] = run_reuse_compile_into_file(
        project_dir=project_dir, report_path=report_path
    )
    second_hits: int = compile_reuse_hit_count(report_path=report_path)

    assert (first.returncode, second.returncode) == (0, 0), first.stderr + second.stderr
    assert (first_hits, second_hits) == test_case.expected_hit_counts


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseEventTestCase(
            description="invocation_events_on_reuse_hit",
            sink_name="orders_events",
            expected_event_types=("invocation_started", "invocation_completed"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_lifecycle_sink_when_compile_is_reused_then_invocation_events_are_exported(
    compile_reuse_project: Path, tmp_path: Path, test_case: CompileReuseEventTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    write_recording_sink(project_dir=project_dir, sink_name=test_case.sink_name)
    cold_events: Path = tmp_path / "cold-events.jsonl"
    warm_events: Path = tmp_path / "warm-events.jsonl"

    cold: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env={"ORDERS_EVENT_PATH": str(cold_events)}
    )
    warm: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env={"ORDERS_EVENT_PATH": str(warm_events)}
    )
    events: list[dict[str, object]] = recorded_events(path=warm_events)

    assert cold.returncode == 0, cold.stderr
    assert warm.reused, warm.stderr
    assert tuple(event["event_type"] for event in events) == test_case.expected_event_types
    assert [cast(dict[str, object], event["payload"])["command"] for event in events] == [
        "compile",
        "compile",
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseTimingsTestCase(
            description="reuse_hit_timings",
            expected_timing_names=frozenset(
                {
                    "project_reuse_hits",
                    "project_reuse_misses",
                    "project_reuse_bypasses",
                    "project_reuse_check_ms",
                    "total_ms",
                }
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reused_compile_when_replayed_then_compile_timings_describe_the_reuse(
    compile_reuse_project: Path, test_case: CompileReuseTimingsTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    _ = run_reuse_compile(project_dir=project_dir)

    replay: CompileReuseRun = run_reuse_compile(project_dir=project_dir)

    assert replay.reused
    assert frozenset(replay.timings) == test_case.expected_timing_names
    assert replay.timings["project_reuse_hits"] == 1


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseProviderSettingsTestCase(
            description="required_setting_unset",
            edited_env={},
            expected_returncode=1,
            expected_rewarm_hit=False,
        ),
        CompileReuseProviderSettingsTestCase(
            description="setting_value_changed",
            edited_env={ORDERS_API_TOKEN_ENV_VAR: "beta"},
        ),
        CompileReuseProviderSettingsTestCase(
            description="setting_value_invalid",
            edited_env={ORDERS_API_TOKEN_ENV_VAR: "alpha", ORDERS_API_TIMEOUT_ENV_VAR: "soon"},
            expected_returncode=1,
            expected_rewarm_hit=False,
        ),
        CompileReuseProviderSettingsTestCase(
            description="env_file_changed",
            edited_env={ORDERS_API_TOKEN_ENV_VAR: "alpha"},
            edit=write_orders_api_timeout_file,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_provider_settings_when_their_inputs_change_then_reuse_misses_like_full_compile(
    compile_reuse_project: Path, test_case: CompileReuseProviderSettingsTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    write_orders_api_provider(project_dir=project_dir)
    cold_env: dict[str, str] = {ORDERS_API_TOKEN_ENV_VAR: "alpha"}
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=cold_env)
    warm: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=cold_env)
    assert (cold.returncode, warm.reused) == (0, True), cold.stderr

    test_case.edit(project_dir)
    edited: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=test_case.edited_env)
    reference: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env=test_case.edited_env, args=("--no-cache",)
    )
    rewarmed: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=test_case.edited_env)

    assert edited.reused is False
    assert edited.returncode == test_case.expected_returncode, edited.stderr
    assert (edited.returncode, edited.report) == (reference.returncode, reference.report)
    assert rewarmed.reused is test_case.expected_rewarm_hit
    assert (rewarmed.returncode, rewarmed.report) == (reference.returncode, reference.report)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseConcurrentWriterTestCase(
            description="other_compile_edits_artifact",
            other_write=edit_compiled_model,
            expected_stored_entries=0,
            expected_rerun_reused=False,
        ),
        CompileReuseConcurrentWriterTestCase(
            description="other_compile_adds_artifact",
            other_write=add_stale_compiled_file,
            expected_stored_entries=0,
            expected_rerun_reused=False,
        ),
        CompileReuseConcurrentWriterTestCase(
            description="project_model_edited_during_compile",
            other_write=edit_project_model_during_compile,
            expected_stored_entries=0,
            expected_rerun_reused=False,
        ),
        CompileReuseConcurrentWriterTestCase(
            description="other_compile_rewrites_same_bytes",
            other_write=rewrite_compiled_model_unchanged,
            expected_stored_entries=1,
            expected_rerun_reused=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_other_writer_before_store_when_compiling_then_only_own_artifacts_are_stored(
    compile_reuse_project: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    test_case: CompileReuseConcurrentWriterTestCase,
) -> None:
    project_dir: Path = compile_reuse_project
    enable_compile_reuse(monkeypatch)
    write_before_reuse_store(
        monkeypatch=monkeypatch, project_dir=project_dir, write=test_case.other_write
    )

    code: int = compile_in_process(project_dir=project_dir)
    stored: int = len(compile_reuse_entry_paths(project_dir=project_dir))
    rerun: tuple[int, bool] = compile_in_process_reused(project_dir=project_dir, capsys=capsys)
    rerun_compiled: dict[str, bytes] = compiled_artifacts(project_dir=project_dir)
    reference: CompileReuseRun = run_reuse_compile(project_dir=project_dir, args=("--no-cache",))

    assert (code, stored) == (0, test_case.expected_stored_entries)
    assert rerun == (reference.returncode, test_case.expected_rerun_reused)
    assert rerun_compiled == reference.compiled


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseLargeFileTestCase(
            description="untouched_then_restamped_data_file",
            size_bytes=8 * 1024 * 1024,
            relative_path="data/orders_export.bin",
            expected_reads=(0, 0, 1, 1, 0),
            expected_reused=(False, True, False, True, True),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_large_data_file_when_storing_and_reusing_then_it_is_read_only_after_restamp(
    compile_reuse_project: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    test_case: CompileReuseLargeFileTestCase,
) -> None:
    project_dir: Path = compile_reuse_project
    data_file: Path = write_large_file(
        project_dir=project_dir,
        relative_path=test_case.relative_path,
        size_bytes=test_case.size_bytes,
    )
    enable_compile_reuse(monkeypatch)
    settle_racy_window(monkeypatch=monkeypatch)
    digested: list[int] = record_digested_files(monkeypatch=monkeypatch)

    stored: tuple[int, int, bool] = in_process_compile_reads(
        project_dir=project_dir, digested=digested, capsys=capsys
    )
    reused: tuple[int, int, bool] = in_process_compile_reads(
        project_dir=project_dir, digested=digested, capsys=capsys
    )
    touch_back(data_file, seconds=3600)
    restamped: tuple[int, int, bool] = in_process_compile_reads(
        project_dir=project_dir, digested=digested, capsys=capsys
    )
    touch_back(data_file, seconds=7200)
    verified: tuple[int, int, bool] = in_process_compile_reads(
        project_dir=project_dir, digested=digested, capsys=capsys
    )
    refreshed: tuple[int, int, bool] = in_process_compile_reads(
        project_dir=project_dir, digested=digested, capsys=capsys
    )
    runs: tuple[tuple[int, int, bool], ...] = (stored, reused, restamped, verified, refreshed)

    assert tuple(run[0] for run in runs) == (0, 0, 0, 0, 0)
    assert tuple(run[1] for run in runs) == test_case.expected_reads
    assert tuple(run[2] for run in runs) == test_case.expected_reused


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseStoreFailureTestCase(
            description="unexpected_store_error", error=RuntimeError("settings went away")
        )
    ],
    ids=lambda case: case.description,
)
def test_given_store_raises_when_compiling_then_report_and_exit_code_are_kept(
    compile_reuse_project: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    test_case: CompileReuseStoreFailureTestCase,
) -> None:
    project_dir: Path = compile_reuse_project
    enable_compile_reuse(monkeypatch)
    fail_reuse_store(monkeypatch=monkeypatch, error=test_case.error)

    code, stdout, stderr = compile_in_process_output(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = run_reuse_compile(project_dir=project_dir, args=("--no-cache",))

    assert code == test_case.expected_returncode == reference.returncode
    assert json.loads(stdout)["command"] == "compile"
    assert "Traceback" not in stderr
    assert len(compile_reuse_entry_paths(project_dir=project_dir)) == (
        test_case.expected_stored_entries
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CompileReuseStoreFailureTestCase(
            description="interrupted_store",
            error=KeyboardInterrupt(),
            expected_raised=KeyboardInterrupt,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_store_is_interrupted_when_compiling_then_the_interrupt_propagates(
    compile_reuse_project: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: CompileReuseStoreFailureTestCase,
) -> None:
    project_dir: Path = compile_reuse_project
    enable_compile_reuse(monkeypatch)
    fail_reuse_store(monkeypatch=monkeypatch, error=test_case.error)

    with pytest.raises(BaseException) as raised:
        _ = compile_in_process(project_dir=project_dir)

    assert raised.type is test_case.expected_raised


@pytest.mark.parametrize(
    "test_case",
    [
        RetiredCompilerCacheTestCase(
            description="cached_compile_removes_them", compile_args=(), expected_removed=True
        ),
        RetiredCompilerCacheTestCase(
            description="uncached_compile_leaves_the_cache_alone",
            compile_args=("--no-cache",),
            expected_removed=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cache_files_of_older_releases_when_compiling_then_cached_compile_deletes_them(
    compile_reuse_project: Path, test_case: RetiredCompilerCacheTestCase
) -> None:
    retired: tuple[Path, ...] = write_retired_compiler_cache_files(compile_reuse_project)

    run: CompileReuseRun = run_reuse_compile(
        project_dir=compile_reuse_project, args=test_case.compile_args
    )

    assert run.returncode == 0, run.stderr
    assert [path.exists() for path in retired] == [not test_case.expected_removed] * len(retired)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    [
        CompiledTreeStampTestCase(
            description="stale_file_beside_compiled_models",
            edit=add_stale_compiled_file_beside_models,
            expected_removed_path="target/compiled/models/marts/stale_mart.sql",
        ),
        CompiledTreeStampTestCase(
            description="stale_file_in_compiled_models_root",
            edit=add_stale_compiled_file,
            expected_removed_path="target/compiled/models/stale_model.sql",
        ),
        CompiledTreeStampTestCase(
            description="stale_file_in_linked_compiled_root",
            edit=add_stale_file_in_compiled_root,
            expected_removed_path="target/compiled/stray.sql",
            prepare=link_compiled_root,
            expected_stamped=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stamped_compiled_tree_when_a_stale_file_appears_then_the_next_compile_removes_it(
    compile_reuse_project: Path, test_case: CompiledTreeStampTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    uncached: dict[str, str] = {COMPILE_REUSE_DISABLE_ENV_VAR: "1"}
    test_case.prepare(project_dir)
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=uncached)
    time.sleep(_SETTLED_TREE_SECONDS)
    stamped: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=uncached)
    assert compiled_tree_stamp_path(project_dir=project_dir).is_file() is test_case.expected_stamped
    unchanged: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=uncached)
    test_case.edit(project_dir)

    cleaned: CompileReuseRun = run_reuse_compile(project_dir=project_dir, env=uncached)

    assert not (project_dir / test_case.expected_removed_path).exists()
    for run in (stamped, unchanged, cleaned):
        assert (run.returncode, run.report, run.compiled) == (
            cold.returncode,
            cold.report,
            cold.compiled,
        )
