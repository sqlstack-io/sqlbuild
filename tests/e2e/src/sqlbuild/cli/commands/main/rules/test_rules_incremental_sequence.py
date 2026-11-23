"""Incremental built-in Rules must equal a cache-free compile across a chain of edits."""

import shutil
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    move_project_file,
    replace_project_text,
    write_project_file,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    RulesCacheEditCase,
    RulesEditChainCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    BUILT_IN_RULES_PROJECT,
    rule_cache_counts,
    rules_compile_outcome,
    write_bulk_rules_cache,
    write_project_files,
)

_CONFIG: str = "sqlbuild_project.toml"
_ORDER_TOTALS: str = "sql/models/commerce/mart/commerce__mart__order_totals.sql"
_PRODUCTS: str = "sql/models/commerce/mart/commerce__mart__products.sql"

EDIT_SEQUENCE: tuple[RulesCacheEditCase, ...] = (
    RulesCacheEditCase(
        "model_sql_edited",
        lambda root: replace_project_text(root, _ORDER_TOTALS, "amount > 7", "amount > 9"),
    ),
    RulesCacheEditCase(
        "model_added",
        lambda root: write_project_file(
            root,
            _PRODUCTS,
            'MODEL (description "Products");\n\nSELECT * FROM __ref("commerce__stg__orders")\n',
        ),
    ),
    RulesCacheEditCase(
        "model_removed",
        lambda root: (root / "sql/models/commerce/mart/commerce__mart__customers.sql").unlink(),
    ),
    RulesCacheEditCase(
        "model_renamed",
        lambda root: move_project_file(
            root, _PRODUCTS, "sql/models/commerce/mart/commerce__mart__catalog_products.sql"
        ),
    ),
    RulesCacheEditCase(
        "threshold_changed",
        lambda root: replace_project_text(
            root, _CONFIG, "min_tests_per_model = 1", "min_tests_per_model = 0"
        ),
    ),
    RulesCacheEditCase(
        "scoped_ignore_added",
        lambda root: replace_project_text(
            root,
            _CONFIG,
            "[rules.thresholds]",
            '[[rules.rule_ignores]]\nrules = ["SQBRTEST201"]\n'
            'paths = ["sql/models/commerce/staging/commerce__stg__orders.sql"]\n'
            'reason = "Audits arrive with the source contract"\n\n[rules.thresholds]',
        ),
    ),
    RulesCacheEditCase(
        "exception_added",
        lambda root: replace_project_text(
            root,
            _CONFIG,
            "[rules.thresholds]",
            f'[[rules.rule_exceptions]]\nrule = "SQBRCONTRACT101"\npath = "{_ORDER_TOTALS}"\n'
            'reason = "Contract arrives with the next release"\n\n[rules.thresholds]',
        ),
    ),
    RulesCacheEditCase(
        "rules_toggled_off",
        lambda root: replace_project_text(
            root,
            _CONFIG,
            'domains = ["commerce"]',
            'domains = ["commerce"]\nignore = ["SQBRDECLARATION", "SQBRGRAPH"]',
        ),
    ),
    RulesCacheEditCase(
        "dialect_changed",
        lambda root: replace_project_text(
            root,
            _CONFIG,
            'adapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"',
            'adapter = "snowflake"\n\n[defaults]\ndatabase = "warehouse"\nschema = "analytics"',
        ),
    ),
    RulesCacheEditCase(
        "rules_toggled_on",
        lambda root: replace_project_text(
            root, _CONFIG, '\nignore = ["SQBRDECLARATION", "SQBRGRAPH"]', ""
        ),
    ),
)


@pytest.mark.parametrize(
    "test_case",
    (
        RulesEditChainCase(
            description="model_config_and_dialect_edits",
            edits=EDIT_SEQUENCE,
            expected_cold_codes=("SQBRGRAPH102", "SQBRTEST104"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warm_rules_cache_when_applying_edit_chain_then_every_compile_equals_uncached(
    tmp_path: Path, test_case: RulesEditChainCase
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_project_files(project_dir=project_dir, files=BUILT_IN_RULES_PROJECT)
    cold: tuple[int, str, str] = rules_compile_outcome(project_dir)
    assert cold[0] == test_case.expected_exit_code
    for code in test_case.expected_cold_codes:
        assert f'"{code}"' in cold[1], code
    oracle: tuple[int, str, str] = cold

    for step, edit_case in enumerate(test_case.edits):
        edit_case.edit(project_dir)
        edited: tuple[int, str, str] = rules_compile_outcome(project_dir)
        oracle_dir: Path = tmp_path / f"oracle_{step}"
        shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
        oracle = rules_compile_outcome(oracle_dir, "--no-cache")

        assert edited == oracle, edit_case.description
        assert edited[0] == edit_case.expected_exit_code, edit_case.description

    rewarmed: tuple[int, str, str] = rules_compile_outcome(project_dir)
    assert rewarmed == oracle
    assert rule_cache_counts(project_dir)[1] == 0


@pytest.mark.parametrize(
    "test_case",
    (
        RulesCacheEditCase(
            "corrupt_model_findings_cache",
            lambda root: write_bulk_rules_cache(
                root=root, file_name="native.json", text='{"fingerprint": "trunc'
            ),
        ),
        RulesCacheEditCase(
            "corrupt_response_memo",
            lambda root: write_bulk_rules_cache(
                root=root, file_name="native-response.json", text='{"identity": '
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_corrupt_built_in_rules_cache_when_compiling_then_rules_fall_back_to_evaluation(
    tmp_path: Path, test_case: RulesCacheEditCase
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_project_files(project_dir=project_dir, files=BUILT_IN_RULES_PROJECT)
    cold: tuple[int, str, str] = rules_compile_outcome(project_dir)
    replace_project_text(project_dir, _ORDER_TOTALS, "amount > 7", "amount > 9")
    test_case.edit(project_dir)

    recovered: tuple[int, str, str] = rules_compile_outcome(project_dir)
    oracle_dir: Path = tmp_path / "oracle"
    shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
    oracle: tuple[int, str, str] = rules_compile_outcome(oracle_dir, "--no-cache")

    assert cold[0] == recovered[0] == test_case.expected_exit_code
    assert recovered == oracle


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
