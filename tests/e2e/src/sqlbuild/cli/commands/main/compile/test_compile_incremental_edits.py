"""Compiles after an edit must match an uncached compile of the same edited project."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    BrokenEditInvalidationTestCase,
    ExternalModuleEditTestCase,
    IncrementalEditSequenceTestCase,
    NativeAnalysisStoreTestCase,
    RandomEditChainTestCase,
    SharedCacheKeyTestCase,
    SqlTestScanStoreTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    CompileReuseRun,
    IncrementalEditComparison,
    RandomEditChain,
    adapter_switched,
    add_external_flavor_macro,
    build_between,
    change_adapter_lexical_rules,
    compare_incremental_compile,
    compile_edit_without_reuse,
    compile_in_process,
    compile_without_reuse_between,
    compiled_artifact_tampered,
    compiled_text,
    completed_order_udf_signature_change,
    corrupt_native_analysis_store,
    corrupt_sql_test_scan_store,
    disable_project_reuse,
    edit_installed_code,
    edit_sql_test_scan_input,
    edit_staging_type,
    edit_step,
    edit_unrelated_model,
    enable_compile_reuse,
    fact_audit_added,
    fact_comment,
    fact_error_fixed,
    fact_error_introduced,
    fact_quantity_type_declared,
    fact_quantity_type_removed,
    ignore_project_changes,
    in_process_reuse_run,
    keep_invalidation,
    move_project_file,
    native_analysis_counts,
    order_status_nullability_change,
    other_target_build_between,
    payment_expression_type_change,
    payments_comment,
    plan_between,
    random_edit_plan,
    replace_project_text,
    replace_stored_scans_misshapen,
    replace_stored_scans_undecodable,
    run_reuse_compile,
    sql_test_scan_counts,
    staging_column_removed,
    staging_column_renamed,
    staging_column_restored,
    staging_comment,
    staging_contract_change,
    staging_extra_flag,
    staging_header_change,
    staging_new_column,
    staging_quantity_restored,
    staging_quantity_text,
    staging_rename_reverted,
    staging_type_change,
    star_chain_added,
    star_chain_steps,
    star_chain_with_twin_added,
    stg_orders_test_edit,
    stg_orders_test_edited_again,
    twin_header_changed,
    upgrade_native_build,
    write_external_flavor,
    write_generated_edit_models,
    write_project_file,
)


@pytest.mark.parametrize(
    "test_case",
    [
        IncrementalEditSequenceTestCase(
            description="model_edits",
            steps=(
                edit_step("comment_without_shape_change", staging_comment),
                edit_step("type_change_propagates_downstream", staging_type_change),
                edit_step("new_output_column", staging_new_column),
                edit_step("header_config_change", staging_header_change),
                edit_step("column_contract_change", staging_contract_change),
                edit_step("attached_audit_added", fact_audit_added),
                edit_step("error_introduced", fact_error_introduced),
                edit_step("error_fixed", fact_error_fixed),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="model_set_changes",
            steps=(
                edit_step(
                    "model_added",
                    lambda root: write_project_file(
                        root,
                        "sql/models/marts/order_quantities.sql",
                        "MODEL (description 'Order quantities.');\n\n"
                        'SELECT order_id, quantity FROM __ref("stg_orders")\n',
                    ),
                ),
                edit_step("edit_after_add", staging_comment),
                edit_step(
                    "model_renamed",
                    lambda root: move_project_file(
                        root,
                        "sql/models/marts/order_quantities.sql",
                        "sql/models/marts/order_quantity_totals.sql",
                    ),
                ),
                edit_step(
                    "model_removed",
                    lambda root: (root / "sql/models/marts/order_quantity_totals.sql").unlink(),
                ),
                edit_step("edit_after_remove", fact_comment),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="declaration_edits",
            steps=(
                edit_step(
                    "macro_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/macros/python/currency.py",
                        "{price_cents} * {quantity}",
                        "{quantity} * {price_cents}",
                    ),
                ),
                edit_step("edit_after_macro", fact_comment),
                edit_step(
                    "module_imported_by_macro_edit",
                    lambda root: replace_project_text(
                        root, "sql/macros/python/_rounding.py", "_SCALE: int = 2", "_SCALE: int = 3"
                    ),
                ),
                edit_step(
                    "scoped_macro_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/models/marts/_sqlbuild/_macros/python/currency.py",
                        "/ 100.0, 2)",
                        "/ 100.0, 3)",
                    ),
                ),
                edit_step(
                    "path_defaults_edit",
                    lambda root: replace_project_text(
                        root,
                        "sqlbuild_project.toml",
                        '[path_defaults.staging]\nmaterialized = "view"',
                        '[path_defaults.staging]\nmaterialized = "table"',
                    ),
                ),
                edit_step(
                    "enum_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/models/marts/_sqlbuild/_enums/order_channel.sql",
                        'WEB "web"',
                        'WEB "online"',
                    ),
                ),
                edit_step(
                    "constant_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/models/marts/_sqlbuild/_constants/min_quantity.sql",
                        "value 1)",
                        "value 2)",
                    ),
                ),
                edit_step(
                    "new_scope_folder",
                    lambda root: write_project_file(
                        root,
                        "sql/models/staging/_sqlbuild/_macros/python/staging_totals.py",
                        "def staging_total_cents(price_cents: str, quantity: str) -> str:\n"
                        '    """Calculate a staging line total."""\n'
                        '    return f"({price_cents}) * ({quantity})"\n',
                    ),
                ),
                edit_step("edit_after_declarations", staging_comment),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="resource_edits",
            steps=(
                edit_step(
                    "seed_edit",
                    lambda root: replace_project_text(
                        root, "sql/seeds/waffle_types.csv", "Classic Belgian", "Classic Brussels"
                    ),
                ),
                edit_step(
                    "source_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/sources/raw.yml",
                        "      - name: quantity\n        type: BIGINT",
                        "      - name: quantity\n        type: INT8",
                    ),
                ),
                edit_step("edit_after_source", staging_type_change),
                edit_step("test_edit", stg_orders_test_edit),
                edit_step(
                    "audit_edit",
                    lambda root: replace_project_text(
                        root,
                        "sql/models/marts/_sqlbuild/_audits/generic/expression_is_true.sql",
                        "WHERE NOT (@expression)",
                        "WHERE NOT (@expression) AND 1 = 1",
                    ),
                ),
                edit_step("edit_after_resources", fact_comment),
                edit_step("compiled_artifact_tampered", compiled_artifact_tampered),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="analysis_input_edits",
            steps=(
                edit_step("star_chain_added", star_chain_added),
                edit_step("upstream_type_flips_downstream_shapes", staging_type_change),
                edit_step("contract_type_edit", staging_contract_change),
                edit_step("source_expression_edit", payment_expression_type_change),
                edit_step("udf_signature_edit", completed_order_udf_signature_change),
                edit_step("schema_entry_nullability_edit", order_status_nullability_change),
                edit_step("edit_after_analysis_inputs", fact_comment),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="upstream_shape_changes",
            steps=(
                edit_step("upstream_column_removed", staging_column_removed),
                edit_step("edit_while_downstream_broken", payments_comment),
                edit_step("upstream_column_restored", staging_column_restored),
                edit_step("upstream_column_renamed", staging_column_renamed),
                edit_step("upstream_rename_reverted", staging_rename_reverted),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="contract_changes",
            steps=(
                edit_step("downstream_type_declared", fact_quantity_type_declared),
                edit_step("upstream_type_breaks_contract", staging_quantity_text),
                edit_step("edit_after_contract_error", fact_comment),
                edit_step("upstream_type_restored", staging_quantity_restored),
                edit_step("downstream_type_removed", fact_quantity_type_removed),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="sql_test_scan_inputs",
            steps=(
                edit_step("test_edited", stg_orders_test_edit),
                edit_step("unrelated_model_edited", fact_comment),
                edit_step("adapter_switched", adapter_switched(before="duckdb", after="postgres")),
                edit_step("test_edited_on_other_adapter", stg_orders_test_edited_again),
                edit_step("adapter_restored", adapter_switched(before="postgres", after="duckdb")),
            ),
        ),
        IncrementalEditSequenceTestCase(
            description="star_chain_after_plan",
            steps=star_chain_steps(between=plan_between),
        ),
        IncrementalEditSequenceTestCase(
            description="star_chain_after_build",
            steps=star_chain_steps(between=build_between),
        ),
        IncrementalEditSequenceTestCase(
            description="star_chain_after_uncached_compile",
            steps=star_chain_steps(between=compile_without_reuse_between),
        ),
        IncrementalEditSequenceTestCase(
            description="star_chain_after_other_target_build",
            steps=star_chain_steps(between=other_target_build_between),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_edit_sequence_when_compiling_with_caches_then_each_step_matches_uncached_compile(
    compile_reuse_project: Path, test_case: IncrementalEditSequenceTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    warm: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    assert (cold.returncode, warm.returncode, warm.reused) == (0, 0, True), warm.stderr

    for step in test_case.steps:
        step.edit(project_dir)
        step.between(project_dir)
        comparison: IncrementalEditComparison = compare_incremental_compile(project_dir=project_dir)

        assert comparison.incremental.reused is step.expected_replayed, step.description
        assert comparison.matches is test_case.expected_matches_uncached, (
            step.description,
            comparison.mismatched_artifacts,
            comparison.incremental.stderr,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        RandomEditChainTestCase(description="seed_7", seed=7, model_count=24, step_count=12),
        RandomEditChainTestCase(description="seed_19", seed=19, model_count=24, step_count=12),
    ],
    ids=lambda case: case.description,
)
def test_given_random_edit_chain_when_compiling_with_caches_then_each_step_matches_uncached_compile(
    compile_reuse_project: Path, test_case: RandomEditChainTestCase
) -> None:
    project_dir: Path = compile_reuse_project
    write_generated_edit_models(
        project_dir=project_dir, model_count=test_case.model_count, seed=test_case.seed
    )
    cold: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    assert cold.returncode == 0, cold.stderr
    chain: RandomEditChain = RandomEditChain(project_dir=project_dir, seed=test_case.seed)

    for step, kind in enumerate(
        random_edit_plan(seed=test_case.seed, step_count=test_case.step_count)
    ):
        chain.apply(kind)
        chain.intervene()
        comparison: IncrementalEditComparison = compare_incremental_compile(project_dir=project_dir)

        assert comparison.matches is test_case.expected_matches_uncached, (
            step,
            kind,
            comparison.mismatched_artifacts,
            comparison.incremental.stderr,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        BrokenEditInvalidationTestCase(
            description="intact_invalidation_after_new_output_column",
            edit=staging_new_column,
            sabotage=keep_invalidation,
            expected_matches_uncached=True,
        ),
        BrokenEditInvalidationTestCase(
            description="compile_replayed_despite_model_edit",
            edit=staging_new_column,
            sabotage=ignore_project_changes,
            expected_matches_uncached=False,
        ),
        BrokenEditInvalidationTestCase(
            description="intact_invalidation_after_error_introduced",
            edit=fact_error_introduced,
            sabotage=keep_invalidation,
            expected_matches_uncached=True,
        ),
        BrokenEditInvalidationTestCase(
            description="intact_invalidation_after_test_edit",
            edit=stg_orders_test_edit,
            sabotage=keep_invalidation,
            expected_matches_uncached=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_broken_cache_invalidation_when_compiling_an_edit_then_the_oracle_reports_a_mismatch(
    compile_reuse_project: Path,
    test_case: BrokenEditInvalidationTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = compile_reuse_project
    enable_compile_reuse(monkeypatch)
    test_case.sabotage(monkeypatch)
    assert compile_in_process(project_dir=project_dir) == 0
    test_case.edit(project_dir)

    broken: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = run_reuse_compile(project_dir=project_dir, args=("--no-cache",))
    comparison: IncrementalEditComparison = IncrementalEditComparison(
        incremental=broken, reference=reference
    )

    assert comparison.matches is test_case.expected_matches_uncached, (
        comparison.mismatched_artifacts
    )


@pytest.mark.parametrize(
    "test_case",
    [SharedCacheKeyTestCase(description="native_dependency_order")],
    ids=lambda case: case.description,
)
def test_given_models_sharing_an_analysis_cache_key_when_upstream_changes_then_it_matches_uncached(
    compile_reuse_project: Path,
    test_case: SharedCacheKeyTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = compile_reuse_project
    enable_compile_reuse(monkeypatch)
    star_chain_with_twin_added(project_dir)
    assert compile_in_process(project_dir=project_dir) == 0
    staging_extra_flag(project_dir)
    twin_header_changed(project_dir)
    compile_edit_without_reuse(project_dir, monkeypatch)

    incremental: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = in_process_reuse_run(
        project_dir=project_dir, capsys=capsys, args=("--no-cache",)
    )
    comparison: IncrementalEditComparison = IncrementalEditComparison(
        incremental=incremental, reference=reference
    )

    assert comparison.matches is test_case.expected_matches_uncached, (
        comparison.mismatched_artifacts
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestScanStoreTestCase(
            description="test_file_edited",
            change=edit_sql_test_scan_input,
            expected_rescans=2,
        ),
        SqlTestScanStoreTestCase(
            description="unrelated_model_edited",
            change=edit_unrelated_model,
            expected_rescans=0,
        ),
        SqlTestScanStoreTestCase(
            description="adapter_lexical_rules_changed",
            change=change_adapter_lexical_rules,
            expected_rescans=12,
        ),
        SqlTestScanStoreTestCase(
            description="native_build_upgraded",
            change=upgrade_native_build,
            expected_rescans=12,
        ),
        SqlTestScanStoreTestCase(
            description="installed_python_code_changed",
            change=edit_installed_code,
            expected_rescans=12,
        ),
        SqlTestScanStoreTestCase(
            description="store_file_corrupted",
            change=corrupt_sql_test_scan_store,
            expected_rescans=12,
        ),
        SqlTestScanStoreTestCase(
            description="stored_entries_undecodable",
            change=replace_stored_scans_undecodable,
            expected_rescans=12,
        ),
        SqlTestScanStoreTestCase(
            description="stored_entries_misshapen",
            change=replace_stored_scans_misshapen,
            expected_rescans=12,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_sql_test_scans_when_inputs_change_then_only_changed_files_rescan(
    compile_reuse_project: Path,
    test_case: SqlTestScanStoreTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = compile_reuse_project
    disable_project_reuse(monkeypatch)
    test_case.arrange(monkeypatch)
    cold: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    test_case.change(project_dir, monkeypatch)

    incremental: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = in_process_reuse_run(
        project_dir=project_dir, capsys=capsys, args=("--no-cache",)
    )
    comparison: IncrementalEditComparison = IncrementalEditComparison(
        incremental=incremental, reference=reference
    )
    stored: int = sum(sql_test_scan_counts(cold))

    assert cold.returncode == 0, cold.stderr
    assert sql_test_scan_counts(cold) == (0, stored)
    assert sql_test_scan_counts(incremental) == (
        stored - test_case.expected_rescans,
        test_case.expected_rescans,
    )
    assert sql_test_scan_counts(reference) == (0, 0)
    assert comparison.matches is test_case.expected_matches_uncached, (
        comparison.mismatched_artifacts
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeAnalysisStoreTestCase(
            description="unchanged_project",
            change=lambda _root, _monkeypatch: None,
            expected_misses=0,
        ),
        NativeAnalysisStoreTestCase(
            description="leaf_model_comment",
            change=edit_unrelated_model,
            expected_misses=1,
        ),
        NativeAnalysisStoreTestCase(
            description="staging_type_reaches_consumers",
            change=edit_staging_type,
            expected_misses=13,
        ),
        NativeAnalysisStoreTestCase(
            description="native_build_upgraded",
            change=upgrade_native_build,
            expected_misses=17,
        ),
        NativeAnalysisStoreTestCase(
            description="installed_python_code_changed",
            change=edit_installed_code,
            expected_misses=17,
        ),
        NativeAnalysisStoreTestCase(
            description="store_file_corrupted",
            change=corrupt_native_analysis_store,
            expected_misses=17,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_native_analyses_when_inputs_change_then_only_changed_models_miss(
    compile_reuse_project: Path,
    test_case: NativeAnalysisStoreTestCase,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = compile_reuse_project
    disable_project_reuse(monkeypatch)
    cold: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    test_case.change(project_dir, monkeypatch)

    incremental: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    reference: CompileReuseRun = in_process_reuse_run(
        project_dir=project_dir, capsys=capsys, args=("--no-cache",)
    )
    analysed: int = native_analysis_counts(cold)[1]

    assert cold.returncode == 0, cold.stderr
    assert native_analysis_counts(cold) == (0, analysed, 0)
    assert native_analysis_counts(incremental) == (
        analysed - test_case.expected_misses,
        test_case.expected_misses,
        0,
    )
    assert native_analysis_counts(reference) == (0, 0, analysed)
    assert IncrementalEditComparison(incremental=incremental, reference=reference).matches, (
        incremental.stderr
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ExternalModuleEditTestCase(
            description="module_rewritten_in_place_after_a_compile",
            initial_value="'vanilla'",
            edited_value="'choco'",
            expected_matches_uncached=True,
            expected_compiled_value="'choco' AS flavor",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_module_imported_while_rendering_when_it_changes_after_a_compile_then_output_is_fresh(
    compile_reuse_project: Path, tmp_path: Path, test_case: ExternalModuleEditTestCase
) -> None:
    extlib: Path = tmp_path / "extlib"
    env: dict[str, str] = add_external_flavor_macro(
        project_dir=compile_reuse_project, extlib=extlib, value=test_case.initial_value
    )
    cold: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)
    staging_comment(compile_reuse_project)
    edited: CompileReuseRun = run_reuse_compile(project_dir=compile_reuse_project, env=env)

    write_external_flavor(extlib, test_case.edited_value)
    comparison: IncrementalEditComparison = IncrementalEditComparison(
        incremental=run_reuse_compile(project_dir=compile_reuse_project, env=env),
        reference=run_reuse_compile(
            project_dir=compile_reuse_project, env=env, args=("--no-cache",)
        ),
    )

    assert (cold.returncode, edited.returncode) == (0, 0), edited.stderr
    assert comparison.incremental.reused is False
    assert comparison.matches is test_case.expected_matches_uncached, (
        comparison.mismatched_artifacts
    )
    assert test_case.expected_compiled_value in compiled_text(
        run=comparison.incremental, suffix="fact_orders.sql"
    )


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
