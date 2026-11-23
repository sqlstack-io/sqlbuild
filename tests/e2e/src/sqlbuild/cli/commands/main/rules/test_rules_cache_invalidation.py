"""A warm Rules compile after any single input edit must equal a compile without caches."""

import shutil
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    replace_project_text,
    write_project_file,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import RulesCacheEditCase
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    INCREMENTAL_RULES_PROJECT,
    move_order_totals_description_to_path_defaults,
    rule_cache_counts,
    rules_compile_outcome,
    write_project_files,
)


@pytest.mark.parametrize(
    "test_case",
    (
        RulesCacheEditCase(
            "model_sql_edit",
            lambda root: replace_project_text(
                root,
                "sql/models/staging/stg_orders.sql",
                "CAST(10.5 AS DOUBLE)",
                "CAST(12.5 AS DOUBLE)",
            ),
        ),
        RulesCacheEditCase(
            "model_contract_edit",
            lambda root: replace_project_text(
                root, "sql/models/staging/stg_orders.sql", "  contract enforced,\n", ""
            ),
        ),
        RulesCacheEditCase(
            "model_description_edited",
            lambda root: replace_project_text(
                root,
                "sql/models/staging/stg_orders.sql",
                "Test model stg_orders.",
                "Staged order rows.",
            ),
        ),
        RulesCacheEditCase(
            "model_description_removed",
            lambda root: replace_project_text(
                root,
                "sql/models/marts/order_totals.sql",
                "MODEL (description 'Test model order_totals.',\n",
                "MODEL (\n",
            ),
            expected_diagnostics_fragment='"code": "P010"',
        ),
        RulesCacheEditCase(
            "model_description_moved_to_path_defaults",
            move_order_totals_description_to_path_defaults,
        ),
        RulesCacheEditCase(
            "model_added",
            lambda root: write_project_file(
                root,
                "sql/models/marts/order_counts.sql",
                'MODEL (description "Test model order_counts.");\n\nSELECT order_id FROM __ref("stg_orders")\n',
            ),
        ),
        RulesCacheEditCase(
            "consumer_removed",
            lambda root: replace_project_text(
                root,
                "sql/models/marts/order_totals.sql",
                'FROM __ref("stg_orders") AS o\n',
                "FROM (SELECT 1 AS order_id, 1.0 AS amount) AS o\n",
            ),
        ),
        RulesCacheEditCase(
            "test_edit",
            lambda root: replace_project_text(
                root,
                "tests/unit/test_order_totals.sql",
                "TEST ();",
                'TEST (name "order_totals_applies_discount");',
            ),
        ),
        RulesCacheEditCase(
            "macro_edit",
            lambda root: replace_project_text(
                root, "sql/models/marts/_sqlbuild/_macros/python/amounts.py", "* 0.9", "* 0.8"
            ),
        ),
        RulesCacheEditCase(
            "var_edit",
            lambda root: replace_project_text(
                root, "sqlbuild_project.toml", 'discount_rate = "0.1"', 'discount_rate = "0.2"'
            ),
        ),
        RulesCacheEditCase(
            "rule_source_edit",
            lambda root: replace_project_text(
                root,
                "rules/governance.py",
                '"Contracts must be enforced"',
                '"Output contracts must be enforced"',
            ),
        ),
        RulesCacheEditCase(
            "rule_helper_edit",
            lambda root: replace_project_text(root, "rules/limits.py", "= 30", "= 5"),
        ),
        RulesCacheEditCase(
            "tree_file_edit",
            lambda root: replace_project_text(root, "config/policy.yml", "false", "true"),
        ),
        RulesCacheEditCase(
            "tree_file_added",
            lambda root: write_project_file(root, "config/extra.yml", "enabled: true\n"),
        ),
        RulesCacheEditCase(
            "rules_threshold_edit",
            lambda root: replace_project_text(
                root,
                "sqlbuild_project.toml",
                "min_custom_rule_test_cases = 0\n",
                "min_custom_rule_test_cases = 0\nmin_tests_per_model = 0\n",
            ),
        ),
        RulesCacheEditCase(
            "target_switch",
            lambda root: write_project_file(root, "sqlbuild_local.toml", 'target = "prod"\n'),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warm_rules_cache_when_one_input_changes_then_compile_equals_uncached_compile(
    tmp_path: Path, test_case: RulesCacheEditCase
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_project_files(project_dir=project_dir, files=INCREMENTAL_RULES_PROJECT)
    cold: tuple[int, str, str] = rules_compile_outcome(project_dir)
    assert cold[0] == 1
    assert '"XSQBRGOV001"' in cold[1]
    hits, misses = rule_cache_counts(project_dir)
    assert hits > 0
    assert misses == 0

    test_case.edit(project_dir)
    edited: tuple[int, str, str] = rules_compile_outcome(project_dir)
    oracle_dir: Path = tmp_path / "oracle"
    shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
    oracle: tuple[int, str, str] = rules_compile_outcome(oracle_dir, "--no-cache")
    rewarmed: tuple[int, str, str] = rules_compile_outcome(project_dir)

    assert edited[0] == test_case.expected_exit_code
    assert test_case.expected_diagnostics_fragment in edited[1]
    assert edited == oracle
    assert rewarmed == oracle
    assert rule_cache_counts(project_dir)[1] == 0


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
