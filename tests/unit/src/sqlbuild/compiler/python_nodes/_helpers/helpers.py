"""Helpers for internal Python-node helper tests."""

from __future__ import annotations

import ast
import hashlib
import importlib
import importlib.util
import linecache
import sys
import textwrap
from collections import Counter
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from functools import partial
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import MappingProxyType, ModuleType
from typing import Any, NamedTuple, TypeGuard, cast

import pytest

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSource,
    CompileModelConfig,
    DiscoveredAssetFunction,
    DiscoveredCheckFunction,
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
    DiscoveredPythonNodeFunctions,
    DiscoveredSourceFile,
    LocalConfig,
    ProjectConfig,
    ProjectGraph,
    PythonNodeGraph,
    PythonNodeIdentity,
    PythonSqlRunSelection,
    SourceColumnEntry,
    SourceEntry,
    build_python_node_graph,
    compiled_graph_keys,
    discover_python_node_functions,
    resolve_python_sql_run_selection,
)
from sqlbuild.assets import asset
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
    CompiledSourceView,
)
from sqlbuild.compiler.python_nodes._helpers.user_inventory import python_node_identity
from sqlbuild.providers import Provider
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.python_nodes.types import PythonCheckSeverity
from sqlbuild.refs import model, source
from tests.unit.src.sqlbuild.compiler.graph.helpers import project_graph_from_indexes


def fetch_events(_ctx: object) -> list[dict[str, object]]:
    return []


def fetch_pages(_ctx: object) -> list[dict[str, object]]:
    return []


def load_events(_ctx: object) -> list[dict[str, object]]:
    return []


def raw_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def prepare_orders(_ctx: object) -> None:
    return None


def summarize_orders(_ctx: object) -> None:
    return None


@asset(name="prepare_orders")
def imported_prepare_orders(_ctx: object) -> None:
    return None


def export_orders(_ctx: object) -> None:
    return None


def check_orders_export(_ctx: object) -> bool:
    return True


class SlackProvider(Provider):
    channel: str = "#alerts"


def notify_orders(_ctx: object, slack_provider: SlackProvider) -> None:
    return None


def check_loaded_orders(_ctx: object) -> bool:
    return True


def build_terminal_loader_python_node_graph() -> PythonNodeGraph:
    """Build a graph containing one loader owned by a managed source."""
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=(
                DiscoveredLoaderFunction(
                    file_path=Path("/project/loaders/orders.py"),
                    relative_path=Path("python/nodes/loaders/orders.py"),
                    name="raw_orders",
                    function=raw_orders,
                ),
            ),
        )
    )


def build_orders_python_node_graph() -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=(
                DiscoveredLoaderFunction(
                    file_path=Path("/project/loaders/events.py"),
                    relative_path=Path("python/nodes/loaders/events.py"),
                    name="load_events",
                    function=load_events,
                    depends_on=(prepare_orders,),
                ),
            ),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="prepare_orders",
                    function=prepare_orders,
                    tags=("orders", "daily"),
                ),
                DiscoveredAssetFunction(
                    file_path=Path("/project/assets/orders.py"),
                    relative_path=Path("python/nodes/assets/orders.py"),
                    name="export_orders",
                    function=export_orders,
                    depends_on=(imported_prepare_orders,),
                    tags=("exports", "daily"),
                    columns=(
                        SourceColumnEntry(name="order_id", type="INTEGER"),
                        SourceColumnEntry(name="export_uri", type="VARCHAR"),
                    ),
                ),
            ),
            check_functions=(
                DiscoveredCheckFunction(
                    file_path=Path("/project/checks/orders.py"),
                    relative_path=Path("python/nodes/checks/orders.py"),
                    name="check_orders_export",
                    function=check_orders_export,
                    depends_on=(export_orders,),
                    severity=PythonCheckSeverity.WARN,
                    tags=("exports",),
                ),
            ),
        )
    )


def write_python_identity_repo(*, project_dir: Path, repo_files: dict[str, str]) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in repo_files.items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")
    (project_dir / ".git").mkdir(parents=True, exist_ok=True)


def load_python_identity_module(
    *, project_dir: Path, module_path: str, extra_sys_paths: tuple[str, ...] = ()
) -> ModuleType:
    file_path: Path = project_dir / module_path
    module_name: str = f"python_identity_{abs(hash((str(project_dir), module_path)))}"
    spec: ModuleSpec | None = importlib.util.spec_from_file_location(
        module_name,
        file_path,
    )
    loader: object | None = getattr(spec, "loader", None)
    _MODULE_SPEC_VALIDATORS[spec is not None and loader is not None](file_path)
    valid_spec: ModuleSpec = cast(ModuleSpec, spec)
    module: ModuleType = importlib.util.module_from_spec(valid_spec)
    original_path: list[str] = list(sys.path)
    _clear_project_modules(project_dir=project_dir)
    sys.path.insert(0, str(project_dir))
    extra_path: str
    for extra_path in reversed(extra_sys_paths):
        sys.path.insert(0, str(project_dir / extra_path))
    try:
        sys.modules[module_name] = module
        cast(Any, valid_spec.loader).exec_module(module)
    finally:
        sys.path[:] = original_path
    return module


def _accept_module_spec(file_path: Path) -> None:
    del file_path


def _reject_module_spec(file_path: Path) -> None:
    raise RuntimeError(f"Unable to load module from {file_path}")


_MODULE_SPEC_VALIDATORS: MappingProxyType[bool, Callable[[Path], None]] = MappingProxyType(
    {False: _reject_module_spec, True: _accept_module_spec}
)


def _clear_project_modules(*, project_dir: Path) -> None:
    top_level_names: set[str] = {
        (path.name, path.stem)[path.is_file()] for path in project_dir.iterdir()
    } - {(path.name, path.stem)[path.is_file()] for path in project_dir.glob(".*")}
    module_name: str
    for module_name in tuple(sys.modules):
        matches: tuple[bool, ...] = tuple(
            module_name == top_level_name or module_name.startswith(f"{top_level_name}.")
            for top_level_name in top_level_names
        )
        ({False: _keep_module, True: _remove_module}[any(matches)])(module_name)


def _keep_module(module_name: str) -> None:
    del module_name


def _remove_module(module_name: str) -> None:
    sys.modules.pop(module_name, None)


def build_sql_ref_python_node_graph(*, dependency: SqlResourceRef) -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="profile_orders",
                    function=prepare_orders,
                    depends_on=(dependency,),
                ),
            ),
        )
    )


def build_sibling_sql_ref_python_node_graph() -> PythonNodeGraph:
    """Return assets reading orders and, separately, its upstream raw_orders source."""

    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="export_orders",
                    function=prepare_orders,
                    depends_on=(model_ref("orders"),),
                ),
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/raw.py"),
                    relative_path=Path("python/nodes/tasks/raw.py"),
                    name="export_raw_orders",
                    function=prepare_orders,
                    depends_on=(source_ref("raw_orders"),),
                ),
            ),
        )
    )


def build_sql_downstream_asset_to_loader_python_node_graph() -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=(
                DiscoveredLoaderFunction(
                    file_path=Path("/project/loaders/orders.py"),
                    relative_path=Path("python/nodes/loaders/orders.py"),
                    name="load_events",
                    function=load_events,
                    depends_on=(prepare_orders,),
                ),
            ),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="prepare_orders",
                    function=prepare_orders,
                    depends_on=(model_ref("orders"),),
                ),
            ),
        )
    )


def model_ref(name: str) -> SqlResourceRef:
    return model(name)


def source_ref(name: str) -> SqlResourceRef:
    return source(name)


def build_terminal_loader_asset_dependency_python_node_graph() -> PythonNodeGraph:
    return _build_loader_dependency_python_node_graph(
        dependency_function=raw_orders,
        include_intermediate_loader=False,
        dependent_kind="asset",
    )


def build_terminal_loader_check_dependency_python_node_graph() -> PythonNodeGraph:
    return _build_loader_dependency_python_node_graph(
        dependency_function=raw_orders,
        include_intermediate_loader=False,
        dependent_kind="check",
    )


def build_intermediate_loader_asset_dependency_python_node_graph() -> PythonNodeGraph:
    return _build_loader_dependency_python_node_graph(
        dependency_function=fetch_pages,
        include_intermediate_loader=True,
        dependent_kind="asset",
    )


def build_intermediate_loader_check_dependency_python_node_graph() -> PythonNodeGraph:
    return _build_loader_dependency_python_node_graph(
        dependency_function=fetch_pages,
        include_intermediate_loader=True,
        dependent_kind="check",
    )


def _build_loader_dependency_python_node_graph(
    *,
    dependency_function: Callable[..., object],
    include_intermediate_loader: bool,
    dependent_kind: str,
) -> PythonNodeGraph:
    intermediate_loaders: tuple[DiscoveredLoaderFunction, ...] = (
        DiscoveredLoaderFunction(
            file_path=Path("/project/loaders/events.py"),
            relative_path=Path("python/nodes/loaders/events.py"),
            name="fetch_pages",
            function=fetch_pages,
        ),
        DiscoveredLoaderFunction(
            file_path=Path("/project/loaders/events.py"),
            relative_path=Path("python/nodes/loaders/events.py"),
            name="load_events",
            function=load_events,
            depends_on=(fetch_pages,),
        ),
    )
    terminal_loaders: tuple[DiscoveredLoaderFunction, ...] = (
        DiscoveredLoaderFunction(
            file_path=Path("/project/loaders/events.py"),
            relative_path=Path("python/nodes/loaders/events.py"),
            name="raw_orders",
            function=raw_orders,
        ),
    )
    loader_functions: tuple[DiscoveredLoaderFunction, ...] = (
        terminal_loaders,
        intermediate_loaders,
    )[include_intermediate_loader]

    asset_functions: tuple[DiscoveredAssetFunction, ...] = (
        DiscoveredAssetFunction(
            file_path=Path("/project/assets/orders.py"),
            relative_path=Path("python/nodes/assets/orders.py"),
            name="export_orders",
            function=export_orders,
            depends_on=(dependency_function,),
        ),
    )
    check_functions: tuple[DiscoveredCheckFunction, ...] = (
        DiscoveredCheckFunction(
            file_path=Path("/project/checks/orders.py"),
            relative_path=Path("python/nodes/checks/orders.py"),
            name="check_loaded_orders",
            function=check_loaded_orders,
            depends_on=(dependency_function,),
            severity=PythonCheckSeverity.ERROR,
        ),
    )
    asset_functions = ((), asset_functions)[dependent_kind == "asset"]
    check_functions = ((), check_functions)[dependent_kind != "asset"]

    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=loader_functions,
            asset_functions=asset_functions,
            check_functions=check_functions,
        )
    )


def build_python_node_graph_for_case(case_name: str) -> PythonNodeGraph:
    builders: MappingProxyType[str, Callable[[], PythonNodeGraph]] = MappingProxyType(
        {
            "terminal_loader_asset_dependency": (
                build_terminal_loader_asset_dependency_python_node_graph
            ),
            "terminal_loader_check_dependency": (
                build_terminal_loader_check_dependency_python_node_graph
            ),
            "intermediate_loader_asset_dependency": (
                build_intermediate_loader_asset_dependency_python_node_graph
            ),
            "intermediate_loader_check_dependency": (
                build_intermediate_loader_check_dependency_python_node_graph
            ),
            "orders": build_orders_python_node_graph,
        }
    )
    return builders.get(case_name, build_orders_python_node_graph)()


def build_orders_project_graph() -> ProjectGraph:
    raw_orders_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SOURCE,
        name="raw_orders",
    )
    orders_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    source_file: DiscoveredSourceFile = DiscoveredSourceFile(
        file_path=Path("/project/sources/raw.yml"),
        relative_path=Path("sql/sources/raw.yml"),
        contents="sources: []\n",
        source_entries=(),
    )
    raw_orders_source: CompiledSourceView = CompiledSource(
        key=raw_orders_key,
        deps=(),
        name="raw_orders",
        source_entry=SourceEntry(name="raw_orders", loader="raw_orders", managed=True),
        source_file=source_file,
    )
    orders_model: CompiledModelView = CompiledModel(
        key=orders_key,
        deps=(raw_orders_key,),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql='select * from __source("raw_orders")',
        config=CompileModelConfig(values={"tags": ["daily"]}),
        destination=CompiledRelationLocation(
            database=None,
            schema=None,
            name="orders",
            qualified_name="orders",
        ),
    )
    project: CompiledProjectView = CompiledProject(
        run_id="run-id",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(orders_model,),
        sources=(raw_orders_source,),
    )
    return project_graph_from_indexes(
        project=project,
        upstream_deps={raw_orders_key: (), orders_key: (raw_orders_key,)},
        downstream_deps={raw_orders_key: (orders_key,), orders_key: ()},
        tag_index={"daily": frozenset({orders_key})},
        path_index={orders_key: ""},
        all_keys={"raw_orders": raw_orders_key, "orders": orders_key},
    )


def build_model_depends_on_intermediate_loader_project_graph() -> ProjectGraph:
    project_graph: ProjectGraph = build_orders_project_graph()
    orders_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    intermediate_loader_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SOURCE,
        name="fetch_pages",
    )
    return project_graph_from_indexes(
        project=project_graph.project,
        upstream_deps={
            **project_graph.upstream_deps,
            orders_key: (intermediate_loader_key,),
        },
        downstream_deps={
            intermediate_loader_key: (orders_key,),
            orders_key: (),
        },
        tag_index=project_graph.tag_index,
        path_index=project_graph.path_index,
        all_keys=project_graph.all_keys,
    )


def resolve_run_selection(
    *,
    select: tuple[str, ...],
    exclude: tuple[str, ...],
    project_graph: ProjectGraph,
    python_graph: PythonNodeGraph,
) -> PythonSqlRunSelection:
    sql_keys, python_node_names = resolve_python_sql_run_selection(
        project_graph=project_graph, python_graph=python_graph, select=select, exclude=exclude
    )
    return PythonSqlRunSelection(
        sql_keys=compiled_graph_keys(sql_keys), python_node_names=frozenset(python_node_names)
    )


@contextmanager
def imported_identity_nodes(
    *, project_dir: Path, package: str, files: dict[str, bytes], nodes: dict[str, str]
) -> Iterator[dict[str, tuple[Callable[..., object], Path]]]:
    """Write `files` under `project_dir/package` and import each node function from its folder."""

    package_dir: Path = project_dir / package
    for relative_path, contents in files.items():
        file_path: Path = package_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_bytes(contents)
    (project_dir / ".git").mkdir(parents=True)
    try:
        with pytest.MonkeyPatch.context() as patch:
            patch.syspath_prepend(str(project_dir))
            yield {
                name: (
                    getattr(importlib.import_module(f"{package}.{folder}.node"), name),
                    package_dir / folder / "node.py",
                )
                for name, folder in nodes.items()
            }
    finally:
        _forget_modules(package)


def identity_summary(
    *, name: str, node: tuple[Callable[..., object], Path]
) -> tuple[tuple[str, ...], str]:
    """An asset node's dependency paths and qualnames, and a digest of every identity field."""

    function, file_path = node
    identity: PythonNodeIdentity = python_node_identity(
        kind="asset",
        name=name,
        function=function,
        file_path=file_path,
        decorator_config={"description": f"{name} node"},
    )
    fields: tuple[str, ...] = (
        identity.source_path,
        identity.source_hash,
        identity.definition_hash,
        identity.version_hash,
        identity.definition_json,
        identity.metadata_json,
    )
    return (
        tuple(f"{item.source_path}:{item.qualname}" for item in identity.dependencies),
        hashlib.sha256("\n".join(fields).encode()).hexdigest(),
    )


_DOMAINS: tuple[str, ...] = ("orders", "customers", "inventory", "fulfillment", "support")

_SHARED_PROJECT_FILES: dict[str, str] = {
    "python/providers/warehouse_api/provider.py": '''"""Warehouse API client."""

from sqlbuild.providers import Provider


class WarehouseApi(Provider):
    """Client for the warehouse API."""

    def page_size(self) -> int:
        return 100
''',
    "python/nodes/shared/text.py": '''"""Text helpers."""


def normalize_text(value: str) -> str:
    return value.strip().lower()


def clean_status(value: str) -> str:
    return normalize_text(value).replace("-", "_")
''',
    "python/nodes/shared/money.py": '''"""Money helpers."""

from python.nodes.shared.text import normalize_text


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

from python.nodes.shared.money import Ledger, Money
from python.nodes.shared.text import clean_status


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
    folder: str = f"python/nodes/{domain}/{name}"
    return {
        f"{folder}/node.py": f'''"""Sync {domain} batch {index}."""

from python.providers.warehouse_api.provider import WarehouseApi
from python.nodes.{domain}._helpers.rows import RowBatch
from python.nodes.{domain}.{name}.main import main
from sqlbuild.assets import asset


@asset(description="Sync {domain} batch {index}.")
def {name}(ctx, warehouse_api: WarehouseApi):
    RowBatch([])
    return main(ctx=ctx, warehouse_api=warehouse_api)
''',
        f"{folder}/main.py": f'''"""Sync {domain} batch {index}."""

from python.nodes.{domain}._helpers.rows import RowBatch


def main(*, ctx, warehouse_api):
    return {{"node": "{name}", "rows": len(RowBatch([]).rows), "size": warehouse_api.page_size()}}
''',
    }


def _write_shared_project(
    project_dir: Path, node_count: int
) -> tuple[dict[str, str], dict[str, str]]:
    """Write the project; return its shared files and its node files, by resolved path."""

    shared: dict[str, str] = dict(_SHARED_PROJECT_FILES)
    for domain in _DOMAINS:
        shared[f"python/nodes/{domain}/_helpers/rows.py"] = _domain_rows(domain)
    nodes: dict[str, str] = {}
    for index in range(node_count):
        nodes.update(_node_files(_DOMAINS[index % len(_DOMAINS)], index))
    for relative_path, contents in (shared | nodes).items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")
    (project_dir / ".git").mkdir(parents=True)
    return (
        {str((project_dir / path).resolve()): text for path, text in shared.items()},
        {str((project_dir / path).resolve()): text for path, text in nodes.items()},
    )


def _is_definition(
    node: ast.AST,
) -> TypeGuard[ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef]:
    return isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef)


def _object_sources(files: dict[str, str]) -> tuple[str, ...]:
    """Each function's and class's dedented source, as `inspect.getsource` reads it."""

    sources: list[str] = []
    for text in files.values():
        lines: list[str] = text.splitlines(keepends=True)
        for definition in filter(_is_definition, ast.walk(ast.parse(text))):
            start: int = [*definition.decorator_list, definition][0].lineno
            sources.append(
                textwrap.dedent("".join(lines[start - 1 : definition.end_lineno])).strip()
            )
    return tuple(sources)


def _unit_work(
    work: Counter[tuple[str, object]], files: dict[str, str]
) -> dict[tuple[str, str], int]:
    """Reads and parses of each file, and parses and hashes of each object source in it."""

    sources: tuple[str, ...] = _object_sources(files)
    return (
        {("read", path): work[("read", path)] for path in files}
        | {("file parse", path): work[("parse", text)] for path, text in files.items()}
        | {("object parse", source): work[("parse", source)] for source in sources}
        | {("object hash", source): work[("hash", source.encode())] for source in sources}
    )


def _work_by_kind(units: dict[tuple[str, str], int]) -> Counter[str]:
    by_kind: Counter[str] = Counter()
    for (kind, _unit), count in units.items():
        by_kind[kind] += count
    return by_kind


def _forget_modules(package: str) -> None:
    """Remove `package` and its submodules from `sys.modules`."""

    for module_name in tuple(filter(partial(_in_package, package), sys.modules)):
        del sys.modules[module_name]


def _in_package(package: str, module_name: str) -> bool:
    return module_name.split(".")[0] == package


class DiscoveryWork(NamedTuple):
    """Reads, parses and hashes counted while discovering a generated project."""

    assets: int
    most_per_unit: int
    shared: Counter[str]
    per_node: float


def discovery_work(
    *, project_dir: Path, node_count: int, monkeypatch: pytest.MonkeyPatch
) -> DiscoveryWork:
    """Count reads, parses and hashes of each project file and object while discovering it."""

    shared, nodes = _write_shared_project(project_dir, node_count)
    work: Counter[tuple[str, object]] = Counter()
    update_cache: Callable[..., list[str]] = linecache.updatecache
    parse: Callable[..., ast.Module] = ast.parse
    sha256: Callable[..., Any] = hashlib.sha256

    def counted_update_cache(
        filename: str, module_globals: dict[str, Any] | None = None
    ) -> list[str]:
        work[("read", filename)] += 1
        return update_cache(filename, module_globals)

    def counted_parse(source: str | bytes, *args: Any, **kwargs: Any) -> ast.Module:
        work[("parse", source)] += 1
        return parse(source, *args, **kwargs)

    def counted_sha256(data: bytes = b"", *args: Any, **kwargs: Any) -> Any:
        work[("hash", bytes(data))] += 1
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
    _forget_modules("providers")
    shared_units: dict[tuple[str, str], int] = _unit_work(work, shared)
    node_units: dict[tuple[str, str], int] = _unit_work(work, nodes)
    return DiscoveryWork(
        assets=len(discovered.assets),
        most_per_unit=max((shared_units | node_units).values()),
        shared=_work_by_kind(shared_units),
        per_node=sum(node_units.values()) / node_count,
    )
