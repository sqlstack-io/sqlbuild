from __future__ import annotations

import pytest

from sqlbuild._native import (
    MissingUpstream,
    WarehouseSnapshot,
    check_buildability,
    missing_upstream_message,
)
from sqlbuild.adapter.contract.models import RelationInfo
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    CheckBuildabilityTestCase,
    MissingUpstreamMessageTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    build_snapshot_from_relation_names,
    dbt_ref_key,
    model_key,
    seed_key,
    source_key,
)

_RELATIONSHIPS_ORIGIN: str = "audit 'relationships' on 'stg_orders' reads 'waffle_types'"
_STATUS_ORIGIN: str = "audit 'known_status' on 'orders' reads 'order_statuses'"


@pytest.mark.parametrize(
    "test_case",
    [
        CheckBuildabilityTestCase(
            description="returns no missing when all deps are in scope",
            selected_keys=frozenset({model_key("a"), model_key("b")}),
            upstream_deps={
                model_key("a"): (),
                model_key("b"): (model_key("a"),),
            },
            existing_relation_names=(),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="returns no missing when dep exists in warehouse",
            selected_keys=frozenset({model_key("b")}),
            upstream_deps={
                model_key("b"): (model_key("a"),),
            },
            existing_relation_names=("a",),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="returns missing dep not in scope or warehouse",
            selected_keys=frozenset({model_key("b")}),
            upstream_deps={
                model_key("b"): (model_key("a"),),
            },
            existing_relation_names=(),
            expected_missing=(
                MissingUpstream(
                    key=model_key("a"),
                    required_by=(model_key("b"),),
                ),
            ),
        ),
        CheckBuildabilityTestCase(
            description="sorts missing by dependent count descending",
            selected_keys=frozenset({model_key("x"), model_key("y"), model_key("z")}),
            upstream_deps={
                model_key("x"): (model_key("a"), model_key("b")),
                model_key("y"): (model_key("a"),),
                model_key("z"): (model_key("b"),),
            },
            existing_relation_names=(),
            expected_missing=(
                MissingUpstream(
                    key=model_key("a"),
                    required_by=(model_key("x"), model_key("y")),
                ),
                MissingUpstream(
                    key=model_key("b"),
                    required_by=(model_key("x"), model_key("z")),
                ),
            ),
        ),
        CheckBuildabilityTestCase(
            description=(
                "dbt ref dep is external and does not require warehouse relation by logical name"
            ),
            selected_keys=frozenset({model_key("report")}),
            upstream_deps={
                model_key("report"): (dbt_ref_key("analytics.fact_orders"),),
            },
            existing_relation_names=(),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="source dep satisfied by warehouse relation",
            selected_keys=frozenset({model_key("orders")}),
            upstream_deps={
                model_key("orders"): (source_key("raw_orders"),),
            },
            existing_relation_names=("raw_orders",),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="seed dep in scope is not missing",
            selected_keys=frozenset({model_key("report"), seed_key("countries")}),
            upstream_deps={
                model_key("report"): (seed_key("countries"),),
                seed_key("countries"): (),
            },
            existing_relation_names=(),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="returns no missing for empty scope",
            selected_keys=frozenset(),
            upstream_deps={
                model_key("a"): (model_key("b"),),
            },
            existing_relation_names=(),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="deferred relation satisfies missing upstream",
            selected_keys=frozenset({model_key("b")}),
            upstream_deps={
                model_key("b"): (model_key("a"),),
            },
            existing_relation_names=(),
            deferred_relation_names=("a",),
            expected_missing=(),
        ),
        CheckBuildabilityTestCase(
            description="deferred relation does not satisfy unrelated upstream",
            selected_keys=frozenset({model_key("b")}),
            upstream_deps={
                model_key("b"): (model_key("a"),),
            },
            existing_relation_names=(),
            deferred_relation_names=("c",),
            expected_missing=(
                MissingUpstream(
                    key=model_key("a"),
                    required_by=(model_key("b"),),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scope_and_snapshot_when_checking_buildability_then_returns_expected(
    test_case: CheckBuildabilityTestCase,
) -> None:
    snapshot: WarehouseSnapshot = build_snapshot_from_relation_names(
        test_case.existing_relation_names
    )
    deferred_relations: dict[str, RelationInfo] = {
        name: RelationInfo(database=None, schema="prod", name=name, relation_type="BASE TABLE")
        for name in test_case.deferred_relation_names
    }

    result: tuple[MissingUpstream, ...] = check_buildability(
        selected_keys=test_case.selected_keys,
        upstream_deps=test_case.upstream_deps,
        snapshot=snapshot,
        deferred_relations=deferred_relations or None,
    )

    assert result == test_case.expected_missing


@pytest.mark.parametrize(
    "test_case",
    [
        MissingUpstreamMessageTestCase(
            description="data-lineage misses are listed with how to build them",
            missing=(
                MissingUpstream(key=model_key("a"), required_by=(model_key("b"),)),
                MissingUpstream(key=model_key("c"), required_by=(model_key("b"),)),
            ),
            edge_origins={},
            expected_message=(
                "2 upstream models this build reads don't exist in the warehouse\n"
                "  a\n  c\n\n"
                "  Build them too:  add + before your selection"
            ),
        ),
        MissingUpstreamMessageTestCase(
            description="a miss required only by an audit read names the audit",
            missing=(
                MissingUpstream(
                    key=seed_key("waffle_types"), required_by=(model_key("stg_orders"),)
                ),
            ),
            edge_origins={
                (model_key("stg_orders"), seed_key("waffle_types")): _RELATIONSHIPS_ORIGIN
            },
            expected_message=(
                "cannot build selected scope: audit 'relationships' on 'stg_orders' reads "
                "'waffle_types', which is not selected and does not exist in the warehouse; "
                "select it or build it first"
            ),
        ),
        MissingUpstreamMessageTestCase(
            description="a miss also required by data lineage is a lineage miss",
            missing=(
                MissingUpstream(
                    key=seed_key("waffle_types"),
                    required_by=(model_key("stg_orders"), model_key("waffle_menu")),
                ),
            ),
            edge_origins={
                (model_key("stg_orders"), seed_key("waffle_types")): _RELATIONSHIPS_ORIGIN
            },
            expected_message=(
                "1 upstream seed this build reads doesn't exist in the warehouse\n"
                "  waffle_types\n\n"
                "  Build it too:  add + before your selection"
            ),
        ),
        MissingUpstreamMessageTestCase(
            description="mixed misses list lineage dependencies and audit reads",
            missing=(
                MissingUpstream(key=model_key("a"), required_by=(model_key("b"),)),
                MissingUpstream(
                    key=seed_key("waffle_types"), required_by=(model_key("stg_orders"),)
                ),
                MissingUpstream(key=seed_key("order_statuses"), required_by=(model_key("orders"),)),
            ),
            edge_origins={
                (model_key("stg_orders"), seed_key("waffle_types")): _RELATIONSHIPS_ORIGIN,
                (model_key("orders"), seed_key("order_statuses")): _STATUS_ORIGIN,
            },
            expected_message=(
                "1 upstream model this build reads doesn't exist in the warehouse\n"
                "  a\n\n"
                "  Build it too:  add + before your selection\n\n"
                "  Also, 2 audit reads "
                f"are not selected and do not exist in the warehouse ({_RELATIONSHIPS_ORIGIN}; "
                f"{_STATUS_ORIGIN}); select them or build them first"
            ),
        ),
        MissingUpstreamMessageTestCase(
            description="more than five audit reads are truncated",
            missing=tuple(
                MissingUpstream(key=seed_key(f"codes_{index}"), required_by=(model_key("orders"),))
                for index in range(6)
            ),
            edge_origins={
                (model_key("orders"), seed_key(f"codes_{index}")): (
                    f"audit 'check_{index}' on 'orders' reads 'codes_{index}'"
                )
                for index in range(6)
            },
            expected_message=(
                "cannot build selected scope: 6 audit reads are not selected and do not exist in "
                "the warehouse ("
                + "; ".join(
                    f"audit 'check_{index}' on 'orders' reads 'codes_{index}'" for index in range(5)
                )
                + "; ...); select them or build them first"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_upstreams_when_describing_then_message_names_their_cause(
    test_case: MissingUpstreamMessageTestCase,
) -> None:
    assert (
        missing_upstream_message(missing=test_case.missing, edge_origins=test_case.edge_origins)
        == test_case.expected_message
    )
