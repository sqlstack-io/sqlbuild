"""Seeded SQL-test projects and both planning paths for the native planning glue tests."""

from __future__ import annotations

import random
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass
from itertools import chain, compress
from pathlib import Path
from typing import TYPE_CHECKING, cast

import sqlbuild._native as native_module
from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    CompiledSqlTest,
    NativeSqlTestAssembly,
    NativeSqlTestFailure,
    build_compile_inputs,
    build_project_graph,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.helpers import GoldenEntry, golden_entry

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@dataclass(frozen=True)
class PlanningCallOutcome:
    """What one planning call returned, or the type and text of what it raised."""

    value: object
    raised: tuple[str, str] | None


@dataclass(frozen=True)
class SqlTestCorpusShape:
    """The cursor windows and failing shapes a generated SQL-test project draws from."""

    windows: tuple[str, ...]
    stray_window_share: float
    helper_redefinition_share: float
    assertion_share: float
    unflattenable_assertion_share: float


_PROJECT_TOML: str = (
    'name = "orders_testing"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SOURCES: str = """sources:
  - name: raw_orders
    description: Orders feed.
    schema: main
    table: raw_orders
    columns:
      - name: order_id
        type: INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: INTEGER
      - name: status
        type: VARCHAR
      - name: ordered_at
        type: TIMESTAMP
  - name: raw_customers
    description: Customers feed.
    schema: main
    table: raw_customers
    columns:
      - name: customer_id
        type: INTEGER
      - name: region
        type: VARCHAR
"""
_SEEDS: str = """seeds:
  - name: regions
    description: Region labels.
    columns:
      - name: region
        type: VARCHAR
      - name: label
        type: VARCHAR
"""
_MACROS: str = (
    "def cents(column: str) -> str:\n"
    '    return f"({column}) * 100"\n\n'
    "def known_region() -> str:\n"
    '    return "region IS NOT NULL"\n'
)
_UDF: str = (
    'FUNCTION (\n  description "Whether an amount is large",\n'
    "  arguments (amount INTEGER),\n  returns BOOLEAN,\n);\n\namount > 100\n"
)
_TABLE_FUNCTION: str = (
    'FUNCTION (\n  description "Orders of one customer",\n'
    "  arguments (p_customer_id INTEGER),\n"
    "  returns table (order_id INTEGER, amount INTEGER)\n);\n\n"
    'SELECT order_id, amount FROM __ref("stg_orders") WHERE customer_id = p_customer_id\n'
)
_BASE_MODELS: dict[str, str] = {
    "stg_orders": (
        'SELECT order_id, customer_id, amount, status, ordered_at\nFROM __source("raw_orders")'
    ),
    "stg_customers": (
        'SELECT customer_id, region\nFROM __source("raw_customers")\nWHERE @py.known_region()'
    ),
    "int_orders": (
        'SELECT o.order_id, o.customer_id, c.region, @py.cents("o.amount") AS amount_cents,\n'
        '  __udf("udf__is_big")(o.amount) AS is_big\n'
        'FROM __ref("stg_orders") o\nJOIN __ref("stg_customers") c ON o.customer_id = c.customer_id'
    ),
    "fct_regions": (
        "WITH totals AS (\n"
        '  SELECT region, SUM(amount_cents) AS total_cents FROM __ref("int_orders") GROUP BY 1\n'
        ")\nSELECT t.region, r.label, t.total_cents\n"
        'FROM totals t\nLEFT JOIN __seed("regions") r ON t.region = r.region'
    ),
    "customer_orders": ('SELECT order_id, amount\nFROM __table_fn("table_fn__customer_orders")(1)'),
}
_DAILY_ORDERS: str = (
    "MODEL (description 'Daily orders.',\n  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n  cursor ordered_at,\n  cursor_type timestamp,\n"
    "  cursor_grain day,\n  cursor_inputs (raw_orders ordered_at,),\n);\n\n"
    "SELECT ordered_at, amount\n"
    'FROM __source("raw_orders")\n'
    "WHERE ordered_at >= __cursor_start() AND ordered_at < __cursor_end()\n"
)
_FIXTURES: dict[str, str] = {
    "__source__raw_orders": (
        "SELECT 1 AS order_id, 10 AS customer_id, 150 AS amount, 'placed' AS status, "
        "TIMESTAMP '2026-02-01 10:00:00' AS ordered_at"
    ),
    "__source__raw_customers": "SELECT 10 AS customer_id, 'east' AS region",
    "__seed__regions": "SELECT 'east' AS region, 'East' AS label",
    "__ref__stg_orders": (
        "SELECT 1 AS order_id, 10 AS customer_id, 150 AS amount, 'placed' AS status, "
        "TIMESTAMP '2026-02-01 10:00:00' AS ordered_at"
    ),
    "__ref__stg_customers": "SELECT 10 AS customer_id, 'east' AS region",
    "__ref__int_orders": (
        "SELECT 1 AS order_id, 10 AS customer_id, 'east' AS region, 15000 AS amount_cents, "
        "TRUE AS is_big"
    ),
    "__table_fn__table_fn__customer_orders": "SELECT 1 AS order_id, 150 AS amount",
    "__macro__cents": "SELECT 'amount'",
}
_UPSTREAM: dict[str, tuple[str, ...]] = {
    "stg_orders": ("__source__raw_orders",),
    "stg_customers": ("__source__raw_customers",),
    "int_orders": ("__ref__stg_orders", "__ref__stg_customers", "__source__raw_orders"),
    "fct_regions": ("__ref__int_orders", "__seed__regions", "__source__raw_customers"),
    "customer_orders": ("__table_fn__table_fn__customer_orders",),
    "daily_orders": ("__source__raw_orders",),
}
_DIRECT_TESTS: tuple[str, ...] = (
    'TEST (mode macro, name "doubles_cents");\n\nWITH\n'
    "amounts AS (SELECT 2 AS amount),\n"
    '__macro_actual__ AS (SELECT @py.cents("amount") AS cents FROM amounts),\n'
    "__macro_expected__ AS (SELECT 200 AS cents)\nSELECT 1\n",
    'TEST (mode udf, name "flags_big_amounts");\n\nWITH\n'
    '__udf_actual__ AS (SELECT __udf("udf__is_big")(150) AS is_big),\n'
    "__udf_expected__ AS (SELECT TRUE AS is_big)\nSELECT 1\n",
    'TEST (mode table_fn, name "lists_customer_orders");\n\nWITH\n'
    "__table_fn_actual__ AS (\n"
    '  SELECT order_id, amount FROM __table_fn("table_fn__customer_orders")(10)\n),\n'
    "__table_fn_expected__ AS (SELECT 1 AS order_id, 150 AS amount)\nSELECT 1\n",
)
_DIRECT_TEST_SHARE: float = 0.8
_MOCK_SHARE: float = 0.85
_MACRO_MOCK_SHARE: float = 0.5
_HELPER_SHARE: float = 0.4
_REGIONS_ASSERTION: str = 'SELECT * FROM __ref("fct_regions") WHERE 1 = 0'
NO_WINDOW: str = ""
VALID_WINDOW: str = ', cursor_start "2026-02-01", cursor_end "2026-02-03"'
INVERTED_WINDOW: str = ', cursor_start "2026-02-03", cursor_end "2026-02-01"'


def generated_sql_test_files(
    *, rng: random.Random, test_count: int, shape: SqlTestCorpusShape
) -> dict[str, str]:
    """A project of mocked, expected, asserted, helper, direct and failing SQL tests."""

    files: dict[str, str] = _project_files()
    files.update(
        (f"tests/unit/test_generated_{index}.sql", _model_test(rng=rng, index=index, shape=shape))
        for index in range(test_count)
    )
    files.update(
        compress(
            (
                (f"tests/unit/test_direct_{index}.sql", sql)
                for index, sql in enumerate(_DIRECT_TESTS)
            ),
            [rng.random() < _DIRECT_TEST_SHARE for _ in _DIRECT_TESTS],
        )
    )
    return files


def _project_files() -> dict[str, str]:
    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sources/raw.yml": _SOURCES,
        "seeds/regions.yml": _SEEDS,
        "seeds/regions.csv": "region,label\neast,East\n",
        "macros/python/amounts.py": _MACROS,
        "functions/sql/udf__is_big.sql": _UDF,
        "functions/sql/table_fn__customer_orders.sql": _TABLE_FUNCTION,
        "models/daily_orders.sql": _DAILY_ORDERS,
    }
    files.update(
        (f"models/{name}.sql", f'MODEL (description "Model {name}.");\n\n{sql}\n')
        for name, sql in _BASE_MODELS.items()
    )
    return files


def edge_sql_test_files(*, extra_files: dict[str, str]) -> dict[str, str]:
    """The planning project's models, sources and functions plus `extra_files`."""

    return {**_project_files(), **extra_files}


_DEEP_SUBQUERIES: int = 100
EDGE_UNICODE_CONTENTS: str = (
    'TEST (name "unicode_contents");\n\nWITH\n'
    "-- r\u00e9sum\u00e9: \u212aelvin_rows AS (a comment, not the header)\n"
    "kelvin_rows AS (\n"
    "  SELECT order_id, amount, 'caf\u00e9 \u2603' AS note "
    'FROM __source("raw_orders")\n),\n'
    "via_rows AS (SELECT *, '\u017f' AS long_s FROM kelvin_rows),\n"
    "__source__raw_orders AS (SELECT * FROM via_rows),\n"
    "__expected__stg_orders AS (SELECT order_id FROM KELVIN_ROWS)\nSELECT 1\n"
)
EDGE_DEEP_HELPER: str = (
    'TEST (name "deep_helper");\n\nWITH\n'
    "deep_rows AS (SELECT * FROM "
    + "(SELECT * FROM " * _DEEP_SUBQUERIES
    + "(WITH shadow AS (SELECT 1 AS order_id) SELECT * FROM shadow) AS s"
    + ") AS d" * _DEEP_SUBQUERIES
    + "),\n"
    'shadow AS (SELECT order_id, amount FROM __source("raw_orders")),\n'
    "__source__raw_orders AS (SELECT * FROM deep_rows),\n"
    "__expected__stg_orders AS (SELECT 1 AS order_id)\nSELECT 1\n"
)
EDGE_DECIMAL_CONTEXT: str = (
    'TEST (name "decimal_context", parameters (p_decimal (type decimal, nullable true)), '
    'cases (rounded (p_decimal "1.23456789012345678901234567895"), '
    'carried (p_decimal "9.99999999999999999999999999995E+999998"), '
    'subnormal (p_decimal "123456E-1000030"), '
    'flushed (p_decimal "-6E-1000028"), '
    'half_even (p_decimal "5E-1000027")));\n\n'
    'WITH\n__source__raw_orders AS (SELECT 1 AS order_id, @param("p_decimal") AS p),\n'
    '__assert__stg_orders_rows AS (SELECT * FROM __ref("stg_orders") WHERE 1 = 0)\nSELECT 1\n'
)
EDGE_DECIMAL_OVERFLOW: str = EDGE_DECIMAL_CONTEXT.replace(
    '"5E-1000027"', '"9.99999999999999999999999999995E+999999"'
)
EDGE_UNICODE_MACRO_SCOPE: dict[str, str] = {
    "models/weird_amounts.sql": (
        'MODEL (description "Model weird_amounts.");\n\n'
        'WITH o AS (SELECT amount FROM __source("raw_orders"))\nSELECT amount@\u00f6re FROM o\n'
    ),
    "tests/unit/test_direct_macro.sql": _DIRECT_TESTS[0],
}
EDGE_INVALID_UNREAD_HELPER: str = (
    'TEST (name "invalid_unread_helper");\n\nWITH\n'
    "bad_rows AS (SELECT * FROM __ref(1)),\n"
    'source_rows AS (SELECT order_id FROM __source("raw_orders")),\n'
    "__source__raw_orders AS (SELECT * FROM source_rows),\n"
    "__expected__stg_orders AS (SELECT 1 AS order_id)\nSELECT 1\n"
)


def _model_test(*, rng: random.Random, index: int, shape: SqlTestCorpusShape) -> str:
    """One model test: the first target's first mock, then random mocks, helper and checks.

    A helper may be read through a CTE whose nested WITH redefines it, which native planning
    rejects; an assertion on a model beginning with WITH cannot be flattened for SQL Server.
    """

    targets: list[str] = rng.sample(sorted(_UPSTREAM), k=rng.choice((1, 1, 2)))
    windowed: bool = "daily_orders" in targets or rng.random() < shape.stray_window_share
    window: str = rng.choice(((NO_WINDOW,), shape.windows)[windowed])
    fixtures: list[str] = list(chain.from_iterable(_UPSTREAM[target] for target in targets))
    mocks: list[str] = [
        fixtures[0],
        *compress(fixtures, [rng.random() < _MOCK_SHARE for _ in fixtures]),
        *rng.sample(sorted(_FIXTURES), k=rng.choice((0, 0, 0, 1))),
        *(("__macro__cents",) * ("int_orders" in targets and rng.random() < _MACRO_MOCK_SHARE)),
    ]
    has_helper: bool = rng.random() < _HELPER_SHARE
    helper: str = f"wanted_{index}"
    shadow: str = f"shadowing_{index}"
    redefined: bool = rng.random() < shape.helper_redefinition_share
    helper_ctes: list[tuple[str, str]] = [
        (
            shadow,
            f"WITH {helper} AS (SELECT 1 AS order_id) SELECT order_id FROM {helper}",
        ),
        (
            helper,
            (
                rng.choice(
                    (
                        "SELECT 1 AS order_id",
                        f'SELECT order_id FROM __ref("{rng.choice(targets)}")',
                    )
                ),
                f"SELECT order_id FROM {shadow}",
            )[redefined],
        ),
    ][int(not redefined) : 2 * int(has_helper)]
    expected_sql: str = ("SELECT 1 AS order_id", f"SELECT order_id FROM {helper}")[has_helper]
    checks: list[tuple[str, str]] = [
        (
            (f"__expected__{target}", expected_sql),
            (f"__assert__{target}_has_rows", f'SELECT * FROM __ref("{target}") WHERE 1 = 0'),
        )[rng.random() < shape.assertion_share]
        for target in targets
    ]
    unflattenable: list[tuple[str, str]] = [(f"__assert__regions_{index}", _REGIONS_ASSERTION)][
        : int(rng.random() < shape.unflattenable_assertion_share)
    ]
    ctes: dict[str, str] = {
        **{mock: _FIXTURES[mock] for mock in mocks},
        **dict(helper_ctes),
        **dict(checks),
        **dict(unflattenable),
    }
    body: str = ",\n".join(f"{name} AS (\n  {sql}\n)" for name, sql in ctes.items())
    return f'TEST (name "generated_{index}"{window});\n\nWITH\n{body}\nSELECT 1\n'


_CASE_VALUES: dict[str, tuple[str, ...]] = {
    "string": ('"open"', '"O\\\'Brien"', '"caf\u00e9 \u2603"', '""', '"a\\\\b"', "null"),
    "integer": ("0", "-7", "8", "9223372036854775807", "null"),
    "boolean": ("true", "false", "null"),
    "float": ("1.25", "-0.5", "0.1", "100.0", "0.0", "null"),
    "decimal": (
        '"2.4700"',
        '"-0.000"',
        '"3.00"',
        '"1E+5"',
        '"0.0001230"',
        '"12345678901234567890123456789.5"',
        "null",
    ),
}
_REFERENCING_HELPERS: tuple[str, ...] = (
    'SELECT order_id, amount FROM __source("raw_orders")',
    'SELECT order_id, amount FROM __ref("stg_orders")',
    'SELECT region, label FROM __seed("regions")',
    'SELECT order_id, amount FROM __table_fn("table_fn__customer_orders")(1)',
    "SELECT 1 AS order_id, 2 AS amount",
)
_MOCK_READS: tuple[str, ...] = (
    "SELECT * FROM {helper}",
    "SELECT * FROM {upper_helper}",
    "SELECT * FROM {via}",
    "WITH {helper} AS (SELECT 1 AS order_id) SELECT * FROM {helper}",
    "SELECT * FROM main.{helper}",
    "SELECT * FROM {helper} WHERE order_id ==> 1",
    "SELECT 1 AS order_id",
)
_ASSEMBLY_TARGETS: tuple[str, ...] = ("stg_orders", "int_orders", "fct_regions")


def generated_sql_test_assembly_files(
    *, rng: random.Random, test_count: int, shape: SqlTestCorpusShape
) -> dict[str, str]:
    """The planning projects plus typed parameter cases and mocks reading referencing helpers."""

    files: dict[str, str] = generated_sql_test_files(rng=rng, test_count=test_count, shape=shape)
    files.update(
        (f"tests/unit/test_cases_{index}.sql", _case_test(rng=rng, index=index))
        for index in range(test_count)
    )
    files.update(
        (f"tests/unit/test_helper_mock_{index}.sql", _helper_mock_test(rng=rng, index=index))
        for index in range(test_count)
    )
    return files


def _case_test(*, rng: random.Random, index: int) -> str:
    """A parameterized test over random typed parameters and up to three random cases."""

    kinds: list[str] = rng.sample(sorted(_CASE_VALUES), k=rng.randint(1, 3))
    parameters: str = ", ".join(f"p_{kind} (type {kind}, nullable true)" for kind in kinds)
    cases: str = ", ".join(
        f"case_{case} ({_case_values(rng=rng, kinds=kinds)})" for case in range(rng.randint(1, 3))
    )
    target: str = rng.choice(_ASSEMBLY_TARGETS)
    used: str = ", ".join(f'@param("p_{kind}") AS p_{kind}' for kind in kinds)
    return (
        f'TEST (name "cases_{index}", parameters ({parameters}), cases ({cases}));\n\n'
        f"WITH\n{_UPSTREAM[target][0]} AS (SELECT 1 AS order_id, {used}),\n"
        f'__assert__{target}_rows AS (SELECT * FROM __ref("{target}") WHERE 1 = 0)\nSELECT 1\n'
    )


def _case_values(*, rng: random.Random, kinds: list[str]) -> str:
    return ", ".join(f"p_{kind} {rng.choice(_CASE_VALUES[kind])}" for kind in kinds)


def _helper_mock_test(*, rng: random.Random, index: int) -> str:
    """A model test whose mocks read helpers, some of which call a reference."""

    helper: str = f"source_rows_{index}"
    via: str = f"via_{index}"
    target: str = rng.choice(_ASSEMBLY_TARGETS)
    mocks: list[str] = rng.sample(list(_UPSTREAM[target]), k=rng.randint(1, len(_UPSTREAM[target])))
    reads: dict[str, str] = {"helper": helper, "upper_helper": helper.upper(), "via": via}
    mock_ctes: list[str] = [
        f"{mock} AS (\n  {rng.choice(_MOCK_READS).format(**reads)}\n)" for mock in mocks
    ]
    check: str = rng.choice(
        (
            f"__expected__{target} AS (SELECT order_id FROM {helper})",
            f'__assert__{target}_rows AS (SELECT * FROM __ref("{target}") WHERE 1 = 0)',
            f"__expected__{target} AS (SELECT 1 AS order_id)",
        )
    )
    comment: str = rng.choice(("", "", "-- r\u00e9sum\u00e9\n"))
    ctes: str = ",\n".join(
        (
            f"{helper} AS (\n  {rng.choice(_REFERENCING_HELPERS)}\n)",
            f"{via} AS (SELECT * FROM {helper})",
            *mock_ctes,
            check,
        )
    )
    return f'TEST (name "helper_mock_{index}");\n\nWITH\n{comment}{ctes}\nSELECT 1\n'


_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=DuckDbAdapter(),
    collection_rendering=CollectionRendering.VALUE_LIST,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
)


def write_project(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write a project's files below `project_dir`."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def assembly_outcome(*, project_dir: Path) -> PlanningCallOutcome:
    """The compiled SQL tests and diagnostics with native assembly, or what it raises."""

    return _outcome(lambda: _compiled_tests_and_diagnostics(project_dir=project_dir))


def _compiled_tests_and_diagnostics(*, project_dir: Path) -> tuple[object, object]:
    project: CompiledProjectView = build_project_graph(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
    ).project
    return project.sql_tests, project.diagnostics


def native_assembly_answers(*, project_dir: Path) -> Counter[str]:
    """Count tests the native assembly answers for a project, with diagnostics, cases or failures.

    A test whose assembly raises counts under `failed_<kind>`.
    """

    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=_ADAPTER_CONTEXT,
        run_id="integration_run",
    )
    assemblies: tuple[NativeSqlTestAssembly, ...] = native_module.assemble_native_sql_tests(
        inputs=inputs, sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name
    )
    answers: Counter[str] = Counter()
    failures: list[NativeSqlTestFailure | None] = [item.failure for item in assemblies]
    answers.update(f"failed_{failure.kind}" for failure in filter(None, failures))
    tests: list[CompiledSqlTest | None] = [item.test for item in assemblies]
    native: list[CompiledSqlTest] = list(filter(None, tests))
    answers["native_assembled"] += len(native)
    answers["native_with_diagnostics"] += sum(bool(item.diagnostics) for item in assemblies)
    answers["native_case_fingerprints"] += sum(test.case_fingerprint is not None for test in native)
    return answers


def assembly_golden_entry(*, outcome: PlanningCallOutcome, project_dir: Path) -> GoldenEntry:
    """One project's compiled SQL tests, diagnostics and raised error as a golden entry."""

    tests, diagnostics = cast(
        tuple[object, object], (outcome.value, (None, None))[outcome.raised is not None]
    )
    return golden_entry(
        "sql_test_assembly",
        {"sql_tests": tests, "diagnostics": diagnostics, "raised": outcome.raised},
        masked=(str(project_dir),),
    )


def outcome_kind(outcome: object) -> str:
    """`raised` when the call raised, else `answered`; only native rows count as native work."""

    called: PlanningCallOutcome = cast(PlanningCallOutcome, outcome)
    return ("answered", "raised")[called.raised is not None]


def _outcome(call: Callable[[], object]) -> PlanningCallOutcome:
    try:
        return PlanningCallOutcome(value=call(), raised=None)
    except Exception as error:  # noqa: BLE001 - both paths must raise the same error
        return PlanningCallOutcome(value=None, raised=(type(error).__name__, str(error)))
