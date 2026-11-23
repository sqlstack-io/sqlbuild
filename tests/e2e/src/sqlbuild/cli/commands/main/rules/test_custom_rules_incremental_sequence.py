"""Incremental custom Rules across split hosts must equal a cache-free compile after every edit."""

import shutil
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    replace_project_text,
    write_project_file,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    OneModelCustomRuleEditCase,
    RulesCacheEditCase,
    RulesEditChainCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    ONE_MODEL_EDIT_CACHED_RESULTS,
    ONE_MODEL_EDIT_PROJECT,
    custom_rules_edit_chain,
    rule_cache_counts,
    rules_compile_outcome,
    rules_compile_run,
    write_project_files,
)

CONFIG: str = "sqlbuild_project.toml"
ORDER_TOTALS: str = "sql/models/marts/order_totals.sql"
SOURCES: str = "sql/sources/raw.yml"
APPROVED: str = "rules/approved_sources.yaml"
CUSTOM_EDIT_SEQUENCE: tuple[RulesCacheEditCase, ...] = (
    RulesCacheEditCase(
        "model_sql_edited",
        lambda root: replace_project_text(root, ORDER_TOTALS, "amount > 7", "amount > 9"),
    ),
    RulesCacheEditCase(
        "source_in_shared_file_renamed",
        lambda root: replace_project_text(root, SOURCES, "raw_products", "raw_returns"),
    ),
    RulesCacheEditCase(
        "tracked_rule_input_edited",
        lambda root: replace_project_text(
            root, APPROVED, "raw_customers\n", "raw_customers\nraw_returns\n"
        ),
    ),
    RulesCacheEditCase(
        "mart_added",
        lambda root: write_project_file(
            root,
            "sql/models/marts/customer_totals.sql",
            'MODEL (description "Customer totals");\n\n'
            'SELECT customer_id FROM __ref("stg_customers")\n',
        ),
    ),
    RulesCacheEditCase(
        "model_removed",
        lambda root: (root / "sql/models/intermediate/orders_007.sql").unlink(),
    ),
    RulesCacheEditCase(
        "rules_cache_disabled",
        lambda root: replace_project_text(
            root,
            CONFIG,
            "[rules.thresholds]",
            "[rules.cache]\nenabled = false\n\n[rules.thresholds]",
        ),
    ),
    RulesCacheEditCase(
        "source_edited_without_cache",
        lambda root: replace_project_text(root, SOURCES, "raw_returns", "raw_refunds"),
    ),
    RulesCacheEditCase(
        "rules_cache_enabled",
        lambda root: replace_project_text(root, CONFIG, "[rules.cache]\nenabled = false\n\n", ""),
    ),
    RulesCacheEditCase(
        "source_restored_with_cache",
        lambda root: replace_project_text(root, SOURCES, "raw_refunds", "raw_returns"),
    ),
)


@pytest.mark.parametrize(
    "test_case",
    (
        RulesEditChainCase(
            description="model_source_input_and_cache_toggle_edits",
            edits=CUSTOM_EDIT_SEQUENCE,
            expected_cold_codes=("XSQBRORD102", "XSQBRORD103", "XSQBRORD201"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warm_custom_rules_when_applying_edit_chain_then_every_compile_equals_uncached(
    tmp_path: Path, test_case: RulesEditChainCase
) -> None:
    cold, edited, oracles = custom_rules_edit_chain(root=tmp_path, edits=test_case.edits)
    rewarmed: tuple[int, str, str] = rules_compile_outcome(tmp_path / "orders")

    assert cold[0] == test_case.expected_exit_code
    assert all(f'"{code}"' in cold[1] for code in test_case.expected_cold_codes), cold[1]
    assert edited == oracles
    assert {outcome[0] for outcome in edited} == {test_case.expected_exit_code}
    assert '"XSQBRORD101"' in edited[0][1]
    assert ("raw_returns" in edited[1][1], "raw_returns" in edited[2][1]) == (True, False)
    assert rewarmed == oracles[-1]
    assert rule_cache_counts(tmp_path / "orders")[1] == 0


@pytest.mark.parametrize(
    "test_case",
    (
        OneModelCustomRuleEditCase(
            description="edit_adds_a_finding",
            edited_model="sql/models/orders_002.sql",
            before="SELECT 2 AS",
            after="SELECT 3 AS",
            expected_warm_counts=(ONE_MODEL_EDIT_CACHED_RESULTS, 0),
            expected_edit_counts=(ONE_MODEL_EDIT_CACHED_RESULTS - 1, 1),
            expected_edit_exit_code=1,
        ),
        OneModelCustomRuleEditCase(
            description="edit_removes_the_finding",
            edited_model="sql/models/orders_003.sql",
            before="SELECT 3 AS",
            after="SELECT 30 AS",
            expected_warm_counts=(ONE_MODEL_EDIT_CACHED_RESULTS, 0),
            expected_edit_counts=(ONE_MODEL_EDIT_CACHED_RESULTS - 1, 1),
            expected_edit_exit_code=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_custom_rule_when_one_model_is_edited_then_only_that_model_reruns(
    tmp_path: Path, test_case: OneModelCustomRuleEditCase
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_project_files(project_dir=project_dir, files=ONE_MODEL_EDIT_PROJECT)
    cold, cold_counts = rules_compile_run(project_dir)
    _, warm_counts = rules_compile_run(project_dir)
    replace_project_text(project_dir, test_case.edited_model, test_case.before, test_case.after)

    edited, edit_counts = rules_compile_run(project_dir)
    oracle_dir: Path = tmp_path / "oracle"
    shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
    oracle: tuple[int, str, str] = rules_compile_outcome(oracle_dir, "--no-cache")

    assert cold[0] == 1, cold[1]
    assert cold_counts == (0, ONE_MODEL_EDIT_CACHED_RESULTS)
    assert warm_counts == test_case.expected_warm_counts
    assert edit_counts == test_case.expected_edit_counts
    assert edited[0] == test_case.expected_edit_exit_code, edited[1]
    assert edited == oracle


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
