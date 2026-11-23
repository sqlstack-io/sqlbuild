import json

import pytest

from sqlbuild._native import PythonNodeDiscoveryError
from sqlbuild.assets import asset
from sqlbuild.discovery import Discovered, Item, get_discover_definition
from sqlbuild.python_nodes.main.discovered_result_json import discovered_result_json
from sqlbuild.python_nodes.main.read_asset_definition import read_asset_definition
from sqlbuild.python_nodes.models import AssetDefinition, DiscoverDefinition
from sqlbuild.refs import model, tag
from tests.unit.src.sqlbuild.python_nodes.main._test_types import (
    DiscoverAuthoringErrorTestCase,
    DiscoverDefinitionTestCase,
    DiscoveredArgsErrorTestCase,
    DiscoveredResultTestCase,
)
from tests.unit.src.sqlbuild.python_nodes.main.helpers import not_discovered, orders_plan, step

PROVIDERS: frozenset[str] = frozenset({"warehouse"})


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredResultTestCase(
            description="item IDs, model and tag dependencies and args become JSON",
            items=(
                Item(id="load", args={"step_id": "a"}),
                Item(
                    id="fit",
                    depends_on=("load", model("orders"), tag("daily", exclude="slow")),
                    args={"step_id": "b", "retries": 3},
                ),
            ),
            expected_items=[
                {"id": "load", "depends_on": [], "args": {"step_id": "a"}},
                {
                    "id": "fit",
                    "depends_on": [
                        {"item": "load"},
                        {"model": "orders"},
                        {"tag": {"name": "daily", "exclude": "slow"}},
                    ],
                    "args": {"step_id": "b", "retries": 3},
                },
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_items_when_serialising_then_dependencies_and_args_keep_their_kinds(
    test_case: DiscoveredResultTestCase,
) -> None:
    returned: Discovered = Discovered(items=test_case.items, tags={"release": "r1"}, max_parallel=2)

    payload: object = json.loads(
        discovered_result_json(returned=returned, functions=(step,), provider_names=PROVIDERS)
    )

    assert payload == {
        "items": test_case.expected_items,
        "tags": {"release": "r1"},
        "max_parallel": 2,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredArgsErrorTestCase(
            description="an unknown key is refused",
            args={"step_id": "a", "speed": 2},
            expected_error_fragment="item 'x': step has no parameter speed",
        ),
        DiscoveredArgsErrorTestCase(
            description="a parameter without a default must be supplied",
            args={"retries": 2},
            expected_error_fragment="item 'x': args leave step's step_id unset",
        ),
        DiscoveredArgsErrorTestCase(
            description="a provider parameter cannot be supplied",
            args={"step_id": "a", "warehouse": "x"},
            expected_error_fragment="item 'x': step has no parameter warehouse",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_args_that_do_not_fit_the_asset_when_serialising_then_the_set_fails(
    test_case: DiscoveredArgsErrorTestCase,
) -> None:
    returned: Discovered = Discovered(items=(Item(id="x", args=test_case.args),))

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discovered_result_json(returned=returned, functions=(step,), provider_names=PROVIDERS)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverDefinitionTestCase(
            description="the asset names its discover function's set",
            expected_definition=DiscoverDefinition(name="orders_plan", check_every="15m"),
            expected_for_each="orders_plan",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_discover_function_when_reading_definitions_then_the_asset_names_its_set(
    test_case: DiscoverDefinitionTestCase,
) -> None:
    discover_definition: DiscoverDefinition | None = get_discover_definition(orders_plan)
    asset_definition: AssetDefinition | None = read_asset_definition(step)

    assert (discover_definition, asset_definition and asset_definition.for_each) == (
        test_case.expected_definition,
        test_case.expected_for_each,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverAuthoringErrorTestCase(
            description="for_each refuses a function that is not a discover function",
            expected_error_fragment="for_each takes a function decorated with @discover",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_plain_function_when_used_as_for_each_then_the_decorator_refuses_it(
    test_case: DiscoverAuthoringErrorTestCase,
) -> None:
    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        asset(for_each=not_discovered)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverAuthoringErrorTestCase(
            description="a list is not a Discovered",
            expected_error_fragment="must return Discovered",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_return_value_that_is_not_discovered_when_serialising_then_it_names_the_type(
    test_case: DiscoverAuthoringErrorTestCase,
) -> None:
    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discovered_result_json(returned=[], functions=(), provider_names=frozenset())


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
