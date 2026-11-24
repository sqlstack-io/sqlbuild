"""Scoped projects and outcomes for the native declaration scope tests."""

from __future__ import annotations

import random
from collections import defaultdict
from collections.abc import Callable, Iterator, Mapping, Sequence
from dataclasses import dataclass, fields, replace
from itertools import chain
from pathlib import Path
from typing import cast

import pytest

import sqlbuild._native as _native
from sqlbuild._native import (
    ConstantDeclaration,
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DeclarationResolutionContext,
    DeclarationScopeBuild,
    DeclarationScopeResolver,
    DeclarationVisibility,
    DiscoveredProjectInputs,
    DiscoveryError,
    EnumDeclaration,
    LoadedMacro,
    ResourceIdentity,
    ResourceKind,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    SqlLexicalSyntax,
    VisibilityReason,
    VisibilityRecord,
    build_declaration_scope,
    discover_project_inputs,
    resolve_declaration_context,
    resolve_scope_declaration_visibility,
    resolve_scope_path_visibility,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile._helpers.render.user_macros import load_project_macros
from sqlbuild.compiler.compile.exceptions import CompileInputError
from tests.integration.src.sqlbuild.compiler.helpers import GoldenEntry, golden_entry

CURRENT_PATH_COMPONENT: str = "."
DECLARATION_LEXICAL_STEM: str = "__declaration__.sql"

PROJECT_CONFIG: str = (
    'name = "scope_parity"\nadapter = "duckdb"\n\n[connection]\ndatabase = "scope.duckdb"\n'
)
RAW_SOURCES: str = (
    "sources:\n  - name: raw_orders\n    description: Orders feed.\n"
    "    expression: >-\n      (SELECT 1 AS order_id)\n"
)
SCOPED_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": PROJECT_CONFIG,
    "sql/sources/raw.yml": RAW_SOURCES,
    "sql/macros/python/money.py": 'def cents(value: str) -> str:\n    return f"{value} * 100"\n',
    "sql/enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "sql/models/sales/_sqlbuild/macros/python/labels.py": (
        "def sales_label(value: str) -> str:\n    return f\"'sales:' || {value}\"\n"
    ),
    "sql/models/sales/_sqlbuild/constants/limits.sql": "CONSTANT (name sales_cap, value 3);\n",
    "sql/models/sales/eu/_macros/python/rates.py": (
        'def eu_rate(value: str) -> str:\n    return f"{value} * 2"\n'
    ),
    "sql/models/sales/eu/_enums/region.sql": "ENUM (name eu_region, members [NORTH, SOUTH]);\n",
    "sql/models/sales/orders.sql": (
        'MODEL (\n  description "Orders",\n  enums (\n    _state [OPEN, CLOSED],\n  ),\n);\n\n'
        "SELECT\n  order_id,\n  @py.sales_label('1') AS label,\n"
        '  @const("sales_cap") AS cap,\n  @enum("_state").OPEN AS state,\n'
        '  @enum("order_status").PLACED AS status\nFROM __source("raw_orders")\n'
    ),
    "sql/models/sales/eu/eu_orders.sql": (
        'MODEL (\n  description "EU orders",\n);\n\n'
        "SELECT\n  order_id,\n  @py.eu_rate('2') AS rate,\n"
        "  @enum(\"eu_region\").NORTH AS region,\n  @py.cents('3') AS cents,\n"
        "  @py.sales_label('4') AS label,\n  @const(\"sales_cap\") AS cap\n"
        'FROM __ref("orders")\n'
    ),
    "sql/models/other/customers.sql": (
        'MODEL (\n  description "Customers",\n);\n\n'
        "SELECT 1 AS customer_id, @enum(\"order_status\").SHIPPED AS status, @py.cents('5') AS "
        "cents\n"
    ),
    "tests/unit/test_orders.sql": (
        'TEST (name "orders_label");\n\nWITH\n'
        "__source__raw_orders AS (\n  SELECT 1 AS order_id\n),\n"
        "__expected__orders AS (\n"
        "  SELECT 1 AS order_id, @py.sales_label('1') AS label, @const(\"sales_cap\") AS cap\n)\n"
        "SELECT 1\n"
    ),
    "tests/unit/test_eu.sql": (
        'TEST (name "eu_rate");\n\nWITH\n__ref__orders AS (\n  SELECT 1 AS order_id\n),\n'
        "__expected__eu_orders AS (\n"
        "  SELECT 1 AS order_id, @py.eu_rate('2') AS rate, @enum(\"eu_region\").NORTH AS "
        "region\n)\n"
        "SELECT 1\n"
    ),
    "tests/unit/test_label_macro.sql": (
        'TEST (mode macro, name "labels_sales");\n\nWITH\n'
        "__macro_actual__ AS (\n  SELECT @py.sales_label('1') AS label\n),\n"
        "__macro_expected__ AS (\n  SELECT 'sales:' || 1 AS label\n)\nSELECT 1\n"
    ),
    "tests/scenarios/eu_flow.sql": (
        'SCENARIO (\n  description "EU flow"\n);\n\nWITH\n'
        "__ref__orders AS (\n  SELECT 1 AS order_id\n),\n"
        "__expected__eu_orders AS (\n"
        '  SELECT 1 AS order_id, @enum("eu_region").SOUTH AS region\n)\n'
        "SELECT 1\n"
    ),
}


@dataclass(frozen=True)
class ScopeOutcome:
    """What building the declaration scope produced."""

    kind: str
    value: tuple[object, ...]
    grants: int = 0
    error: str = ""


_BROKEN_SCENARIO_CTES: tuple[str, ...] = (
    "__expected__ AS (\n  SELECT 1\n)",
    "__expected__orders AS (\n  SELECT (1\n)",
    "__expected__orders (SELECT 1)",
)
_FOLDERS: tuple[str, ...] = (
    "sql/models/sales",
    "sql/models/sales/eu",
    "sql/models/sales/eu/north",
    "sql/models/other",
    "sql/models/other/archive",
)
_PLACEMENTS: tuple[str, ...] = (
    "sql/{kind}s/f{index}",
    "{folder}/{kind}s/f{index}",
    "{folder}/_sqlbuild/{kind}s/f{index}",
    "{folder}/_{kind}s/f{index}",
    "{folder}/_sqlbuild/_{kind}s/f{index}",
)
_DECLARATION_KINDS: tuple[str, ...] = ("macro", "enum", "constant")


def write_project(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write `files` below `project_dir`."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def generated_scope_files(*, rng: random.Random) -> dict[str, str]:
    """Return a seeded project of scoped declarations, models, tests and scenarios."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": PROJECT_CONFIG,
        "sql/sources/raw.yml": RAW_SOURCES,
    }
    models: list[str] = [f"model_{index}" for index in range(rng.randint(1, 6))]
    for index, model in enumerate(models):
        private: str = rng.choice(("", "", f"  constants (\n    _limit_{index} 2,\n  ),\n"))
        files[f"{rng.choice(_FOLDERS)}/{model}.sql"] = (
            f'MODEL (\n  description "Model {index}",\n{private});\n\n'
            'SELECT order_id\nFROM __source("raw_orders")\n'
        )
    macros: list[str] = []
    for index in range(rng.randint(0, 8)):
        kind: str = rng.choice(_DECLARATION_KINDS)
        path: str = rng.choice(_PLACEMENTS).format(
            kind=kind, folder=rng.choice(_FOLDERS), index=index
        )
        declaration: tuple[str, str, tuple[str, ...]] = _DECLARATION_WRITERS[kind](
            path=path, index=index, variant=rng.randint(0, 4)
        )
        files[declaration[0]] = declaration[1]
        macros.extend(declaration[2])
    for index in range(rng.randint(0, 4)):
        files[f"tests/unit/test_{index}.sql"] = _TEST_WRITERS[
            rng.choices(_TEST_KINDS, weights=_TEST_WEIGHTS)[0]
        ](
            index=index,
            calls=_macro_calls(rng=rng, macros=macros),
            expected=rng.sample([*models, "missing_model"], k=rng.randint(1, 2)),
        )
    scenario: dict[str, str] = {
        "tests/scenarios/flow.sql": (
            'SCENARIO (\n  description "Flow"\n);\n\nWITH\n'
            "__source__raw_orders AS (\n  SELECT 1 AS order_id\n),\n"
            f"__expected__{rng.choice([*models, 'missing_model'])} AS (\n"
            "  SELECT 1 AS order_id\n)\nSELECT 1\n"
        )
    }
    broken_scenario: dict[str, str] = {
        "tests/scenarios/flow.sql": (
            'SCENARIO (\n  description "Flow"\n);\n\nWITH\n'
            "__source__raw_orders AS (\n  SELECT 1 AS order_id\n),\n"
            f"{rng.choice(_BROKEN_SCENARIO_CTES)}\n"
        )
    }
    files.update(rng.choice((scenario, scenario, broken_scenario, {}, {}, {})))
    return files


def _macro_writer(*, path: str, index: int, variant: int) -> tuple[str, str, tuple[str, ...]]:
    del variant
    name: str = f"macro_{index}"
    root, stem = path.rsplit("/", 1)
    return (
        f"{root}/python/nodes/{stem}.py",
        f"def {name}(value: str) -> str:\n    return value\n",
        (name,),
    )


def _enum_writer(*, path: str, index: int, variant: int) -> tuple[str, str, tuple[str, ...]]:
    del index
    return f"{path}.sql", f"ENUM (name enum_{variant}, members [A, B]);\n", ()


def _constant_writer(*, path: str, index: int, variant: int) -> tuple[str, str, tuple[str, ...]]:
    del index
    return f"{path}.sql", f"CONSTANT (name constant_{variant}, value 1);\n", ()


_DECLARATION_WRITERS: dict[str, Callable[..., tuple[str, str, tuple[str, ...]]]] = {
    "macro": _macro_writer,
    "enum": _enum_writer,
    "constant": _constant_writer,
}


def _macro_calls(*, rng: random.Random, macros: list[str]) -> str:
    return "".join(
        f", @py.{name}('{position}') AS value_{position}"
        for position, name in enumerate(rng.sample(macros, k=min(len(macros), rng.randint(0, 2))))
    )


def _macro_test(*, index: int, calls: str, expected: list[str]) -> str:
    del expected
    return (
        f'TEST (mode macro, name "macro_test_{index}");\n\nWITH\n'
        f"__macro_actual__ AS (\n  SELECT 1 AS id{calls}\n),\n"
        f"__macro_expected__ AS (\n  SELECT 1 AS id{calls}\n)\nSELECT 1\n"
    )


def _broken_test(*, index: int, calls: str, expected: list[str]) -> str:
    del calls, expected
    return (
        f'TEST (name "broken_{index}");\n\nWITH\n__source__raw_orders AS (\n  SELECT 1\n),\n'
        "__expected__ AS (\n  SELECT 1\n)\nSELECT 1\n"
    )


def _broken_macro_test(*, index: int, calls: str, expected: list[str]) -> str:
    del expected
    return (
        f'TEST (mode macro, name "broken_macro_{index}");\n\nWITH\n'
        f"__macro_actual__ AS SELECT 1 AS id{calls}\n"
    )


def _expected_model_test(*, index: int, calls: str, expected: list[str]) -> str:
    blocks: str = ",\n".join(
        f"__expected__{model} AS (\n  SELECT 1 AS order_id{calls}\n)" for model in expected
    )
    return (
        f'TEST (name "test_{index}");\n\nWITH\n__source__raw_orders AS (\n'
        f"  SELECT 1 AS order_id\n),\n{blocks}\nSELECT 1\n"
    )


_TEST_KINDS: tuple[str, ...] = ("macro", "broken", "broken_macro", "expected_model")
_TEST_WEIGHTS: tuple[int, ...] = (3, 1, 1, 6)
_TEST_WRITERS: dict[str, Callable[..., str]] = {
    "macro": _macro_test,
    "broken": _broken_test,
    "broken_macro": _broken_macro_test,
    "expected_model": _expected_model_test,
}


def scope_outcome(*, project_dir: Path) -> ScopeOutcome:
    """Return what building the project's declaration scope produced."""

    try:
        discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
        macros: dict[str, LoadedMacro] = load_project_macros(discovered.macro_files)
    except (DiscoveryError, CompileInputError) as error:
        return ScopeOutcome(kind="discovery", value=(), error=str(error))
    return _scope_outcome(
        discovered=discovered, macros=macros, syntax=DuckDbAdapter().sql_lexical_syntax
    )


def scope_command_index(*, project_dir: Path) -> ScopeIndex:
    """Return the offline `sqb scope` index."""

    return _native.load_or_build_scope_index(project_dir=project_dir, no_cache=True)


def _scope_outcome(
    *,
    discovered: DiscoveredProjectInputs,
    macros: dict[str, LoadedMacro],
    syntax: SqlLexicalSyntax,
) -> ScopeOutcome:
    try:
        scope: DeclarationScopeBuild = build_declaration_scope(
            discovered_inputs=discovered, loaded_macros=macros, sql_lexical_syntax=syntax
        )
    except CompileInputError as error:
        return ScopeOutcome(kind="error", value=(type(error.__cause__).__name__,), error=str(error))
    return ScopeOutcome(
        kind="ok",
        value=(scope.index, _lookup_shape(scope.resolver.lookup), scope.resolver.resource_specific),
        grants=len(scope.index.grants),
    )


def _lookup_shape(lookup: ScopeLookup) -> tuple[object, ...]:
    return (
        lookup.index,
        tuple(lookup.resources.items()),
        tuple(lookup.resources_by_path.items()),
        tuple(lookup.declarations.items()),
        tuple(lookup.usages_by_consumer.items()),
        tuple(lookup.usages_by_declaration.items()),
        tuple(lookup.grants_by_resource.items()),
        lookup.visibility_index.identities,
        lookup.visibility_index.global_positions,
        tuple(lookup.visibility_index.private_positions.items()),
        tuple(lookup.visibility_index.local_positions.items()),
        tuple(lookup.visibility_index.inherited_positions.items()),
    )


_TEST_LABEL: str = "tests/unit/test.sql"
_SCENARIO_LABEL: str = "tests/scenarios/orders.sql"
_LEADING: tuple[str, ...] = (
    "",
    "-- header\n",
    "/* note ) */ ",
    "# hash comment\n",
    "// slash comment\n",
    "/* outer /* inner */ ) */ ",
    "\n\t ",
    "\x1c",
    "\u00a0",
    "'quoted' ",
)
_WITH: tuple[str, ...] = (
    "WITH ",
    "with\n",
    "WITH RECURSIVE ",
    "With/*c*/",
    "w\u0131th ",
    "WITHIN ",
)
_CTE_NAMES: tuple[str, ...] = (
    "__expected__orders",
    "__expected__customers",
    "__source__raw_orders",
    "__ref__stg_orders",
    "__assert__totals",
    "__expected__",
    "__expected__caf\u00e9",
    '"__expected__quoted"',
    "helper",
)
_COLUMN_LISTS: tuple[str, ...] = ("", " (order_id, amount)", "(x)")
_AS: tuple[str, ...] = (" AS ", " as\n", "AS", " ")
_BODIES: tuple[str, ...] = (
    "(SELECT 1 AS order_id)",
    "(SELECT ')' AS x, '(' AS y)",
    "(SELECT 'it\\')' AS x)",
    "(SELECT $$ ) $$ AS x)",
    "(SELECT $tag$ ( $tag$ AS x)",
    "(SELECT '''a ) b''' AS x)",
    "(SELECT r'\\' AS x) ",
    "(SELECT E'\\')' AS x)",
    "(SELECT 1 -- )\n)",
    "(SELECT 1 # )\n)",
    '(SELECT "caf\u00e9" FROM t)',
    "(SELECT (1)",
)
_TRAILING: tuple[str, ...] = (
    "",
    "\nSELECT 1\n",
    " select 1 ;",
    " SELECT 1; -- done\n",
    " SELECT 2",
    " SELECT 1 /* open",
    "\u00a0SELECT 1",
    " $x",
    " ; ",
)
LEXICAL_SYNTAXES: dict[str, SqlLexicalSyntax] = {
    "generic": SqlLexicalSyntax(),
    "backslash_triple_raw": SqlLexicalSyntax(
        backslash_escape_quotes=frozenset({"'", '"'}),
        raw_string_prefix=True,
        triple_quoted_strings=True,
        line_comment_prefixes=frozenset({"--", "#"}),
    ),
    "escape_prefix": SqlLexicalSyntax(escape_string_prefix=True),
    "nested_comments": SqlLexicalSyntax(nested_block_comments=True),
    "slash_comments": SqlLexicalSyntax(line_comment_prefixes=frozenset({"--", "//"})),
}


@dataclass(frozen=True)
class RelationshipScans:
    """The native relationship scans of one corpus as golden entries, and how many failed."""

    golden: list[GoldenEntry]
    scanned: int
    errors: int


def generated_expected_model_sqls(*, rng: random.Random, count: int) -> list[str]:
    """Return seeded SQL test bodies mixing valid, dialect-sensitive and invalid CTE layouts."""

    return [_generated_expected_model_sql(rng=rng) for _ in range(count)]


def _generated_expected_model_sql(*, rng: random.Random) -> str:
    ctes: list[str] = [
        f"{rng.choice(_CTE_NAMES)}{rng.choice(_COLUMN_LISTS)}{rng.choice(_AS)}{rng.choice(_BODIES)}"
        for _ in range(rng.randint(1, 4))
    ]
    return f"{rng.choice(_LEADING)}{rng.choice(_WITH)}{','.join(ctes)}{rng.choice(_TRAILING)}"


def _scan_outcome(outcome: tuple[str | None, Sequence[object]]) -> tuple[object, ...] | str:
    """A scan's error, or its values as a tuple."""

    error, values = outcome
    return {True: lambda: tuple(values), False: lambda: cast(str, error)}[error is None]()


def relationship_scans(*, sqls: list[str], syntax: SqlLexicalSyntax) -> RelationshipScans:
    """Scan each SQL text's test names, scenario names and test CTEs natively."""

    test_texts: list[tuple[str, str]] = [(sql, _TEST_LABEL) for sql in sqls]
    scenario_texts: list[tuple[str, str]] = [(sql, _SCENARIO_LABEL) for sql in sqls]
    outcomes: list[tuple[object, object, object]] = list(
        zip(
            map(
                _scan_outcome,
                _native.scope_expected_model_names(test_texts, False, syntax.native_mapping),
            ),
            map(
                _scan_outcome,
                _native.scope_expected_model_names(scenario_texts, True, syntax.native_mapping),
            ),
            map(_scan_outcome, _native.scope_test_ctes(test_texts, syntax.native_mapping)),
            strict=True,
        )
    )
    flat: list[object] = list(chain.from_iterable(outcomes))
    return RelationshipScans(
        golden=[golden_entry("relationship_scan", outcome) for outcome in outcomes],
        scanned=sum(isinstance(outcome, tuple) for outcome in flat),
        errors=sum(isinstance(outcome, str) for outcome in flat),
    )


UNICODE_RELATIONSHIP_CHARACTERS: tuple[str, ...] = (
    "\u0345",
    "\u216b",
    "\u00b2",
    "\u0663",
    "\u00e9",
    "\u1885",
    "\u2160",
    "\u00aa",
    "\u02b0",
    "\u0300",
    "\U0001d7ce",
    "\u2155",
    "\u00df",
    "\ufb00",
    "\u0130",
    "\u1e9e",
)
_UNICODE_RELATIONSHIP_TEMPLATES: tuple[str, ...] = (
    "WITH {c}x AS (SELECT 1) SELECT 1",
    "WITH x{c} AS (SELECT 1) SELECT 1",
    "WITH __expected__a{c} AS (SELECT 1)",
    "WITH __expected__a AS (SELECT 1) {c}",
    "{c}WITH x AS (SELECT 1)",
    "WITH{c} x AS (SELECT 1)",
    "WITH x AS{c}(SELECT 1)",
    "WITH x AS (SELECT 1) SELECT{c} 1",
    "WITH x AS (SELECT 1) ,{c}y AS (SELECT 2)",
    "wit{c} x AS (SELECT 1)",
    "WITH x a{c} (SELECT 1)",
)


def unicode_relationship_sqls(characters: tuple[str, ...]) -> list[str]:
    """Each template around each character: identifier, keyword and separator edges."""

    return list(
        chain.from_iterable(
            map(lambda template: template.format(c=character), _UNICODE_RELATIONSHIP_TEMPLATES)
            for character in characters
        )
    )


@dataclass(frozen=True)
class ContextParity:
    """Native and Python declaration contexts for every consumer of some projects."""

    targets: tuple[str, ...] = ()
    native: tuple[object, ...] = ()
    python: tuple[object, ...] = ()
    native_contexts: int = 0
    granted_contexts: int = 0
    private_contexts: int = 0
    python_only_paths: int = 0


_CONTEXT_DICT_FIELDS: tuple[str, ...] = tuple(
    item.name for item in fields(DeclarationResolutionContext) if item.name != "consumer"
)


def declaration_context_parity(
    *, project_dir: Path, monkeypatch: pytest.MonkeyPatch
) -> ContextParity:
    """Resolve every resource, resource path and declaration file natively and in Python."""

    try:
        discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
        macros: dict[str, LoadedMacro] = load_project_macros(discovered.macro_files)
        scope: DeclarationScopeBuild = build_declaration_scope(
            discovered_inputs=discovered,
            loaded_macros=macros,
            sql_lexical_syntax=DuckDbAdapter().sql_lexical_syntax,
        )
    except (DiscoveryError, CompileInputError):
        return ContextParity()
    resolver: DeclarationScopeResolver = scope.resolver
    targets: list[tuple[Path, ResourceIdentity | None]] = [
        *(
            (Path(records[0].path), identity)
            for identity, records in resolver.lookup.resources.items()
        ),
        *((Path(path), None) for path in resolver.lookup.resources_by_path),
        *((Path(record.path), None) for record in scope.index.declarations),
    ]
    native: list[DeclarationResolutionContext] = [
        resolve_declaration_context(
            resolver=replace(resolver, contexts_by_directory={}),
            file_path=file_path,
            resource=resource,
        )
        for file_path, resource in targets
    ]
    python: list[DeclarationResolutionContext] = [
        _python_declaration_context(resolver=resolver, file_path=file_path, resource=resource)
        for file_path, resource in targets
    ]
    indexed: list[bool] = [
        bool(resource) or file_path.as_posix() in resolver.lookup.resources_by_path
        for file_path, resource in targets
    ]
    records: list[list[VisibilityRecord]] = list(map(_visibility_records, native))
    return ContextParity(
        targets=tuple(
            f"{project_dir.name}:{file_path}:{resource}" for file_path, resource in targets
        ),
        native=tuple(map(_context_shape, native)),
        python=tuple(map(_context_shape, python)),
        native_contexts=sum(indexed),
        granted_contexts=sum(map(_has_grant, records)),
        private_contexts=sum(map(_has_private_value, records)),
        python_only_paths=len(indexed) - sum(indexed),
    )


def combined_context_parity(parities: list[ContextParity]) -> ContextParity:
    """Concatenate the compared contexts and add up the coverage counts of several projects."""

    return ContextParity(
        targets=tuple(chain.from_iterable(parity.targets for parity in parities)),
        native=tuple(chain.from_iterable(parity.native for parity in parities)),
        python=tuple(chain.from_iterable(parity.python for parity in parities)),
        native_contexts=sum(parity.native_contexts for parity in parities),
        granted_contexts=sum(parity.granted_contexts for parity in parities),
        private_contexts=sum(parity.private_contexts for parity in parities),
        python_only_paths=sum(parity.python_only_paths for parity in parities),
    )


def _has_grant(records: list[VisibilityRecord]) -> bool:
    return any(record.through is not None for record in records)


def _has_private_value(records: list[VisibilityRecord]) -> bool:
    return any(record.reason is VisibilityReason.PRIVATE_OWNER for record in records)


def _visibility_records(context: DeclarationResolutionContext) -> list[VisibilityRecord]:
    return list(
        chain.from_iterable(
            chain(
                context.enum_visibility.values(),
                context.constant_visibility.values(),
                context.macro_visibility.values(),
            )
        )
    )


def _context_shape(context: DeclarationResolutionContext) -> tuple[object, ...]:
    return (
        *(tuple(getattr(context, name).items()) for name in _CONTEXT_DICT_FIELDS),
        context.consumer,
    )


def _python_declaration_context(
    *, resolver: DeclarationScopeResolver, file_path: Path, resource: ResourceIdentity | None
) -> DeclarationResolutionContext:
    """The context the deleted Python projection built from the scope library's resolution."""

    resolution: DeclarationVisibility = resolve_scope_declaration_visibility(
        lookup=resolver.lookup, target=resource or file_path
    )
    visible, inaccessible, visibility = {
        True: _path_visibility,
        False: _resource_visibility,
    }[resolution.target.unknown](resolver=resolver, resolution=resolution, target_path=file_path)
    values: Mapping[DeclarationIdentity, object] = resolver.projection.declarations
    macro_records: tuple[DeclarationRecord, ...] = _valued(visible, values, LoadedMacro)
    return DeclarationResolutionContext(
        enums=_values_by_name(visible, values, EnumDeclaration),
        constants=_values_by_name(visible, values, ConstantDeclaration),
        inaccessible_enums=_by_name(inaccessible, DeclarationKind.ENUM),
        inaccessible_constants=_by_name(inaccessible, DeclarationKind.CONSTANT),
        enum_visibility=_visibility_by_name(visible, visibility, DeclarationKind.ENUM),
        constant_visibility=_visibility_by_name(visible, visibility, DeclarationKind.CONSTANT),
        macros=_values_by_name(visible, values, LoadedMacro),
        macro_records={record.identity.name: record for record in macro_records},
        macro_visibility=_visibility_by_name(visible, visibility, DeclarationKind.MACRO),
        inaccessible_macros=_by_name(inaccessible, DeclarationKind.MACRO),
        consumer=resource or next((match.identity for match in resolution.target.matches), None),
    )


def _valued(
    records: tuple[DeclarationRecord, ...],
    values: Mapping[DeclarationIdentity, object],
    value_type: type,
) -> tuple[DeclarationRecord, ...]:
    return tuple(
        filter(lambda record: isinstance(values.get(record.identity), value_type), records)
    )


def _values_by_name[T](
    records: tuple[DeclarationRecord, ...],
    values: Mapping[DeclarationIdentity, object],
    value_type: type[T],
) -> dict[str, T]:
    return {
        record.identity.name: cast(T, values[record.identity])
        for record in _valued(records, values, value_type)
    }


def _of_kind(
    records: tuple[DeclarationRecord, ...], kind: DeclarationKind
) -> tuple[DeclarationRecord, ...]:
    return tuple(filter(lambda record: record.identity.kind is kind, records))


def _by_name(
    records: tuple[DeclarationRecord, ...], kind: DeclarationKind
) -> dict[str, DeclarationRecord]:
    return {record.identity.name: record for record in _of_kind(records, kind)}


def _visibility_by_name(
    records: tuple[DeclarationRecord, ...],
    visibility: dict[DeclarationIdentity, list[VisibilityRecord]],
    kind: DeclarationKind,
) -> dict[str, tuple[VisibilityRecord, ...]]:
    return {
        record.identity.name: tuple(visibility.get(record.identity, ()))
        for record in _of_kind(records, kind)
    }


type _Visibility = tuple[
    tuple[DeclarationRecord, ...],
    tuple[DeclarationRecord, ...],
    dict[DeclarationIdentity, list[VisibilityRecord]],
]


def _resource_visibility(
    *, resolver: DeclarationScopeResolver, resolution: DeclarationVisibility, target_path: Path
) -> _Visibility:
    _ = target_path
    visibility: defaultdict[DeclarationIdentity, list[VisibilityRecord]] = defaultdict(list)
    for record in resolution.visible:
        visibility[record.declaration].append(record)
    return (
        tuple(resolver.lookup.declarations[item.declaration][0] for item in resolution.visible),
        tuple(resolver.lookup.declarations[identity][0] for identity in resolution.inaccessible),
        dict(visibility),
    )


_PATH_VISIBLE_REASONS: dict[ScopeKind, VisibilityReason] = {
    ScopeKind.GLOBAL: VisibilityReason.GLOBAL,
    ScopeKind.LOCAL: VisibilityReason.LOCAL_OWNER,
    ScopeKind.INHERITED: VisibilityReason.INHERITED_ANCESTOR,
}


def _path_visibility(
    *, resolver: DeclarationScopeResolver, resolution: DeclarationVisibility, target_path: Path
) -> _Visibility:
    _ = resolution
    lexical_path: Path = next(
        map(
            lambda record: (
                Path(record.owning_path or CURRENT_PATH_COMPONENT) / DECLARATION_LEXICAL_STEM
            ),
            filter(
                lambda record: (
                    record.path == target_path.as_posix() and record.scope is not ScopeKind.PRIVATE
                ),
                resolver.lookup.index.declarations,
            ),
        ),
        target_path,
    )
    visible, inaccessible = resolve_scope_path_visibility(lookup=resolver.lookup, path=lexical_path)
    path_resource: ResourceIdentity = ResourceIdentity(
        ResourceKind.MODEL, f"<path:{lexical_path.as_posix()}>"
    )
    reasons: Iterator[tuple[DeclarationRecord, VisibilityReason | None]] = (
        (record, _PATH_VISIBLE_REASONS[record.scope]) for record in visible
    )
    return (
        visible,
        inaccessible,
        {
            record.identity: [
                VisibilityRecord(path_resource, record.identity, cast(VisibilityReason, reason))
            ]
            for record, reason in filter(lambda pair: pair[1] is not None, reasons)
        },
    )
