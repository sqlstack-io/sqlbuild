"""Python-node identities stay byte-identical when one discovery pass shares their work."""

from __future__ import annotations

import ast
import hashlib
import importlib
import inspect
import linecache
import sys
import textwrap
from collections import Counter
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    DiscoveredPythonNodeFunctions,
    PythonNodeIdentity,
    discover_python_node_functions,
)
from sqlbuild.compiler.python_nodes._helpers.object_source import normalized_source
from sqlbuild.compiler.python_nodes._helpers.user_inventory import (
    python_identity_pass,
    python_node_identity,
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


@pytest.fixture
def shop_nodes(tmp_path: Path) -> Iterator[dict[str, tuple[Callable[..., object], Path]]]:
    package_dir: Path = tmp_path / _PACKAGE
    for relative_path, contents in _FILES.items():
        file_path: Path = package_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_bytes(contents)
    (tmp_path / ".git").mkdir()
    sys.path.insert(0, str(tmp_path))
    try:
        yield {
            name: (
                getattr(importlib.import_module(f"{_PACKAGE}.{folder}.node"), name),
                package_dir / folder / "node.py",
            )
            for name, folder in (("orders_node", "orders"), ("customers_node", "customers"))
        }
    finally:
        sys.path.remove(str(tmp_path))
        for module_name in [name for name in sys.modules if name.split(".")[0] == _PACKAGE]:
            del sys.modules[module_name]


def _identity(name: str, node: tuple[Callable[..., object], Path]) -> PythonNodeIdentity:
    function, file_path = node
    return python_node_identity(
        kind="task",
        name=name,
        function=function,
        file_path=file_path,
        decorator_config={"description": f"{name} node"},
    )


def _digest(identity: PythonNodeIdentity) -> str:
    fields: tuple[str, ...] = (
        identity.source_path,
        identity.source_hash,
        identity.definition_hash,
        identity.version_hash,
        identity.definition_json,
        identity.metadata_json,
    )
    return hashlib.sha256("\n".join(fields).encode()).hexdigest()


def _summary(identity: PythonNodeIdentity) -> tuple[tuple[str, ...], str]:
    return (
        tuple(f"{dependency.source_path}:{dependency.qualname}" for dependency in identity.dependencies),
        _digest(identity),
    )


def test_given_shared_shapes_when_identities_are_built_alone_then_they_match_recorded_identities(
    shop_nodes: dict[str, tuple[Callable[..., object], Path]],
) -> None:
    summaries: dict[str, tuple[tuple[str, ...], str]] = {
        name: _summary(_identity(name, node)) for name, node in shop_nodes.items()
    }

    assert summaries == _EXPECTED


def test_given_shared_shapes_when_one_pass_builds_every_identity_then_they_match_recorded_identities(
    shop_nodes: dict[str, tuple[Callable[..., object], Path]],
) -> None:
    with python_identity_pass():
        summaries: dict[str, tuple[tuple[str, ...], str]] = {
            name: _summary(_identity(name, node)) for name, node in shop_nodes.items()
        }

    assert summaries == _EXPECTED


_DOMAINS: tuple[str, ...] = ("orders", "customers", "inventory", "fulfillment", "support")

_SHARED_PROJECT_FILES: dict[str, str] = {
    "providers/warehouse_api/provider.py": '''"""Warehouse API client."""

from sqlbuild.providers import Provider


class WarehouseApi(Provider):
    """Client for the warehouse API."""

    def page_size(self) -> int:
        return 100
''',
    "python/shared/text.py": '''"""Text helpers."""


def normalize_text(value: str) -> str:
    return value.strip().lower()


def clean_status(value: str) -> str:
    return normalize_text(value).replace("-", "_")
''',
    "python/shared/money.py": '''"""Money helpers."""

from python.shared.text import normalize_text


class Money:
    def __init__(self, cents: int, currency: str) -> None:
        self.cents = cents
        self.currency = normalize_text(currency)

    def to_units(self) -> float:
        return to_units(self.cents)


class Ledger:
    def total(self, amounts: list[Money]) -> float:
        return sum(amount.to_units() for amount in amounts)


def to_units(cents: int) -> float:
    return cents / 100
''',
}


def _domain_rows(domain: str) -> str:
    return f'''"""{domain.title()} row helpers."""

from python.shared.money import Ledger, Money
from python.shared.text import clean_status


class RowBatch:
    def __init__(self, rows: list[dict[str, str]]) -> None:
        self.domain = "{domain}"
        self.rows = [map_row(row) for row in rows]
        self.total = Ledger().total([Money(1, "usd")])


def map_row(raw: dict[str, str]) -> dict[str, object]:
    status: str = clean_status(raw["status"])
    return {{"domain": "{domain}", "status": status, "amount": Money(1, "usd").to_units()}}
'''


def _node_files(domain: str, index: int) -> dict[str, str]:
    name: str = f"{domain}_sync_{index:03d}"
    folder: str = f"python/{domain}/{name}"
    return {
        f"{folder}/node.py": f'''"""Sync {domain} batch {index}."""

from providers.warehouse_api.provider import WarehouseApi
from python.{domain}._helpers.rows import RowBatch
from python.{domain}.{name}.main import main
from sqlbuild.tasks import task


@task(description="Sync {domain} batch {index}.")
def {name}(ctx, warehouse_api: WarehouseApi):
    RowBatch([])
    return main(ctx=ctx, warehouse_api=warehouse_api)
''',
        f"{folder}/main.py": f'''"""Sync {domain} batch {index}."""

from python.{domain}._helpers.rows import RowBatch


def main(*, ctx, warehouse_api):
    return {{"node": "{name}", "rows": len(RowBatch([]).rows), "size": warehouse_api.page_size()}}
''',
    }


def _write_shared_project(project_dir: Path, node_count: int) -> dict[str, str]:
    files: dict[str, str] = dict(_SHARED_PROJECT_FILES)
    for domain in _DOMAINS:
        files[f"python/{domain}/_helpers/rows.py"] = _domain_rows(domain)
    for index in range(node_count):
        files.update(_node_files(_DOMAINS[index % len(_DOMAINS)], index))
    for relative_path, contents in files.items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")
    (project_dir / ".git").mkdir()
    return {str((project_dir / path).resolve()): text for path, text in files.items()}


def _object_sources(files: dict[str, str]) -> dict[str, str]:
    """Each function's and class's dedented source, mapped to its file."""

    sources: dict[str, str] = {}
    for path, text in files.items():
        lines: list[str] = text.splitlines(keepends=True)
        for node in ast.walk(ast.parse(text)):
            if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef):
                start: int = (node.decorator_list[0] if node.decorator_list else node).lineno
                source: str = "".join(lines[start - 1 : node.end_lineno])
                sources[textwrap.dedent(source).strip()] = path
    return sources


def _discovery_work(
    project_dir: Path, node_count: int, monkeypatch: pytest.MonkeyPatch
) -> tuple[Counter[tuple[str, str]], set[str]]:
    """Reads, file parses, object parses and object hashes per file or object during discovery."""

    files: dict[str, str] = _write_shared_project(project_dir, node_count)
    sources: dict[str, str] = _object_sources(files)
    file_texts: dict[str, str] = {text: path for path, text in files.items()}
    work: Counter[tuple[str, str]] = Counter()
    update_cache: Callable[..., list[str]] = linecache.updatecache
    parse: Callable[..., ast.Module] = ast.parse
    sha256: Callable[..., Any] = hashlib.sha256

    def counted_update_cache(filename: str, module_globals: object = None) -> list[str]:
        if filename in files:
            work[("read", filename)] += 1
        return update_cache(filename, module_globals)

    def counted_parse(source: object, *args: object, **kwargs: object) -> ast.Module:
        if isinstance(source, str) and source in file_texts:
            work[("file parse", file_texts[source])] += 1
        elif isinstance(source, str) and source in sources:
            work[("object parse", source)] += 1
        return parse(source, *args, **kwargs)

    def counted_sha256(data: bytes = b"", *args: object, **kwargs: object) -> Any:
        text: str = bytes(data).decode("utf-8", "replace")
        if text in sources:
            work[("object hash", text)] += 1
        return sha256(data, *args, **kwargs)

    linecache.clearcache()
    with monkeypatch.context() as patch:
        patch.setattr(linecache, "updatecache", counted_update_cache)
        patch.setattr(ast, "parse", counted_parse)
        patch.setattr(hashlib, "sha256", counted_sha256)
        patch.syspath_prepend(str(project_dir))
        discovered: DiscoveredPythonNodeFunctions = discover_python_node_functions(
            project_dir=project_dir
        )
    for module_name in [name for name in sys.modules if name.split(".")[0] == "providers"]:
        del sys.modules[module_name]
    assert len(discovered.tasks) == node_count
    shared: set[str] = {
        key for path in files if "_sync_" not in path for key in (path, *_sources_in(sources, path))
    }
    return work, shared


def _sources_in(sources: dict[str, str], path: str) -> tuple[str, ...]:
    return tuple(source for source, source_path in sources.items() if source_path == path)


def test_given_nodes_sharing_helpers_when_discovered_then_each_file_and_object_is_processed_once(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    small, small_shared = _discovery_work(tmp_path / "small", 100, monkeypatch)
    large, large_shared = _discovery_work(tmp_path / "large", 500, monkeypatch)

    def shared_work(work: Counter[tuple[str, str]], shared: set[str]) -> Counter[str]:
        return Counter(kind for (kind, unit), count in work.items() if unit in shared for _ in range(count))

    assert max(small.values()) == 1
    assert max(large.values()) == 1
    assert shared_work(large, large_shared) == shared_work(small, small_shared)
    assert sum(large.values()) - sum(small.values()) == 4 * (
        sum(small.values()) - sum(shared_work(small, small_shared).values())
    )


_CLASS_SHAPES: str = '''import functools

if True:
    class Shadowed:
        def first(self):
            return 1


class Shadowed:  # noqa: F811
    def second(self):
        return 2


class Outer:
    class Inner:
        class Deepest:
            pass

    def method(self):
        class Local:
            pass

        return Local


@functools.total_ordering
class Decorated:
    def __lt__(self, other):
        return False

    def __eq__(self, other):
        return True


class Trailing:
    value = 1
    # trailing comment


def factory():
    class Made:
        pass

    return Made


LocalMade = factory()
OuterLocal = Outer().method()
'''


def test_given_class_shapes_when_reading_sources_in_one_table_then_they_match_inspect(
    tmp_path: Path,
) -> None:
    (tmp_path / "class_shapes.py").write_text(_CLASS_SHAPES, encoding="utf-8")
    sys.path.insert(0, str(tmp_path))
    try:
        module = importlib.import_module("class_shapes")
        objects: list[object] = [
            module.Shadowed,
            module.Shadowed.second,
            module.Outer,
            module.Outer.Inner,
            module.Outer.Inner.Deepest,
            module.Outer.method,
            module.Decorated,
            module.Decorated.__lt__,
            module.Trailing,
            module.factory,
            module.LocalMade,
            module.OuterLocal,
        ]
        table: dict[str, tuple[list[str], dict[str, int]]] = {}

        sources: list[str] = [normalized_source(obj, table) for obj in objects]

        assert sources == [textwrap.dedent(inspect.getsource(obj)).strip() for obj in objects]
    finally:
        sys.path.remove(str(tmp_path))
        sys.modules.pop("class_shapes", None)
