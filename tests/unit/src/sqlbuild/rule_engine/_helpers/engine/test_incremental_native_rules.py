from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import RuleExemption, RuleIgnore, RulesResult
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    IncrementalCacheCountsTestCase,
    IncrementalRulesModeTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import (
    IncrementalModelSpec,
    IncrementalRulesStep,
    evaluate_incremental_step,
)

_ORDERS: IncrementalModelSpec = IncrementalModelSpec(
    name="commerce__mart__orders",
    relative_path="models/commerce/mart/commerce__mart__orders.sql",
    sql="WITH orders AS (SELECT * FROM source_orders) SELECT order_id FROM orders WHERE amount > 7",
    config_values={"materialized": "table"},
)
_CUSTOMERS: IncrementalModelSpec = IncrementalModelSpec(
    name="commerce__mart__customers",
    relative_path="models/commerce/mart/commerce__mart__customers.sql",
    sql="SELECT customer_id FROM warehouse.raw.customers",
)
_PRODUCTS: IncrementalModelSpec = IncrementalModelSpec(
    name="commerce__stg__products",
    relative_path="models/commerce/staging/commerce__stg__products.sql",
    sql="SELECT * FROM warehouse.raw.products",
)
_EDITED_ORDERS: IncrementalModelSpec = replace(_ORDERS, sql=_ORDERS.sql.replace("> 7", "> 9"))
_RENAMED_PRODUCTS: IncrementalModelSpec = replace(
    _PRODUCTS,
    name="commerce__stg__catalog_products",
    relative_path="models/commerce/staging/commerce__stg__catalog_products.sql",
)

EDIT_SEQUENCE: tuple[IncrementalRulesStep, ...] = (
    IncrementalRulesStep("cold", (_ORDERS, _CUSTOMERS)),
    IncrementalRulesStep("warm", (_ORDERS, _CUSTOMERS)),
    IncrementalRulesStep("model_edited", (_EDITED_ORDERS, _CUSTOMERS)),
    IncrementalRulesStep("model_added", (_EDITED_ORDERS, _CUSTOMERS, _PRODUCTS)),
    IncrementalRulesStep("model_removed", (_EDITED_ORDERS, _PRODUCTS)),
    IncrementalRulesStep("model_renamed", (_EDITED_ORDERS, _RENAMED_PRODUCTS)),
    IncrementalRulesStep(
        "threshold_changed",
        (_EDITED_ORDERS, _RENAMED_PRODUCTS),
        thresholds={"min_audits_per_model": 2},
    ),
    IncrementalRulesStep(
        "scoped_ignore_added",
        (_EDITED_ORDERS, _RENAMED_PRODUCTS),
        rule_ignores=(
            RuleIgnore(
                rules=("SQBRGRAPH102",),
                paths=(_RENAMED_PRODUCTS.relative_path,),
                reason="Raw products load is being retired",
            ),
        ),
    ),
    IncrementalRulesStep(
        "exception_added",
        (_EDITED_ORDERS, _RENAMED_PRODUCTS),
        rule_exceptions=(
            RuleExemption(
                rule="SQBRCONTRACT101",
                path=_EDITED_ORDERS.relative_path,
                reason="Contract arrives with the next release",
            ),
        ),
    ),
    IncrementalRulesStep(
        "rules_toggled_off", (_EDITED_ORDERS, _RENAMED_PRODUCTS), ignore=("SQBRMODEL",)
    ),
    IncrementalRulesStep(
        "dialect_changed", (_EDITED_ORDERS, _RENAMED_PRODUCTS), dialect="snowflake"
    ),
    IncrementalRulesStep("rules_toggled_on", (_EDITED_ORDERS, _RENAMED_PRODUCTS)),
)


@pytest.mark.parametrize(
    "test_case",
    (
        IncrementalRulesModeTestCase(
            description="suppressions applied",
            defer_suppressions=False,
            expected_observed_codes=("SQBRGRAPH102", "SQBRCONTRACT101", "SQBRDECLARATION102"),
        ),
        IncrementalRulesModeTestCase(
            description="suppressions deferred with initial findings",
            defer_suppressions=True,
            expected_observed_codes=("SQLCLEAN034", "SQBRGRAPH102", "SQBRCONTRACT101"),
            initial_codes=("SQLCLEAN034",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_edit_sequence_when_evaluating_incrementally_then_each_result_equals_full_evaluation(
    tmp_path: Path, test_case: IncrementalRulesModeTestCase
) -> None:
    cached_dir: Path = tmp_path / "cached"
    cached_dir.mkdir()
    observed: set[str] = set()

    for index, step in enumerate(EDIT_SEQUENCE):
        full_dir: Path = tmp_path / f"full_{index}"
        full_dir.mkdir()
        incremental: RulesResult = evaluate_incremental_step(
            step=step,
            project_dir=cached_dir,
            cache_enabled=True,
            defer_suppressions=test_case.defer_suppressions,
            initial_codes=test_case.initial_codes,
        )
        full: RulesResult = evaluate_incremental_step(
            step=step,
            project_dir=full_dir,
            cache_enabled=False,
            defer_suppressions=test_case.defer_suppressions,
            initial_codes=test_case.initial_codes,
        )

        assert incremental.findings == full.findings, step.description
        assert incremental.evaluated_models == full.evaluated_models, step.description
        observed.update(finding.code for finding in incremental.findings)

    assert set(test_case.expected_observed_codes) <= observed


@pytest.mark.parametrize(
    "test_case",
    (
        IncrementalCacheCountsTestCase(
            description="one edited model of three", expected_hits=2, expected_misses=1
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_one_edited_model_when_evaluating_incrementally_then_only_that_model_misses(
    tmp_path: Path, test_case: IncrementalCacheCountsTestCase
) -> None:
    cold: IncrementalRulesStep = IncrementalRulesStep("cold", (_ORDERS, _CUSTOMERS, _PRODUCTS))
    edited: IncrementalRulesStep = replace(cold, models=(_EDITED_ORDERS, _CUSTOMERS, _PRODUCTS))
    _ = evaluate_incremental_step(
        step=cold, project_dir=tmp_path, cache_enabled=True, defer_suppressions=False
    )

    result: RulesResult = evaluate_incremental_step(
        step=edited, project_dir=tmp_path, cache_enabled=True, defer_suppressions=False
    )

    assert (result.cache_hits, result.cache_misses) == (
        test_case.expected_hits,
        test_case.expected_misses,
    )


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
