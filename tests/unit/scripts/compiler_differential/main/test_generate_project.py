"""Generated projects are deterministic per seed and cover the intended feature space."""

from __future__ import annotations

import random

import pytest

from scripts.compiler_differential.classes.discovery_features import feature_blocks_for_seed
from scripts.compiler_differential.constants import (
    DEFAULT_SEED_COUNT,
    GENERATOR_FEATURE_BLOCKS,
    GENERATOR_FEATURE_STRIDE,
    GENERATOR_RARE_FEATURE_BLOCKS,
    PLAN_LABEL,
)
from scripts.compiler_differential.main.generate_project import generate_project
from scripts.compiler_differential.models import GeneratedProject
from tests.unit.scripts.compiler_differential.main._test_types import (
    ExcludedFeaturePairTestCase,
    FeatureBlockWindowTestCase,
    RareFeatureSeedTestCase,
    SeedDeterminismTestCase,
    SeedRangeTestCase,
)
from tests.unit.scripts.compiler_differential.main.helpers import (
    generated_projects,
    invalid_count,
    project_features,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SeedDeterminismTestCase(description=f"seed_{seed}", seed=seed, expected_identical=True)
        for seed in (0, 7, 41)
    ],
    ids=lambda case: case.description,
)
def test_given_same_seed_when_generating_twice_then_projects_are_identical(
    test_case: SeedDeterminismTestCase,
) -> None:
    first: GeneratedProject = generate_project(seed=test_case.seed)
    second: GeneratedProject = generate_project(seed=test_case.seed)

    assert (first == second) is test_case.expected_identical


@pytest.mark.parametrize(
    "test_case",
    [
        SeedRangeTestCase(
            description="first_sixty_seeds",
            seeds=range(60),
            expected_features=frozenset(
                {
                    "exact_owner_scope",
                    "tree_scope",
                    "project_scope",
                    "model_private_value",
                    "relationship_grant",
                    "name_shared_across_namespaces",
                    "non_ascii_constant",
                    "float_constant",
                    "decimal_constant",
                    "integer_enum",
                    "macro_reads_ctx",
                    "macro_reads_env",
                    "nested_macro",
                    "path_defaults_literal",
                    "path_defaults_wildcard",
                    "sql_hooks",
                    "python_nodes",
                    "unit_test",
                    "scenario",
                    "singular_audit",
                    "column_audits",
                    *GENERATOR_FEATURE_BLOCKS,
                }
            ),
            expected_distinct_projects=60,
            expected_min_invalid=5,
            expected_max_invalid=30,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_seed_range_when_generating_then_features_and_failures_are_covered(
    test_case: SeedRangeTestCase,
) -> None:
    projects: list[GeneratedProject] = generated_projects(test_case.seeds)

    assert test_case.expected_features <= project_features(projects)
    assert len({tuple(project.files.items()) for project in projects}) == (
        test_case.expected_distinct_projects
    )
    assert (
        test_case.expected_min_invalid <= invalid_count(projects) <= test_case.expected_max_invalid
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FeatureBlockWindowTestCase(
            description=f"seeds_{start}_to_{start + GENERATOR_FEATURE_STRIDE - 1}",
            seeds=range(start, start + GENERATOR_FEATURE_STRIDE),
            expected_blocks=frozenset(GENERATOR_FEATURE_BLOCKS)
            - set(GENERATOR_RARE_FEATURE_BLOCKS),
        )
        for start in (0, 5, 37)
    ],
    ids=lambda case: case.description,
)
def test_given_consecutive_seeds_when_choosing_blocks_then_every_common_block_is_forced(
    test_case: FeatureBlockWindowTestCase,
) -> None:
    chosen: set[str] = set()
    for seed in test_case.seeds:
        chosen.update(feature_blocks_for_seed(seed=seed, rng=random.Random(seed)))

    assert test_case.expected_blocks <= chosen


@pytest.mark.parametrize(
    "test_case",
    [
        RareFeatureSeedTestCase(
            description="dbt_ref_seed_fails_compile_but_plans",
            seed=GENERATOR_RARE_FEATURE_BLOCKS["dbt_ref"],
            expected_feature="dbt_ref",
            expected_error_code="C214",
            expected_succeeding_commands=(PLAN_LABEL,),
            other_seeds=(0, *range(2, DEFAULT_SEED_COUNT)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_rare_block_seed_when_generating_then_outcome_is_declared(
    test_case: RareFeatureSeedTestCase,
) -> None:
    project: GeneratedProject = generate_project(seed=test_case.seed)
    others: list[GeneratedProject] = [generate_project(seed=seed) for seed in test_case.other_seeds]

    assert test_case.expected_feature in project.features
    assert project.expected_error_code == test_case.expected_error_code
    assert project.succeeding_commands == test_case.expected_succeeding_commands
    assert not any(test_case.expected_feature in other.features for other in others)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    [
        ExcludedFeaturePairTestCase(
            description="no seed loads through a @loader on a project adapter",
            seeds=range(DEFAULT_SEED_COUNT * 8),
            pair=("managed_source_loader", "project_adapter"),
            expected_any_with_both=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_excluded_feature_pair_when_choosing_blocks_then_no_seed_has_both(
    test_case: ExcludedFeaturePairTestCase,
) -> None:
    any_with_both: bool = any(
        set(test_case.pair) <= set(feature_blocks_for_seed(seed=seed, rng=random.Random(seed)))
        for seed in test_case.seeds
    )

    assert any_with_both is test_case.expected_any_with_both
