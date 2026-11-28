"""Warm compile-cache reuse must match a cache-disabled compile after every input edit."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    CompileCacheDisabledTestCase,
    CompileCacheInvalidationTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    COMPILE_CACHE_REGION_ENV_VAR,
    CompileCacheOutcome,
    compile_cache_outcome,
    move_project_file,
    replace_project_text,
    run_installed_sqb,
    write_project_file,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileCacheInvalidationTestCase(
            description="model_sql_body_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/models/staging/stg_orders.sql",
                "  quantity,",
                "  quantity + 0 AS quantity,",
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="model_header_metadata_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/models/staging/stg_orders.sql",
                "customer_id (nullable false, audits [not_null]),",
                'customer_id (nullable false, description "Buyer.", audits [not_null, unique]),',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="model_header_syntax_error",
            edit=lambda root: replace_project_text(
                root,
                "sql/models/staging/stg_orders.sql",
                "materialized view,",
                "materialized view,,((",
            ),
            expected_failure=True,
        ),
        CompileCacheInvalidationTestCase(
            description="model_added",
            edit=lambda root: write_project_file(
                root,
                "sql/models/marts/order_quantities.sql",
                'MODEL (description "Test model order_quantities.");\n\nSELECT order_id, quantity FROM __ref("stg_orders")\n',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="model_deleted",
            edit=lambda root: (root / "sql/models/marts/channel_orders.sql").unlink(),
            expected_failure=True,
        ),
        CompileCacheInvalidationTestCase(
            description="model_moved_between_folders",
            edit=lambda root: move_project_file(
                root,
                "sql/models/staging/stg_payments.sql",
                "sql/models/staging/payments/stg_payments.sql",
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="source_yaml_column_type_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/sources/raw.yml",
                "      - name: quantity\n        type: BIGINT",
                "      - name: quantity\n        type: INT8",
            ),
            expected_output_change=False,
        ),
        CompileCacheInvalidationTestCase(
            description="source_yaml_column_rename",
            edit=lambda root: replace_project_text(
                root,
                "sql/sources/raw.yml",
                "      - name: quantity\n        type: BIGINT",
                "      - name: item_count\n        type: BIGINT",
            ),
            expected_failure=True,
        ),
        CompileCacheInvalidationTestCase(
            description="source_yaml_syntax_error",
            edit=lambda root: replace_project_text(
                root, "sql/sources/raw.yml", "sources:", "sources: [\n"
            ),
            expected_failure=True,
        ),
        CompileCacheInvalidationTestCase(
            description="unit_test_expected_edit",
            edit=lambda root: replace_project_text(
                root,
                "tests/unit/test_channel_orders.sql",
                "TRUE AS meets_minimum",
                "FALSE AS meets_minimum",
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="second_test_block_in_multi_block_file_edit",
            edit=lambda root: replace_project_text(
                root,
                "tests/unit/test_channel_blocks.sql",
                "SELECT 2 AS order_id, 'web' AS order_channel",
                "SELECT 2 AS order_id, 'partner' AS order_channel",
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="unit_test_structure_error",
            edit=lambda root: replace_project_text(
                root, "tests/unit/test_channel_orders.sql", "SELECT 1\n", "SELECT 2\n"
            ),
            expected_failure=True,
        ),
        CompileCacheInvalidationTestCase(
            description="project_macro_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/macros/python/currency.py",
                'return f"{price_cents} * {quantity}"',
                'return f"({price_cents}) * ({quantity})"',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="scoped_macro_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/models/marts/_sqlbuild/_macros/python/currency.py",
                'return f"ROUND(({column}) / 100.0, 2)"',
                'return f"ROUND(({column}) / 100.0, 3)"',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="enum_member_value_edit",
            edit=lambda root: replace_project_text(
                root,
                "sql/models/marts/_sqlbuild/_enums/order_channel.sql",
                'WEB "web"',
                'WEB "online"',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="constant_value_edit",
            edit=lambda root: replace_project_text(
                root, "sql/models/marts/_sqlbuild/_constants/min_quantity.sql", "value 1", "value 3"
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="project_var_edit",
            edit=lambda root: replace_project_text(
                root,
                "sqlbuild_project.toml",
                'quantity_multiplier = "2"',
                'quantity_multiplier = "5"',
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="target_schema_edit",
            edit=lambda root: replace_project_text(
                root, "sqlbuild_project.toml", 'schema = "dev"', 'schema = "dev_next"'
            ),
        ),
        CompileCacheInvalidationTestCase(
            description="local_config_target_switch",
            edit=lambda root: write_project_file(root, "sqlbuild_local.toml", 'target = "prod"\n'),
        ),
        CompileCacheInvalidationTestCase(
            description="python_loader_edit",
            edit=lambda root: replace_project_text(
                root,
                "python/nodes/loaders/waffle_sources/loaders.py",
                "from sqlbuild.loaders import loader\n",
                "from sqlbuild.loaders import loader\n\nLOADER_REVISION: int = 2\n",
            ),
            expected_output_change=False,
        ),
        CompileCacheInvalidationTestCase(
            description="environment_variable_edit",
            edit=lambda root: None,
            edited_env={COMPILE_CACHE_REGION_ENV_VAR: "west"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_warm_compile_cache_when_input_changes_then_output_matches_cache_disabled_compile(
    cache_invalidation_project: Path, test_case: CompileCacheInvalidationTestCase
) -> None:
    project_dir: Path = cache_invalidation_project
    initial_env: dict[str, str] = {COMPILE_CACHE_REGION_ENV_VAR: "east"}
    cold: CompileCacheOutcome = compile_cache_outcome(project_dir=project_dir, env=initial_env)
    warm: CompileCacheOutcome = compile_cache_outcome(project_dir=project_dir, env=initial_env)
    assert cold.returncode == 0
    assert cold.analysis_cache_misses > 0
    assert warm == CompileCacheOutcome(
        returncode=0,
        diagnostics=cold.diagnostics,
        fingerprint=cold.fingerprint,
        analysis_cache_hits=cold.analysis_cache_misses,
        analysis_cache_misses=0,
    )

    test_case.edit(project_dir)
    edited_env: dict[str, str] = {**initial_env, **test_case.edited_env}
    edited: CompileCacheOutcome = compile_cache_outcome(project_dir=project_dir, env=edited_env)
    reference: CompileCacheOutcome = compile_cache_outcome(
        project_dir=project_dir, env=edited_env, compile_args=("--no-cache",)
    )
    rewarmed: CompileCacheOutcome = compile_cache_outcome(project_dir=project_dir, env=edited_env)

    assert (edited.returncode != 0) is test_case.expected_failure
    assert (
        (edited.diagnostics, edited.fingerprint) != (cold.diagnostics, cold.fingerprint)
    ) is test_case.expected_output_change
    assert edited[:3] == reference[:3]
    assert rewarmed[:3] == reference[:3]
    assert rewarmed.analysis_cache_misses == test_case.expected_rewarmed_analysis_cache_misses


@pytest.mark.parametrize(
    "test_case",
    [
        CompileCacheDisabledTestCase(description="no_cache_flag", compile_args=("--no-cache",)),
        CompileCacheDisabledTestCase(
            description="disable_environment_variable",
            env={"SQLBUILD_DISABLE_COMPILE_CACHE": "1"},
        ),
        CompileCacheDisabledTestCase(
            description="target_compile_cache_false",
            edit=lambda root: replace_project_text(
                root,
                "sqlbuild_project.toml",
                'schema = "dev"',
                'schema = "dev"\ncompile_cache = false',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_disabled_compile_cache_when_compiling_then_no_analyses_are_read_or_written(
    cache_invalidation_project: Path, test_case: CompileCacheDisabledTestCase
) -> None:
    project_dir: Path = cache_invalidation_project
    test_case.edit(project_dir)
    env: dict[str, str] = {COMPILE_CACHE_REGION_ENV_VAR: "east", **test_case.env}

    first: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=("compile", "--json", *test_case.compile_args), env=env
    )
    second: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=("compile", "--json", *test_case.compile_args), env=env
    )

    assert (first.returncode, second.returncode) == (0, 0), first.stderr + second.stderr
    first_timings: dict[str, int] = json.loads(first.stdout)["compile_timings"]
    second_timings: dict[str, int] = json.loads(second.stdout)["compile_timings"]
    assert (
        first_timings["analysis_batch_cache_hits"] + first_timings["analysis_entry_cache_hits"],
        first_timings["analysis_cache_misses"],
    ) == test_case.expected_analysis_cache_counts
    assert (
        second_timings["analysis_batch_cache_hits"] + second_timings["analysis_entry_cache_hits"],
        second_timings["analysis_cache_misses"],
    ) == test_case.expected_analysis_cache_counts
    assert (
        tuple((project_dir / "target").rglob("model-analysis.sqlite3"))
        == test_case.expected_analysis_databases
    )
