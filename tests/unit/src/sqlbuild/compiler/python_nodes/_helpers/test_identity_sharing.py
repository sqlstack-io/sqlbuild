"""Python-node identities stay byte-identical when one discovery pass shares their work."""

from __future__ import annotations

import importlib
import inspect
import textwrap
from contextlib import AbstractContextManager, nullcontext
from operator import attrgetter
from pathlib import Path
from types import ModuleType
from typing import Any, cast

import pytest

from sqlbuild.compiler.python_nodes._helpers.user_inventory import python_identity_pass
from sqlbuild.compiler.python_nodes.classes.identity_pass import IdentityPass
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonClassSourceTestCase,
    PythonIdentitySharingTestCase,
    PythonIdentityWorkTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    DiscoveryWork,
    discovery_work,
    identity_summary,
    imported_identity_nodes,
)

_PACKAGE: str = "shop_identity"

_SHARED_TEXT: str = '''"""Text helpers."""

import functools


def normalize_text(value):
    # leading comment
    return value.strip().lower()
    # trailing comment at body level


def _traced(function):
    @functools.wraps(function)
    def wrapper(*args, **kwargs):
        return function(*args, **kwargs)

    return wrapper


@_traced
def clean_status(value):
    return normalize_text(value).replace("-", "_")


async def fetch_status(value):
    return clean_status(value)


def one_line(value): return normalize_text(value)


shout = lambda value: normalize_text(value).upper()  # noqa: E731


def ping(value):
    return pong(value)


def pong(value):
    if value:
        return ping(value[1:])
    return value
'''

_SHARED_MONEY: str = '''"""Money helpers."""

import dataclasses

from shop_identity.shared.text import normalize_text


@dataclasses.dataclass(frozen=True)
class Money:
    cents: int
    currency: str

    def units(self):
        return to_units(self.cents)

    class Rounding:
        def apply(self, value):
            return round(to_units(value))

    @staticmethod
    def zero():
        return Money(0, normalize_text("USD"))


class Ledger:
    def describe(self):
        return """
summary at column zero
""" + str(to_units(1))


def to_units(cents):
    return cents / 100


def make_counter():
    class Counter:
        def bump(self):
            return to_units(1)

    return Counter
'''

_ORDERS_NODE: str = '''"""Orders node that reaches every shared shape."""

from shop_identity.orders.local import local_helper
from shop_identity.shared.crlf import crlf_helper
from shop_identity.shared.money import Ledger, Money, make_counter
from shop_identity.shared.text import clean_status, fetch_status, one_line, ping, shout


def orders_node(ctx):
    Money.zero()
    Money.Rounding().apply(1)
    Ledger.describe(None)
    make_counter()
    crlf_helper(one_line(shout(clean_status("a"))))
    ping("abc")
    fetch_status("x")
    return local_helper(ctx)
'''

_ORDERS_LOCAL: str = '''"""A helper inside the orders node folder."""

from shop_identity.shared.text import normalize_text


def local_helper(ctx):
    return normalize_text(str(ctx))
'''

_CUSTOMERS_NODE: str = '''"""Customers node that shares the orders folder helper."""

from shop_identity.orders.local import local_helper
from shop_identity.shared.money import Money


class CustomersNode:
    def run(self, ctx):
        return local_helper(ctx), Money(1, "usd")


def customers_node(ctx):
    return CustomersNode().run(ctx)
'''

_FILES: dict[str, bytes] = {
    "shared/text.py": _SHARED_TEXT.encode(),
    "shared/money.py": _SHARED_MONEY.encode(),
    "shared/crlf.py": b"def crlf_helper(value):\r\n    return value\r\n",
    "orders/node.py": _ORDERS_NODE.encode(),
    "orders/local.py": _ORDERS_LOCAL.encode(),
    "customers/node.py": _CUSTOMERS_NODE.encode(),
}

# Each node's dependencies and the digest of its identity, recorded before identities shared work.
_EXPECTED: dict[str, tuple[tuple[str, ...], str]] = {
    "orders_node": (
        (
            "local.py:local_helper",
            "shop_identity/shared/crlf.py:crlf_helper",
            "shop_identity/shared/money.py:Ledger.describe",
            "shop_identity/shared/money.py:Money",
            "shop_identity/shared/money.py:Money.Rounding",
            "shop_identity/shared/money.py:Money.zero",
            "shop_identity/shared/money.py:make_counter",
            "shop_identity/shared/money.py:to_units",
            "shop_identity/shared/text.py:<lambda>",
            "shop_identity/shared/text.py:clean_status",
            "shop_identity/shared/text.py:fetch_status",
            "shop_identity/shared/text.py:normalize_text",
            "shop_identity/shared/text.py:one_line",
            "shop_identity/shared/text.py:ping",
            "shop_identity/shared/text.py:pong",
        ),
        "202337d3a0810b0fb7714781365660d51b6c6f4c3bfc1d0f0bc8cb597ed9baeb",
    ),
    "customers_node": (
        (
            "node.py:CustomersNode",
            "shop_identity/orders/local.py:local_helper",
            "shop_identity/shared/money.py:Money",
            "shop_identity/shared/money.py:to_units",
            "shop_identity/shared/text.py:normalize_text",
        ),
        "bf109e7d21a57481a09980e75c2e0ab3345fec5f71018da550bcfd4eb4f122e3",
    ),
}


_NODES: dict[str, str] = {"orders_node": "orders", "customers_node": "customers"}


@pytest.mark.parametrize(
    "test_case",
    [
        PythonIdentitySharingTestCase(
            description="built node by node", shared_pass=False, expected_summaries=_EXPECTED
        ),
        PythonIdentitySharingTestCase(
            description="built in one shared pass", shared_pass=True, expected_summaries=_EXPECTED
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_shared_shapes_when_identities_are_built_then_they_match_recorded_identities(
    test_case: PythonIdentitySharingTestCase, tmp_path: Path
) -> None:
    sharing: AbstractContextManager[None] = [nullcontext(), python_identity_pass()][
        test_case.shared_pass
    ]

    with (
        imported_identity_nodes(
            project_dir=tmp_path, package=_PACKAGE, files=_FILES, nodes=_NODES
        ) as nodes,
        sharing,
    ):
        summaries: dict[str, tuple[tuple[str, ...], str]] = {
            name: identity_summary(name=name, node=node) for name, node in nodes.items()
        }

    assert summaries == test_case.expected_summaries


@pytest.mark.parametrize(
    "test_case",
    [
        PythonIdentityWorkTestCase(
            description="100 and 500 nodes", node_counts=(100, 500), expected_most_per_unit=1
        )
    ],
    ids=lambda case: case.description,
)
def test_given_nodes_sharing_helpers_when_discovered_then_each_file_and_object_is_processed_once(
    test_case: PythonIdentityWorkTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    small_count, large_count = test_case.node_counts

    small: DiscoveryWork = discovery_work(
        project_dir=tmp_path / "small", node_count=small_count, monkeypatch=monkeypatch
    )
    large: DiscoveryWork = discovery_work(
        project_dir=tmp_path / "large", node_count=large_count, monkeypatch=monkeypatch
    )

    assert (small.assets, large.assets) == test_case.node_counts
    assert (small.most_per_unit, large.most_per_unit) == (
        test_case.expected_most_per_unit,
        test_case.expected_most_per_unit,
    )
    assert large.shared == small.shared
    assert large.per_node == small.per_node


@pytest.mark.parametrize(
    "test_case",
    [
        PythonClassSourceTestCase(
            description="shadowed, nested, local, decorated and trailing classes",
            shapes_path="benchmarks/duckshop/edge_cases/orchestration/python_identity_class_shapes.py",
            object_paths=(
                "Shadowed",
                "Shadowed.second",
                "Outer",
                "Outer.Inner",
                "Outer.Inner.Deepest",
                "Outer.method",
                "Decorated",
                "Decorated.__lt__",
                "Trailing",
                "factory",
                "LocalMade",
                "OuterLocal",
            ),
            expected_sources_like_inspect=12,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_class_shapes_when_one_pass_reads_sources_then_they_match_inspect(
    test_case: PythonClassSourceTestCase, tmp_path: Path
) -> None:
    files: dict[str, bytes] = {"shapes/node.py": Path(test_case.shapes_path).read_bytes()}
    with imported_identity_nodes(
        project_dir=tmp_path, package="class_shapes", files=files, nodes={}
    ):
        module: ModuleType = importlib.import_module("class_shapes.shapes.node")
        objects: list[object] = [attrgetter(path)(module) for path in test_case.object_paths]
        identity_pass: IdentityPass = IdentityPass()

        shared: list[str] = [identity_pass.source(obj) for obj in objects]
        inspected: list[str] = [
            textwrap.dedent(inspect.getsource(cast(Any, obj))).strip() for obj in objects
        ]

    assert sum(map(str.__eq__, shared, inspected)) == test_case.expected_sources_like_inspect
