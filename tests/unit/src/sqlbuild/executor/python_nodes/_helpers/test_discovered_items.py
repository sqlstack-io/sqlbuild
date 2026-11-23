from pathlib import Path

import pytest

from sqlbuild._native import DiscoveredAssetFunction
from sqlbuild.executor.python_nodes._helpers.discovered_items import (
    discovered_context,
    item_args,
    placeholder_failure,
)
from sqlbuild.executor.python_nodes.models import DiscoveredContext
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    DiscoveredItemNodeTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import discovered_step


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredItemNodeTestCase(
            description="an ordinary asset gets no args and no discovered context",
            discovered=None,
            expected_args=None,
            expected_item=None,
            expected_failure=None,
        ),
        DiscoveredItemNodeTestCase(
            description="an item's node gets its args and item",
            discovered={
                "set": "orders_plan",
                "item": "b",
                "args": {"n": 2},
                "tags": {"release": "r1"},
                "placeholder": False,
            },
            expected_args={"n": 2},
            expected_item="b",
            expected_failure=None,
        ),
        DiscoveredItemNodeTestCase(
            description="a placeholder fails with the discovery error",
            discovered={"set": "orders_plan", "placeholder": True, "error": "ssh timeout"},
            expected_args={},
            expected_item=None,
            expected_failure="never discovered: ssh timeout",
        ),
        DiscoveredItemNodeTestCase(
            description="a placeholder no discovery reached says so",
            discovered={"set": "orders_plan", "placeholder": True, "error": None},
            expected_args={},
            expected_item=None,
            expected_failure="never discovered: no discovery has run",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_asset_node_when_running_it_then_discovered_items_supply_args_context_or_failure(
    test_case: DiscoveredItemNodeTestCase,
) -> None:
    node: DiscoveredAssetFunction = DiscoveredAssetFunction(
        file_path=Path("/project/python/plan/node.py"),
        relative_path=Path("python/plan/node.py"),
        name="step[b]",
        function=discovered_step,
        discovered=test_case.discovered,
    )
    context: DiscoveredContext | None = discovered_context(node)

    assert (item_args(node), context and context.item, placeholder_failure(node)) == (
        test_case.expected_args,
        test_case.expected_item,
        test_case.expected_failure,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
